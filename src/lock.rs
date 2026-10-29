//! Compositor-enforced protection, restricted to an explicitly spawned locker.
use crate::{CalloopData, Compositor, apps, focus::SurfaceFocus, state::ClientState};
use anyhow::{Context, Result, ensure};
use smithay::{
    delegate_session_lock,
    input::{keyboard::FilterResult, pointer::MotionEvent},
    output::Output,
    reexports::{
        calloop::{
            LoopHandle,
            ping::{Ping, make_ping},
            timer::{TimeoutAction, Timer},
        },
        wayland_protocols::ext::session_lock::v1::server::ext_session_lock_v1::ExtSessionLockV1,
        wayland_server::{
            Client, Resource, backend::DisconnectReason, protocol::wl_output::WlOutput,
        },
    },
    utils::SERIAL_COUNTER,
    wayland::{
        selection::data_device::set_data_device_focus,
        session_lock::{LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker},
    },
};
use std::{
    ffi::OsString,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    path::PathBuf,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tarot_core::session::{LockGate, LockPhase, RecoveryBudget};

pub struct Permission {
    pub connected: AtomicBool,
    pub allowed: AtomicBool,
    pub ping: Ping,
}
#[derive(Default)]
pub struct Protection {
    pub recovery_events: Vec<egui::Event>,
    pub recovery_touch_position: egui::Pos2,
    pub recovery_retry_key: bool,
    pub recovery_presented: Option<u64>,
    pub last_presented_recovery: bool,
    pub recovery_verified: Option<bool>,
    pub recovery_capture: Option<PathBuf>,
    pub recovery_capture_error: Option<String>,
    pub startup_reserved: bool,
    pub orphaned_presentations: u64,
    pub first_presented_protected: Option<bool>,
    pub unprotected_presentations: u64,
    pub gate: LockGate,
    pub owner: Option<ExtSessionLockV1>,
    pub confirmation: Option<SessionLocker>,
    pub surface: Option<(Output, LockSurface)>,
    pub verify_blank: bool,
    pub verify_surface_rgba: Option<[u8; 4]>,
    pub surface_verified: Option<bool>,
    pub fallback_verified: Option<bool>,
    pub last_presented_surface: Option<bool>,
    pub last_presented_generation: Option<u64>,
    pub presented_frames: u64,
    pub blank_verified: Option<bool>,
    pub blank_verified_generation: Option<u64>,
}
pub struct ManagedLocker {
    probe_mode: bool,
    budget: RecoveryBudget,
    failed: bool,
    started: Instant,
    pub restart_pending: bool,
    pub recoveries: u64,
    program: PathBuf,
    args: Vec<OsString>,
    handle: LoopHandle<'static, CalloopData>,
    ping: Ping,
    pub generation: u64,
    pub pid: Option<u32>,
    pidfd: Option<OwnedFd>,
    client: Option<Client>,
    pub permission: Option<Arc<Permission>>,
    pub error: Option<String>,
}
impl Drop for ManagedLocker {
    fn drop(&mut self) {
        if let Some(permission) = &self.permission {
            permission.allowed.store(false, Ordering::Release);
        }
        if let Some(pidfd) = &self.pidfd {
            let _ = rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::KILL);
        }
    }
}
pub fn start(
    state: &mut Compositor,
    handle: LoopHandle<'static, CalloopData>,
    program: PathBuf,
    args: Vec<OsString>,
    probe_mode: bool,
) -> Result<()> {
    let (ping, source) = make_ping()?;
    handle
        .insert_source(source, |(), _, data| data.state.check_locker())
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state.shell.lock_available = true;
    state.locker = Some(ManagedLocker {
        probe_mode,
        budget: RecoveryBudget::default(),
        failed: false,
        started: Instant::now(),
        restart_pending: false,
        recoveries: 0,
        program,
        args,
        handle,
        ping,
        generation: 0,
        pid: None,
        pidfd: None,
        client: None,
        permission: None,
        error: None,
    });
    if !probe_mode {
        if !state.protected() {
            state
                .protection
                .gate
                .reserve()
                .context("reserving lock protection")?;
        }
        state.enter_protection();
    }
    attempt(state);
    Ok(())
}
fn spawn(state: &mut Compositor) -> Result<()> {
    let mut managed = state.locker.take().context("No locker is configured")?;
    let result = (|| {
        if let Some(permission) = managed.permission.take() {
            permission.allowed.store(false, Ordering::Release);
        }
        if let Some(client) = managed.client.take() {
            state
                .display_handle
                .backend_handle()
                .kill_client(client.id(), DisconnectReason::ConnectionClosed);
        }
        if let Some(pidfd) = managed.pidfd.take() {
            let _ = rustix::process::pidfd_send_signal(&pidfd, rustix::process::Signal::KILL);
        }
        managed.pid = None;
        managed.generation = managed
            .generation
            .checked_add(1)
            .context("locker generation exhausted")?;
        managed.failed = false;
        managed.restart_pending = false;
        managed.started = Instant::now();
        let (server, stream) = UnixStream::pair()?;
        let permission = Arc::new(Permission {
            connected: AtomicBool::new(true),
            allowed: AtomicBool::new(true),
            ping: managed.ping.clone(),
        });
        let client = state.display_handle.clone().insert_client(
            server,
            Arc::new(ClientState {
                locker: Some(permission.clone()),
                ..Default::default()
            }),
        )?;
        let mut command = Command::new(&managed.program);
        command
            .args(&managed.args)
            .env("WAYLAND_SOCKET", stream.as_raw_fd().to_string())
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("DISPLAY")
            .stdin(std::process::Stdio::null());
        // SAFETY: the fork child only clears CLOEXEC on its owned private socket.
        // No other connection acquires the locker permission through this path.
        unsafe {
            command.pre_exec(move || {
                rustix::io::fcntl_setfd(&stream, rustix::io::FdFlags::empty())?;
                Ok(())
            });
        }
        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                permission.allowed.store(false, Ordering::Release);
                state
                    .display_handle
                    .backend_handle()
                    .kill_client(client.id(), DisconnectReason::ConnectionClosed);
                return Err(e.into());
            }
        };
        managed.pid = Some(child.id());
        managed.pidfd = rustix::process::Pid::from_raw(child.id() as i32).and_then(|p| {
            rustix::process::pidfd_open(p, rustix::process::PidfdFlags::empty()).ok()
        });
        if managed.pidfd.is_none() {
            let _ = child.kill();
            let _ = child.wait();
            permission.allowed.store(false, Ordering::Release);
            state
                .display_handle
                .backend_handle()
                .kill_client(client.id(), DisconnectReason::ConnectionClosed);
            anyhow::bail!("Locker supervision requires pidfd support");
        }
        managed.permission = Some(permission);
        managed.client = Some(client);
        managed.error = None;
        if let Some(sender) = &state.app_events {
            apps::monitor_child(
                managed.handle.clone(),
                child,
                apps::ChildTag::Locker(managed.generation),
                sender.clone(),
            );
        }
        Ok(())
    })();
    if result.is_err() {
        managed.error = Some("Locker could not be started".into());
    }
    state.locker = Some(managed);
    result
}
fn retire(state: &mut Compositor) {
    let Some(managed) = &mut state.locker else {
        return;
    };
    if let Some(permission) = managed.permission.take() {
        permission.allowed.store(false, Ordering::Release);
        permission.connected.store(false, Ordering::Release);
    }
    if let Some(client) = managed.client.take() {
        state
            .display_handle
            .backend_handle()
            .kill_client(client.id(), DisconnectReason::ConnectionClosed);
    }
    if let Some(pidfd) = managed.pidfd.take() {
        let _ = rustix::process::pidfd_send_signal(&pidfd, rustix::process::Signal::KILL);
    }
    managed.pid = None;
}
fn failed(state: &mut Compositor, error: &str) {
    if !state.protected() {
        return;
    }
    let Some(managed) = &mut state.locker else {
        return;
    };
    if managed.failed {
        return;
    }
    managed.failed = true;
    managed.error = Some(error.into());
    managed.restart_pending = false;
    let retry = if managed.probe_mode {
        None
    } else {
        managed.budget.failed(managed.started.elapsed())
    };
    let handle = managed.handle.clone();
    let generation = managed.generation;
    retire(state);
    state.protection.gate.orphan();
    state.protection.owner = None;
    state.protection.confirmation = None;
    state.protection.surface = None;
    state.clear_protected_focus();
    state.redraw = true;
    if let Some(delay) = retry {
        state.locker.as_mut().unwrap().restart_pending = true;
        if handle
            .insert_source(Timer::from_duration(delay), move |_, _, data| {
                if data.state.protected()
                    && data
                        .state
                        .locker
                        .as_ref()
                        .is_some_and(|m| m.generation == generation && m.restart_pending)
                {
                    data.state.locker.as_mut().unwrap().recoveries += 1;
                    attempt(&mut data.state);
                }
                TimeoutAction::Drop
            })
            .is_err()
        {
            let managed = state.locker.as_mut().unwrap();
            managed.restart_pending = false;
            managed.error = Some("Cannot schedule locker recovery".into());
        }
    }
}
fn attempt(state: &mut Compositor) {
    if state.protection.gate.phase == LockPhase::Orphaned
        && state.locker.as_ref().is_some_and(|m| !m.probe_mode)
    {
        if state.protection.gate.reserve().is_none() {
            return;
        }
        state.enter_protection();
    }
    if spawn(state).is_err() {
        failed(state, "Locker could not be started");
        return;
    }
    let managed = state.locker.as_ref().unwrap();
    if managed.probe_mode {
        return;
    }
    let generation = managed.generation;
    if managed
        .handle
        .insert_source(
            Timer::from_duration(Duration::from_secs(5)),
            move |_, _, data| {
                let state = &mut data.state;
                if !state.protected()
                    || !state
                        .locker
                        .as_ref()
                        .is_some_and(|m| m.generation == generation && !m.failed)
                {
                    return TimeoutAction::Drop;
                }
                if state.protection.gate.phase == LockPhase::Preparing {
                    return TimeoutAction::ToDuration(Duration::from_secs(1));
                }
                if state.protection.gate.phase == LockPhase::Awaiting
                    || state.active_lock_surface().is_none()
                {
                    failed(state, "Locker did not become ready");
                }
                TimeoutAction::Drop
            },
        )
        .is_err()
    {
        failed(state, "Cannot supervise locker startup");
    }
}
pub fn exited(state: &mut Compositor, generation: u64) {
    if !state
        .locker
        .as_ref()
        .is_some_and(|m| m.generation == generation && !m.failed)
    {
        return;
    }
    if state.protected() {
        failed(state, "Locker exited while protected");
    } else {
        retire(state);
    }
}
impl Compositor {
    pub fn protected(&self) -> bool {
        self.protection.gate.protected()
    }
    pub fn lock_generation(&self) -> Option<u64> {
        self.protected().then_some(self.protection.gate.generation)
    }
    /// Stop ordinary scene/input before forking or waiting for the trusted client.
    pub fn enter_protection(&mut self) {
        let effects = self.shell.cancel_network_input();
        self.apply_effects(effects);
        self.shell.system_menu_open = false;
        self.shell.launcher.menu_open = false;
        self.shell.menus.dismiss();
        self.shell.notifications_open = false;
        self.shell.shell_text_focus = false;
        self.protection.recovery_presented = None;
        self.protection.recovery_verified = None;
        self.protection.owner = None;
        self.protection.confirmation = None;
        self.protection.surface = None;
        self.protection.surface_verified = None;
        self.protection.fallback_verified = None;
        self.protection.last_presented_surface = None;
        self.protection.last_presented_generation = None;
        self.protection.blank_verified = None;
        self.protection.blank_verified_generation = None;
        self.snapshot = None;
        self.snapshot_error = None;
        self.clear_protected_focus();
        self.refresh_keyboard();
        self.ui_dirty = true;
        self.redraw = true;
        if let Some(server) = &self.system_services {
            server.watch(false);
        }
        if let Some(server) = &self.network {
            server.watch(false);
        }
    }
    pub fn request_lock(&mut self) -> Result<()> {
        ensure!(self.locker.is_some(), "No trusted locker is configured");
        if self.protected() {
            return Ok(());
        }
        self.protection
            .gate
            .reserve()
            .context("reserving lock protection")?;
        let managed = self.locker.as_mut().unwrap();
        managed.budget = RecoveryBudget::default();
        managed.restart_pending = false;
        self.enter_protection();
        attempt(self);
        Ok(())
    }
    pub fn check_locker(&mut self) {
        let Some(managed) = &self.locker else {
            return;
        };
        if !self.protected() || managed.failed {
            return;
        }
        let connection_lost = managed.permission.as_ref().is_none_or(|p| {
            !p.connected.load(Ordering::Acquire) || !p.allowed.load(Ordering::Acquire)
        });
        let owner_lost = matches!(
            self.protection.gate.phase,
            LockPhase::Preparing | LockPhase::Locked
        ) && self.protection.owner.as_ref().is_none_or(|o| !o.is_alive());
        let surface_lost = !managed.probe_mode
            && self
                .protection
                .surface
                .as_ref()
                .is_some_and(|(_, surface)| !surface.alive());
        if connection_lost || owner_lost || surface_lost {
            failed(self, "Locker connection or surface lost");
        }
    }
    pub fn restart_locker(&mut self) -> Result<()> {
        self.check_locker();
        ensure!(
            self.protection.gate.phase == LockPhase::Orphaned,
            "Locker recovery requires an orphaned protected session"
        );
        if let Some(managed) = &mut self.locker {
            managed.budget = RecoveryBudget::default();
            managed.restart_pending = false;
        }
        attempt(self);
        Ok(())
    }
    pub fn record_presentation(&mut self, protected: bool) {
        self.protection
            .first_presented_protected
            .get_or_insert(protected);
        if !protected {
            self.protection.unprotected_presentations += 1;
        }
    }
    pub fn recovery_visible(&self) -> bool {
        self.protection.gate.phase == LockPhase::Orphaned
            && self
                .locker
                .as_ref()
                .is_some_and(|m| !m.probe_mode && m.failed && !m.restart_pending)
    }
    pub fn recovery_ready(&self) -> bool {
        self.recovery_visible()
            && self.protection.recovery_presented == Some(self.protection.gate.generation)
    }
    pub fn retry_recovery(&mut self) {
        if self.recovery_ready()
            && let Err(error) = self.restart_locker()
        {
            tracing::warn!(%error,"protected recovery retry failed");
        }
    }
    pub fn lock_status(&self) -> serde_json::Value {
        serde_json::json!({"output":self.output_geometry,"idle":self.idle.status(!self.protected()),"sleep":self.sleep.status(),"recovery_visible":self.recovery_visible(),"recovery_ready":self.recovery_ready(),"last_presented_recovery":self.protection.last_presented_recovery,"recovery_verified":self.protection.recovery_verified,"recovery_capture_error":self.protection.recovery_capture_error,"orphaned_presentations":self.protection.orphaned_presentations,"startup_reserved":self.protection.startup_reserved,"first_presented_protected":self.protection.first_presented_protected,"unprotected_presentations":self.protection.unprotected_presentations,"phase":self.protection.gate.phase,"generation":self.protection.gate.generation,"blank_verified":self.protection.blank_verified,"blank_verified_generation":self.protection.blank_verified_generation,"surface_verified":self.protection.surface_verified,"fallback_verified":self.protection.fallback_verified,"last_presented_surface":self.protection.last_presented_surface,"last_presented_generation":self.protection.last_presented_generation,"presented_frames":self.protection.presented_frames,"locker":self.locker.as_ref().map(|l|serde_json::json!({"pid":l.pid,"generation":l.generation,"error":l.error,"restart_pending":l.restart_pending,"consecutive_failures":l.budget.failures,"recoveries":l.recoveries}))})
    }
    pub fn clear_protected_focus(&mut self) {
        self.protection.recovery_events.clear();
        self.protection.recovery_retry_key = false;
        self.cancel_input_sequences();
        self.popup_input.clear();
        self.ui_events.clear();
        let serial = SERIAL_COUNTER.next_serial();
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.unset_grab(self);
            keyboard.set_focus(self, None, serial);
            for code in keyboard.pressed_keys() {
                self.intercepted_keys.insert(code.raw());
                keyboard.input::<(), _>(
                    self,
                    code,
                    smithay::backend::input::KeyState::Released,
                    SERIAL_COUNTER.next_serial(),
                    0,
                    |_, _, _| FilterResult::Forward,
                );
            }
        }
        if let Some(pointer) = self.seat.get_pointer() {
            pointer.unset_grab(self, serial, 0);
            self.suppressed_pointer_buttons
                .extend(self.pointer_buttons.iter().copied());
            for code in self.pointer_buttons.clone() {
                pointer.button(
                    self,
                    &smithay::input::pointer::ButtonEvent {
                        button: code,
                        state: smithay::backend::input::ButtonState::Released,
                        serial,
                        time: 0,
                    },
                );
            }
            pointer.motion(
                self,
                None,
                &MotionEvent {
                    location: pointer.current_location(),
                    serial,
                    time: 0,
                },
            );
            pointer.frame(self);
        }
        let seat = self.seat.clone();
        set_data_device_focus(&self.display_handle, &seat, None);
    }
    pub fn refresh_lock_focus(&mut self) {
        if !self.protected() {
            return;
        }
        let surface = self.active_lock_surface();
        if let Some(keyboard) = self.seat.get_keyboard()
            && keyboard.current_focus() != surface
        {
            keyboard.set_focus(self, surface, SERIAL_COUNTER.next_serial());
        }
    }
    pub fn active_lock_surface(
        &self,
    ) -> Option<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface> {
        let surface = self.lock_callback_surface()?;
        smithay::backend::renderer::utils::with_renderer_surface_state(&surface, |state| {
            state.buffer().is_some()
                && state.surface_size()
                    == Some((self.layout.width as i32, self.layout.height as i32).into())
        })
        .unwrap_or(false)
        .then_some(surface)
    }
    /// A frame callback grants the trusted client a drawing opportunity, not
    /// permission to show or authenticate an obsolete-size buffer. EGL may
    /// wait for the old callback before it can submit the new-size buffer.
    pub fn lock_callback_surface(
        &self,
    ) -> Option<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface> {
        if self.protection.gate.phase != LockPhase::Locked {
            return None;
        }
        let (_, surface) = self.protection.surface.as_ref()?;
        if !surface.alive() {
            return None;
        }
        Some(surface.wl_surface().clone())
    }
    pub fn lock_surface_under(
        &self,
    ) -> Option<(
        SurfaceFocus,
        smithay::utils::Point<f64, smithay::utils::Logical>,
    )> {
        self.active_lock_surface()
            .map(|surface| (surface.into(), (0.0, 0.0).into()))
    }
    pub fn configure_lock_surface(&mut self) {
        if let Some((output, surface)) = &self.protection.surface {
            if !surface.alive() {
                return;
            }
            if let Some(mode) = output.current_mode() {
                let scale = output.current_scale().integer_scale().max(1) as u32;
                let size = output.current_transform().transform_size(mode.size);
                surface.with_pending_state(|state| {
                    state.size = Some((size.w as u32 / scale, size.h as u32 / scale).into())
                });
                surface.send_configure();
            }
        }
    }
    pub fn clear_color(&self) -> [f32; 4] {
        if self.protected() {
            [0.0, 0.0, 0.0, 1.0]
        } else {
            [0.10, 0.14, 0.16, 1.0]
        }
    }
    pub fn lock_frame_presented(&mut self, generation: u64, surface: bool, recovery: bool) {
        if self.lock_generation() == Some(generation) {
            self.protection.presented_frames += 1;
            if self.protection.gate.phase == LockPhase::Orphaned && !surface {
                self.protection.orphaned_presentations += 1;
            }
            self.protection.last_presented_generation = Some(generation);
            self.protection.last_presented_surface = Some(surface);
            self.protection.last_presented_recovery = recovery;
            if recovery
                && self.recovery_visible()
                && self.protection.recovery_presented != Some(generation)
            {
                self.clear_protected_focus();
                self.protection.recovery_presented = Some(generation);
            }
        }
        self.check_locker();
        if self.protection.gate.presented(generation) {
            if let Some(confirmation) = self.protection.confirmation.take() {
                confirmation.lock();
            }
            self.refresh_lock_focus();
            self.redraw = true;
        }
    }
}
impl SessionLockHandler for Compositor {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.session_lock_state
    }
    fn lock(&mut self, confirmation: SessionLocker) {
        self.check_locker();
        let Some(client) = confirmation.ext_session_lock().client() else {
            return;
        };
        let trusted = self
            .locker
            .as_ref()
            .and_then(|l| l.client.as_ref())
            .is_some_and(|c| *c == client)
            && self
                .locker
                .as_ref()
                .and_then(|l| l.permission.as_ref())
                .is_some_and(|p| {
                    p.connected.load(Ordering::Acquire) && p.allowed.load(Ordering::Acquire)
                });
        if !trusted || self.protection.gate.begin().is_none() {
            return;
        }
        self.enter_protection();
        self.protection.owner = Some(confirmation.ext_session_lock().clone());
        self.protection.confirmation = Some(confirmation);
    }
    fn unlock(&mut self) {
        if self.sleep.blocks_unlock()
            || self.idle.blocks_unlock()
            || self.output_geometry.blocks_unlock()
            || self.protection.last_presented_generation != Some(self.protection.gate.generation)
            || self.protection.last_presented_surface != Some(true)
            || self.active_lock_surface().is_none()
        {
            failed(self, "Unlock requested during sleep or output transition");
            return;
        }
        if !self
            .protection
            .gate
            .release(self.protection.gate.generation)
        {
            return;
        }
        if let Some(managed) = &mut self.locker {
            managed.restart_pending = false;
            if !managed.probe_mode
                && let Some(permission) = &managed.permission
            {
                permission.allowed.store(false, Ordering::Release);
            }
        }
        self.idle_activity();
        self.protection.owner = None;
        self.protection.surface = None;
        self.protection.confirmation = None;
        self.clear_protected_focus();
        self.ui_dirty = true;
        self.redraw = true;
        let focus = if let tarot_core::View::App(id) = self.shell.view {
            Some(id)
        } else {
            None
        };
        self.apply_effects(vec![tarot_core::Effect::Focus(focus)]);
        self.refresh_keyboard();
    }
    fn new_surface(&mut self, surface: LockSurface, output: WlOutput) {
        let Some(owner) = self.protection.owner.as_ref() else {
            return;
        };
        if surface.wl_surface().client() != owner.client() || !self.protected() {
            return;
        }
        let Some(output) = Output::from_resource(&output) else {
            return;
        };
        if self.protection.surface.is_some() {
            surface
                .wl_surface()
                .post_error(0u32, "Only one protected output is supported");
            return;
        }
        let mode = output.current_mode().unwrap();
        let size = output.current_transform().transform_size(mode.size);
        let scale = output.current_scale().integer_scale();
        surface.with_pending_state(|state| {
            state.size = Some((size.w as u32 / scale as u32, size.h as u32 / scale as u32).into())
        });
        self.protection.surface = Some((output, surface));
        self.redraw = true;
    }
}
delegate_session_lock!(Compositor);
