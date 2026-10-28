//! Trusted same-user Rust/egui locker. Launch only over Tarot's private socket.
mod auth;
mod ui;
use anyhow::{Context, Result, ensure};
use calloop::EventLoop;
use calloop_wayland_source::WaylandSource;
use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, Instant},
};
use tarot_core::KeyboardAction as Action;
use tarot_wayland_egui::Graphics;
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop,
    protocol::{
        wl_callback, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat,
        wl_surface, wl_touch,
    },
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1, ext_session_lock_surface_v1, ext_session_lock_v1,
};
use xkbcommon::xkb;
use zeroize::Zeroize;

#[derive(Default)]
struct FrameGate {
    sequence: u64,
    pending: Option<u64>,
}
impl FrameGate {
    fn request(&mut self) -> u64 {
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("frame sequence exhausted");
        self.pending = Some(self.sequence);
        self.sequence
    }
    fn configure(&mut self) {
        self.pending = None;
    }
    fn done(&mut self, ticket: u64) {
        if self.pending == Some(ticket) {
            self.pending = None;
        }
    }
}

struct Locker {
    connection: Connection,
    qh: QueueHandle<Self>,
    compositor: Option<wl_compositor::WlCompositor>,
    output: Option<wl_output::WlOutput>,
    output_name: Option<u32>,
    manager: Option<ext_session_lock_manager_v1::ExtSessionLockManagerV1>,
    lock: Option<ext_session_lock_v1::ExtSessionLockV1>,
    surface: Option<wl_surface::WlSurface>,
    role: Option<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1>,
    graphics: Option<Graphics>,
    context: egui::Context,
    view: ui::View,
    entry: auth::Entry,
    attempt: Option<auth::Attempt>,
    status: ui::Status,
    retry_at: Option<Instant>,
    failures: u32,
    width: u32,
    height: u32,
    scale: i32,
    configured: bool,
    acknowledged: bool,
    position: egui::Pos2,
    contacts: HashSet<i32>,
    touch_owner: Option<i32>,
    touch_cancelled: bool,
    events: Vec<egui::Event>,
    key_state: Option<xkb::State>,
    dirty: bool,
    frame_gate: FrameGate,
    repaint_at: Option<Instant>,
    warmup: u8,
    painted_view: Option<(bool, bool)>,
    start: Instant,
    closed: bool,
    unlocked: bool,
    capture_dir: Option<PathBuf>,
    captured: HashSet<&'static str>,
}
impl Locker {
    fn trace_frame(&self, stage: &str) {
        use std::sync::{
            OnceLock,
            atomic::{AtomicU32, Ordering},
        };
        static ENABLED: OnceLock<bool> = OnceLock::new();
        static LINES: AtomicU32 = AtomicU32::new(0);
        if *ENABLED.get_or_init(|| std::env::var_os("TAROT_LOCK_FRAME_TRACE").is_some())
            && LINES.fetch_add(1, Ordering::Relaxed) < 200
        {
            eprintln!(
                "locker-frame {stage} size={}x{} pending={:?}",
                self.width, self.height, self.frame_gate.pending
            );
        }
    }
    fn ready(&self) -> bool {
        self.acknowledged && self.attempt.is_none() && self.retry_at.is_none()
    }
    fn action(&mut self, mut action: Action) {
        if matches!(action, Action::Hide) && self.attempt.is_none() {
            self.entry.clear();
            self.view.reset();
            self.status = ui::Status::Ready;
            self.dirty = true;
            return;
        }
        if !self.ready() {
            if let Action::Text(text) = &mut action {
                text.zeroize();
            }
            return;
        }
        match &mut action {
            Action::Text(text) => {
                self.view.editing = true;
                self.view.keyboard = true;
                self.entry.push(text);
                self.status = ui::Status::Ready;
                text.zeroize();
            }
            Action::Backspace => self.entry.backspace(),
            Action::Hide => {
                self.entry.clear();
                self.view.reset();
                self.status = ui::Status::Ready;
            }
            Action::Enter if self.entry.count() > 0 => {
                self.attempt = auth::Attempt::start(&mut self.entry).ok();
                self.status = if self.attempt.is_some() {
                    ui::Status::Checking
                } else {
                    ui::Status::Unavailable
                };
                self.events.clear();
                tarot_ui::cancel_pointer_input(&mut self.events);
            }
            Action::Enter => {
                self.view.editing = true;
                self.view.keyboard = true;
            }
        }
        self.dirty = true;
    }
    fn button(&mut self, pressed: bool) {
        self.events.push(egui::Event::PointerButton {
            pos: self.position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        self.dirty = true;
    }
    fn check_auth(&mut self) -> Result<()> {
        let Some(attempt) = &mut self.attempt else {
            return Ok(());
        };
        let outcome = attempt.poll().unwrap_or(Some(auth::Outcome::Unavailable));
        let Some(outcome) = outcome else {
            return Ok(());
        };
        self.attempt = None;
        match outcome {
            auth::Outcome::Accepted if self.acknowledged => {
                self.entry.clear();
                self.events.clear();
                self.lock
                    .take()
                    .context("no active lock")?
                    .unlock_and_destroy();
                self.connection.flush()?;
                self.connection.roundtrip()?;
                self.unlocked = true;
                self.closed = true;
            }
            _ => {
                self.failures = self.failures.saturating_add(1);
                let seconds = 2u64.pow(self.failures.min(5)).min(30);
                self.retry_at = Some(Instant::now() + Duration::from_secs(seconds));
                self.status = if outcome == auth::Outcome::Rejected {
                    ui::Status::Rejected
                } else {
                    ui::Status::Unavailable
                };
                self.entry.clear();
                self.events.clear();
                tarot_ui::cancel_pointer_input(&mut self.events);
                self.dirty = true;
            }
        }
        Ok(())
    }
    fn paint(&mut self) -> Result<()> {
        if !self.configured || !self.dirty || self.frame_gate.pending.is_some() || self.closed {
            return Ok(());
        }
        let surface = self.surface.as_ref().context("no lock surface")?;
        self.trace_frame("paint-start");
        surface.set_buffer_scale(self.scale);
        let (width, height) = (
            self.width * self.scale as u32,
            self.height * self.scale as u32,
        );
        if self.graphics.is_none() {
            self.graphics = Some(Graphics::new(&self.connection, surface, width, height)?);
        }
        self.trace_frame("graphics-ready");
        self.graphics.as_mut().unwrap().resize(width, height);
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.width as f32, self.height as f32),
            )),
            events: std::mem::take(&mut self.events),
            time: Some(self.start.elapsed().as_secs_f64()),
            focused: true,
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.scale as f32);
        tarot_ui::prepare_pointer_input(&self.context, &input.events);
        let (clock, date) = clock();
        let retry_seconds = self.retry_at.map_or(0, |d| {
            d.saturating_duration_since(Instant::now()).as_secs() + 1
        });
        let previous_view = (self.view.editing, self.view.keyboard);
        if self.painted_view != Some(previous_view) {
            self.warmup = 0;
        }
        let mut actions = Vec::new();
        let output = self.context.run(input, |ctx| {
            actions = self.view.show(
                ctx,
                ui::Frame {
                    count: self.entry.count(),
                    status: self.status,
                    retry_seconds,
                    acknowledged: self.acknowledged,
                    clock: &clock,
                    date: &date,
                },
            );
        });
        self.trace_frame("layout-ready");
        let delay = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(Duration::MAX, |o| o.repaint_delay);
        self.repaint_at = Instant::now().checked_add(delay.min(if self.retry_at.is_some() {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(20)
        }));
        let settled = self.warmup >= 2;
        if self.warmup < 2 {
            self.warmup += 1;
            self.repaint_at = Some(Instant::now());
        }
        let stable_view = previous_view == (self.view.editing, self.view.keyboard);
        if !stable_view {
            self.repaint_at = Some(Instant::now());
        }
        surface.frame(&self.qh, self.frame_gate.request());
        self.dirty = false;
        let label = match self.status {
            ui::Status::Checking => "checking",
            ui::Status::Rejected => "rejected",
            ui::Status::Unavailable => "unavailable",
            ui::Status::Ready if !self.view.editing => "cover",
            ui::Status::Ready if self.entry.count() == 0 => "entry-empty",
            ui::Status::Ready => "entry-masked",
        };
        let capture = self
            .capture_dir
            .as_ref()
            .filter(|_| settled && stable_view && self.captured.insert(label))
            .map(|dir| dir.join(format!("{}-{label}.png", std::process::id())));
        self.graphics
            .as_mut()
            .unwrap()
            .paint_capture(&self.context, output, capture.as_deref())?;
        self.trace_frame("paint-complete");
        self.painted_view = Some(previous_view);
        for action in actions {
            self.action(action);
        }
        Ok(())
    }
    fn clear_input(&mut self) {
        // A result from before display-off, VT loss or a focus boundary must
        // never authenticate after waking. Drop kills/reaps the owned worker.
        if self.attempt.take().is_some() {
            // Canceling submitted verification must not become a way to skip
            // the existing failed-attempt delay. Keep the accumulated budget.
            self.failures = self.failures.saturating_add(1);
            self.retry_at =
                Some(Instant::now() + Duration::from_secs(2u64.pow(self.failures.min(5)).min(30)));
            self.status = ui::Status::Unavailable;
        }
        self.entry.clear();
        self.view.reset();
        self.events.clear();
        tarot_ui::cancel_pointer_input(&mut self.events);
        self.contacts.clear();
        self.touch_owner = None;
        self.touch_cancelled = false;
        self.dirty = true;
    }
}
impl Drop for Locker {
    fn drop(&mut self) {
        self.attempt = None;
        self.entry.clear();
        self.graphics = None;
        if let Some(role) = self.role.take() {
            role.destroy();
        }
        if let Some(surface) = self.surface.take() {
            surface.destroy();
        }
        // Never send unlock from cleanup or a destructor.
    }
}
fn clock() -> (String, String) {
    // SAFETY: localtime_r writes only our tm; strftime writes into bounded arrays.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&now, &mut tm).is_null() {
            return ("--:--".into(), String::new());
        }
        let mut a = [0u8; 64];
        let mut b = [0u8; 128];
        let n = libc::strftime(a.as_mut_ptr().cast(), a.len(), c"%H:%M".as_ptr(), &tm);
        let m = libc::strftime(b.as_mut_ptr().cast(), b.len(), c"%A, %B %e".as_ptr(), &tm);
        (
            String::from_utf8_lossy(&a[..n]).into_owned(),
            String::from_utf8_lossy(&b[..m]).into_owned(),
        )
    }
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() == 1 && args[0] == "--pam-worker" {
        std::process::exit(auth::worker());
    }
    if args.len() == 1 && args[0] == "--help" {
        println!(
            "tarot-lock: trusted Rust/egui session locker; launch through tarot --lock-client. Authentication uses the current UID and PAM service tarot-lock."
        );
        return Ok(());
    }
    let capture_dir = if args.len() == 2 && args[0] == "--capture-dir" {
        let path = PathBuf::from(&args[1]);
        ensure!(path.is_dir(), "capture directory must already exist");
        Some(path)
    } else {
        ensure!(args.is_empty(), "unknown locker arguments");
        None
    };
    auth::protect_process()?;
    ensure!(
        std::env::var_os("WAYLAND_SOCKET").is_some(),
        "launch through Tarot's --lock-client option"
    );
    let connection = Connection::connect_to_env()?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    connection.display().get_registry(&qh, ());
    let mut state = Locker {
        connection: connection.clone(),
        qh,
        compositor: None,
        output: None,
        output_name: None,
        manager: None,
        lock: None,
        surface: None,
        role: None,
        graphics: None,
        context: egui::Context::default(),
        view: ui::View::default(),
        entry: auth::Entry::default(),
        attempt: None,
        status: ui::Status::Ready,
        retry_at: None,
        failures: 0,
        width: 360,
        height: 720,
        scale: 1,
        configured: false,
        acknowledged: false,
        position: egui::Pos2::ZERO,
        contacts: HashSet::new(),
        touch_owner: None,
        touch_cancelled: false,
        events: Vec::new(),
        key_state: None,
        dirty: true,
        frame_gate: FrameGate::default(),
        repaint_at: None,
        warmup: 0,
        painted_view: None,
        start: Instant::now(),
        closed: false,
        unlocked: false,
        capture_dir,
        captured: HashSet::new(),
    };
    queue.roundtrip(&mut state)?;
    let output = state.output.as_ref().context("no output")?;
    let lock = state
        .manager
        .as_ref()
        .context("no trusted session-lock interface")?
        .lock(&state.qh, ());
    let surface = state
        .compositor
        .as_ref()
        .context("no compositor")?
        .create_surface(&state.qh, ());
    state.role = Some(lock.get_lock_surface(&surface, output, &state.qh, ()));
    state.surface = Some(surface);
    state.lock = Some(lock);
    let mut event_loop: EventLoop<Locker> = EventLoop::try_new()?;
    WaylandSource::new(connection, queue)
        .insert(event_loop.handle())
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state.connection.flush()?;
    while !state.closed {
        let now = Instant::now();
        let timeout = if state.attempt.is_some() {
            Some(Duration::from_millis(40))
        } else {
            state.repaint_at.map(|d| d.saturating_duration_since(now))
        };
        event_loop.dispatch(timeout, &mut state)?;
        state.check_auth()?;
        let now = Instant::now();
        if state.retry_at.is_some_and(|d| d <= now) {
            state.retry_at = None;
            state.dirty = true;
        }
        if state.repaint_at.is_some_and(|d| d <= now) {
            state.repaint_at = None;
            state.dirty = true;
        }
        state.paint()?;
        state.connection.flush()?;
    }
    ensure!(state.unlocked, "session lock ended without authentication");
    Ok(())
}

impl Dispatch<wl_registry::WlRegistry, ()> for Locker {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(4), qh, ()))
                }
                "wl_output" if state.output.is_none() => {
                    state.output_name = Some(name);
                    state.output = Some(registry.bind(name, version.min(4), qh, ()))
                }
                "wl_seat" => {
                    let _: wl_seat::WlSeat = registry.bind(name, version.min(7), qh, ());
                }
                "ext_session_lock_manager_v1" => {
                    state.manager = Some(registry.bind(name, 1, qh, ()))
                }
                _ => {}
            },
            wl_registry::Event::GlobalRemove { name } if state.output_name == Some(name) => {
                state.closed = true;
            }
            _ => {}
        }
    }
}
impl Dispatch<wl_output::WlOutput, ()> for Locker {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Scale { factor } = event {
            state.scale = factor.clamp(1, 4);
            state.dirty = true;
            state.frame_gate.configure();
            state.warmup = 0;
            state.trace_frame("scale");
        }
    }
}
impl Dispatch<ext_session_lock_v1::ExtSessionLockV1, ()> for Locker {
    fn event(
        state: &mut Self,
        _: &ext_session_lock_v1::ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_session_lock_v1::Event::Locked => {
                state.acknowledged = true;
                state.dirty = true;
            }
            ext_session_lock_v1::Event::Finished => state.closed = true,
            _ => {}
        }
    }
}
impl Dispatch<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1, ()> for Locker {
    fn event(
        state: &mut Self,
        role: &ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            role.ack_configure(serial);
            state.width = width.max(1);
            state.height = height.max(1);
            state.configured = true;
            state.dirty = true;
            // Repaint new geometry without waiting for the previous callback;
            // its late completion cannot clear a newer frame's pending state.
            state.frame_gate.configure();
            state.warmup = 0;
            state.trace_frame("configure");
        }
    }
}
impl Dispatch<wl_callback::WlCallback, u64> for Locker {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        ticket: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.frame_gate.done(*ticket);
        state.trace_frame("frame-done");
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for Locker {
    fn event(
        _: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(c),
        } = event
        {
            if c.contains(wl_seat::Capability::Touch) {
                seat.get_touch(qh, ());
            }
            if c.contains(wl_seat::Capability::Pointer) {
                seat.get_pointer(qh, ());
            }
            if c.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(qh, ());
            }
        }
    }
}
impl Dispatch<wl_keyboard::WlKeyboard, ()> for Locker {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap {
                format: WEnum::Value(wl_keyboard::KeymapFormat::XkbV1),
                fd,
                size,
            } if size > 0 && size <= 1024 * 1024 => {
                let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
                // SAFETY: the compositor supplies the read-only keymap descriptor;
                // bounded nonzero size is checked before xkb's private mmap.
                state.key_state = unsafe {
                    xkb::Keymap::new_from_fd(
                        &context,
                        fd,
                        size as usize,
                        xkb::KEYMAP_FORMAT_TEXT_V1,
                        xkb::KEYMAP_COMPILE_NO_FLAGS,
                    )
                }
                .ok()
                .flatten()
                .map(|k| xkb::State::new(&k));
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(k) = &mut state.key_state {
                    k.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                }
            }
            wl_keyboard::Event::Leave { .. } => state.clear_input(),
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => {
                if !state.ready() {
                    return;
                }
                if let Some(k) = &state.key_state {
                    let code = xkb::Keycode::new(key + 8);
                    let symbol = k.key_get_one_sym(code).raw();
                    match symbol {
                        0xff0d | 0xff8d => state.action(Action::Enter),
                        0xff08 => state.action(Action::Backspace),
                        0xff1b => state.action(Action::Hide),
                        _ => {
                            let mut text = k.key_get_utf8(code);
                            if !text.is_empty() {
                                state.action(Action::Text(std::mem::take(&mut text)));
                            }
                            text.zeroize();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_touch::WlTouch, ()> for Locker {
    fn event(
        state: &mut Self,
        _: &wl_touch::WlTouch,
        event: wl_touch::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_touch::Event::Down { id, x, y, .. } => {
                state.contacts.insert(id);
                if state.contacts.len() > 1 {
                    state.touch_cancelled = true;
                    state.touch_owner = None;
                    tarot_ui::cancel_pointer_input(&mut state.events);
                    state.dirty = true;
                } else if !state.touch_cancelled {
                    state.touch_owner = Some(id);
                    state.position = egui::pos2(x as f32, y as f32);
                    state.events.push(egui::Event::PointerMoved(state.position));
                    state.button(true);
                }
            }
            wl_touch::Event::Motion { id, x, y, .. } if state.touch_owner == Some(id) => {
                state.position = egui::pos2(x as f32, y as f32);
                state.events.push(egui::Event::PointerMoved(state.position));
                state.dirty = true;
            }
            wl_touch::Event::Up { id, .. } => {
                if state.touch_owner == Some(id) {
                    state.button(false);
                    state.events.push(egui::Event::PointerGone);
                    state.touch_owner = None;
                }
                state.contacts.remove(&id);
                if state.contacts.is_empty() {
                    state.touch_cancelled = false;
                }
            }
            wl_touch::Event::Cancel => {
                tarot_ui::cancel_pointer_input(&mut state.events);
                state.contacts.clear();
                state.touch_owner = None;
                state.touch_cancelled = false;
                state.dirty = true;
            }
            _ => {}
        }
    }
}
impl Dispatch<wl_pointer::WlPointer, ()> for Locker {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface_x,
                surface_y,
                ..
            }
            | wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                state.position = egui::pos2(surface_x as f32, surface_y as f32);
                state.events.push(egui::Event::PointerMoved(state.position));
                state.dirty = true;
            }
            wl_pointer::Event::Button {
                button: 0x110,
                state: WEnum::Value(button),
                ..
            } => state.button(button == wl_pointer::ButtonState::Pressed),
            wl_pointer::Event::Leave { .. } => {
                state.events.push(egui::Event::PointerGone);
                state.dirty = true;
            }
            _ => {}
        }
    }
}

delegate_noop!(Locker: ignore wl_compositor::WlCompositor);
delegate_noop!(Locker: ignore wl_surface::WlSurface);
delegate_noop!(Locker: ignore ext_session_lock_manager_v1::ExtSessionLockManagerV1);

#[cfg(test)]
mod frame_tests {
    use super::FrameGate;
    #[test]
    fn configure_allows_a_new_buffer_without_an_old_callback_and_ignores_late_done() {
        let mut gate = FrameGate::default();
        let old = gate.request();
        gate.configure();
        assert!(gate.pending.is_none());
        let current = gate.request();
        gate.done(old);
        assert_eq!(gate.pending, Some(current));
        gate.done(current);
        assert!(gate.pending.is_none());
    }
}
