// Adapted from Smithay smallvil v0.7.0 (MIT).
// Copyright (c) 2017 Victor Berger and Victoria Brekenfeld.
// See third-party/Smithay-MIT.txt for the license.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::OsString,
    sync::Arc,
    time::Instant,
};
use tarot_core::{Command, Effect, Layout, ShellModel, View, WindowId};

use smithay::{
    backend::allocator::{Format, dmabuf::Dmabuf},
    desktop::{PopupManager, Space, Window, WindowSurfaceType},
    input::{Seat, SeatState},
    reexports::{
        calloop::{EventLoop, Interest, LoopSignal, Mode, PostAction, generic::Generic},
        wayland_server::{
            Display, DisplayHandle,
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
        },
    },
    utils::{Logical, Point},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        dmabuf::{DmabufFeedbackBuilder, DmabufGlobal, DmabufState},
        input_method::InputMethodManagerState,
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        session_lock::SessionLockManagerState,
        shell::{wlr_layer::WlrLayerShellState, xdg::XdgShellState},
        shm::ShmState,
        socket::ListeningSocketSource,
        text_input::TextInputManagerState,
        virtual_keyboard::VirtualKeyboardManagerState,
    },
};

use crate::CalloopData;

pub type DmabufImporter = Box<dyn FnMut(&Dmabuf) -> bool>;

pub struct Compositor {
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub space: Space<Window>,
    pub families: crate::families::Families,
    pub loop_signal: LoopSignal,

    // Smithay State
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub xdg_dialog_state: smithay::wayland::shell::xdg::dialog::XdgDialogState,
    pub shm_state: ShmState,
    pub dmabuf_state: DmabufState,
    pub dmabuf_global: Option<DmabufGlobal>,
    pub dmabuf_importer: Option<DmabufImporter>,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Compositor>,
    pub session_lock_state: SessionLockManagerState,
    pub protection: crate::lock::Protection,
    pub sleep: crate::sleep::State,
    pub idle: crate::idle::State,
    pub hardware: crate::hardware::State,
    pub locker: Option<crate::lock::ManagedLocker>,
    pub pointer_buttons: HashSet<u32>,
    pub suppressed_pointer_buttons: HashSet<u32>,
    pub data_device_state: DataDeviceState,
    pub text_input_manager_state: TextInputManagerState,
    pub input_method_manager_state: InputMethodManagerState,
    pub layer_shell_state: WlrLayerShellState,
    pub virtual_keyboard_state: VirtualKeyboardManagerState,
    pub keyboard: Option<crate::keyboard::KeyboardSurface>,
    pub input_method: Option<crate::input_method_process::ManagedInputMethod>,
    pub notifications: Option<crate::notifications::Server>,
    pub preferences: Option<crate::preferences::Server>,
    pub system_services: Option<crate::power::Server>,
    pub network: Option<crate::network::Server>,
    pub audio: Option<crate::audio::Server>,
    pub popups: PopupManager,
    pub popup_grab: Option<smithay::desktop::PopupGrab<Self>>,
    pub popup_owner: Option<WindowId>,
    pub popup_input: VecDeque<crate::popups::UserInput>,

    pub seat: Seat<Self>,
    pub shell: ShellModel,
    pub animation: tarot_core::motion::Animator,
    pub presentation: tarot_core::motion::Presentation,
    pub presented_windows: HashMap<WindowId, crate::focus::RenderGeometry>,
    pub presented_surfaces: HashMap<WlSurface, crate::focus::SurfacePlacement>,
    pub metrics: crate::metrics::RenderMetrics,
    pub app_events: Option<smithay::reexports::calloop::channel::Sender<crate::apps::AppEvent>>,
    pub ui_events: Vec<egui::Event>,
    pub ui_dirty: bool,
    pub redraw: bool,
    pub repaint_at: Option<Instant>,
    pub frame_clock: tarot_core::frame::FrameClock,
    pub frame_callback_at: Option<Instant>,
    pub frame_callback_timer_pending: bool,
    pub hidden_frame_clock: tarot_core::frame::FrameClock,
    pub hidden_frame_callback_at: Option<Instant>,
    pub hidden_frame_callback_timer_pending: bool,
    pub layout: Layout,
    pub output_geometry: tarot_core::orientation::OutputGeometry,
    pub sensors: Option<crate::sensors::Server>,
    pub scale: i32,
    pub next_window_id: u64,
    pub pointer_shell_owned: Option<bool>,
    pub shell_pointer_position: egui::Pos2,
    pub gestures: tarot_core::gesture::GestureRecognizer,
    pub gesture_started: Option<Instant>,
    pub touches: tarot_core::touch::TouchRouter,
    pub native_contacts: HashMap<smithay::backend::input::TouchSlot, (Point<f64, Logical>, u32)>,
    pub intercepted_keys: HashSet<u32>,
    pub snapshot: Option<std::path::PathBuf>,
    pub snapshot_after: std::time::Duration,
    pub snapshot_error: Option<String>,
    pub fatal_error: Option<String>,
}

impl Compositor {
    pub fn new(event_loop: &mut EventLoop<CalloopData>, display: Display<Self>) -> Self {
        let start_time = std::time::Instant::now();

        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_dialog_state =
            smithay::wayland::shell::xdg::dialog::XdgDialogState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let text_input_manager_state = TextInputManagerState::new::<Self>(&dh);
        let input_method_manager_state = InputMethodManagerState::new::<Self, _>(&dh, |client| {
            client
                .get_data::<ClientState>()
                .is_some_and(|data| data.trusted_input_method)
        });
        let layer_shell_state = WlrLayerShellState::new_with_filter::<Self, _>(&dh, |client| {
            client
                .get_data::<ClientState>()
                .is_some_and(|data| data.trusted_input_method)
        });
        let virtual_keyboard_state = VirtualKeyboardManagerState::new::<Self, _>(&dh, |client| {
            client
                .get_data::<ClientState>()
                .is_some_and(|data| data.trusted_input_method)
        });
        let session_lock_state = SessionLockManagerState::new::<Self, _>(&dh, |client| {
            client
                .get_data::<ClientState>()
                .and_then(|d| d.locker.as_ref())
                .is_some_and(|p| {
                    p.allowed.load(std::sync::atomic::Ordering::Acquire)
                        && p.connected.load(std::sync::atomic::Ordering::Acquire)
                })
        });
        let popups = PopupManager::default();
        smithay::wayland::idle_inhibit::IdleInhibitManagerState::new::<Self>(&dh);

        // A seat is a group of keyboards, pointer and touch devices.
        // A seat typically has a pointer and maintains a keyboard focus and a pointer focus.
        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "winit");

        // Notify clients that we have a keyboard, for the sake of the example we assume that keyboard is always present.
        // You may want to track keyboard hot-plug in real compositor.
        seat.add_keyboard(Default::default(), 200, 25).unwrap();

        // Notify clients that we have a pointer (mouse)
        // Here we assume that there is always pointer plugged in
        seat.add_pointer();
        seat.add_touch();

        // A space represents a two-dimensional plane. Windows and Outputs can be mapped onto it.
        //
        // Windows get a position and stacking order through mapping.
        // Outputs become views of a part of the Space and can be rendered via Space::render_output.
        let space = Space::default();

        let socket_name = Self::init_wayland_listener(display, event_loop);

        // Get the loop signal, used to stop the event loop
        let loop_signal = event_loop.get_signal();

        Self {
            start_time,
            display_handle: dh,

            space,
            families: Default::default(),
            loop_signal,
            socket_name,

            compositor_state,
            xdg_shell_state,
            xdg_dialog_state,
            shm_state,
            dmabuf_state: DmabufState::new(),
            dmabuf_global: None,
            dmabuf_importer: None,
            output_manager_state,
            seat_state,
            data_device_state,
            session_lock_state,
            protection: Default::default(),
            sleep: Default::default(),
            idle: Default::default(),
            hardware: Default::default(),
            locker: None,
            pointer_buttons: Default::default(),
            suppressed_pointer_buttons: Default::default(),
            text_input_manager_state,
            input_method_manager_state,
            layer_shell_state,
            virtual_keyboard_state,
            keyboard: None,
            input_method: None,
            notifications: None,
            preferences: None,
            system_services: None,
            network: None,
            audio: None,
            popups,
            popup_grab: None,
            popup_owner: None,
            popup_input: VecDeque::new(),
            seat,
            shell: ShellModel::default(),
            animation: Default::default(),
            presentation: Default::default(),
            presented_windows: Default::default(),
            presented_surfaces: Default::default(),
            ui_events: Vec::new(),
            ui_dirty: true,
            redraw: true,
            repaint_at: None,
            frame_clock: Default::default(),
            frame_callback_at: None,
            frame_callback_timer_pending: false,
            hidden_frame_clock: tarot_core::frame::FrameClock::from_refresh_millihz(4_000),
            hidden_frame_callback_at: None,
            hidden_frame_callback_timer_pending: false,
            layout: Layout {
                width: 360.0,
                height: 720.0,
                keyboard_height: 0.0,
            },
            scale: 1,
            output_geometry: Default::default(),
            sensors: None,
            next_window_id: 0,
            metrics: Default::default(),
            app_events: None,
            pointer_shell_owned: None,
            shell_pointer_position: egui::Pos2::ZERO,
            gestures: Default::default(),
            gesture_started: None,
            touches: Default::default(),
            native_contacts: Default::default(),
            intercepted_keys: HashSet::new(),
            snapshot: None,
            snapshot_after: std::time::Duration::ZERO,
            snapshot_error: None,
            fatal_error: None,
        }
    }

    fn init_wayland_listener(
        display: Display<Compositor>,
        event_loop: &mut EventLoop<CalloopData>,
    ) -> OsString {
        // Creates a new listening socket, automatically choosing the next available `wayland` socket name.
        let listening_socket = ListeningSocketSource::new_auto().unwrap();

        // Get the name of the listening socket.
        // Clients will connect to this socket.
        let socket_name = listening_socket.socket_name().to_os_string();

        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                // Inside the callback, you should insert the client into the display.
                //
                // You may also associate some data with the client when inserting the client.
                if let Err(error) = state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                {
                    tracing::warn!(%error, "accepting Wayland client");
                }
            })
            .expect("Failed to init the wayland event source.");

        // You also need to add the display itself to the event loop, so that client events will be processed by wayland-server.
        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    let lock_surface_was_alive = state
                        .state
                        .protection
                        .surface
                        .as_ref()
                        .map(|(_, s)| s.alive());
                    // Safety: we don't drop the display
                    unsafe {
                        if let Err(error) = display.get_mut().dispatch_clients(&mut state.state) {
                            tracing::warn!(%error, "dispatching Wayland clients");
                        }
                    }
                    if state.state.protected() {
                        state.state.check_locker();
                        state.state.refresh_lock_focus();
                        if lock_surface_was_alive
                            != state
                                .state
                                .protection
                                .surface
                                .as_ref()
                                .map(|(_, s)| s.alive())
                        {
                            state.state.clear_protected_focus();
                            state.state.redraw = true;
                        }
                    }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        socket_name
    }

    pub fn enable_dmabuf(
        &mut self,
        formats: Vec<Format>,
        device: Option<u64>,
        importer: DmabufImporter,
    ) -> anyhow::Result<()> {
        if formats.is_empty() {
            return Ok(());
        }
        let count = formats.len();
        let global = if let Some(device) = device {
            let feedback = DmabufFeedbackBuilder::new(device, formats).build()?;
            self.dmabuf_state
                .create_global_with_default_feedback::<Self>(&self.display_handle, &feedback)
        } else {
            self.dmabuf_state
                .create_global::<Self>(&self.display_handle, formats)
        };
        self.dmabuf_importer = Some(importer);
        self.dmabuf_global = Some(global);
        tracing::info!(
            format_count = count,
            ?device,
            "DMA-BUF client import enabled"
        );
        Ok(())
    }

    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(crate::focus::SurfaceFocus, Point<f64, Logical>)> {
        if self.protected() {
            return self.lock_surface_under();
        }
        if tarot_core::notifications::overlay_contains(
            &self.shell,
            self.layout,
            pos.x as f32,
            pos.y as f32,
        ) {
            return None;
        }
        if let Some(focus) = self.keyboard_surface_under(pos) {
            return Some(focus);
        }
        if !matches!(self.shell.view, View::App(_))
            || !self.layout.content().contains(pos.x as f32, pos.y as f32)
        {
            return None;
        }
        let View::App(active) = self.shell.view else {
            return None;
        };
        if !self
            .presentation
            .window(active)?
            .contains(pos.x as f32, pos.y as f32)
        {
            return None;
        }
        self.families
            .members(active)
            .into_iter()
            .rev()
            .find_map(|id| {
                if !self.families.accepts(id) {
                    return None;
                }
                let window = self.native_window(id)?;
                let local = self.presented_windows.get(&id)?.to_window(pos);
                window
                    .surface_under(local, WindowSurfaceType::ALL)
                    .map(|(surface, _)| (surface.into(), (0.0, 0.0).into()))
            })
    }

    pub fn apply_command(&mut self, command: Command) {
        if self.protected() {
            return;
        }
        if matches!(
            command,
            Command::Home
                | Command::Overview
                | Command::Launcher
                | Command::Activate(_)
                | Command::Launch(_)
                | Command::QuickLaunch(_)
                | Command::EditFavorites(_)
        ) {
            let effects = self.shell.cancel_network_input();
            self.apply_effects(effects);
        }
        if command == Command::CancelGesture {
            self.cancel_input_sequences();
            return;
        }
        let previous = self.shell.view;
        let selected = self.shell.selected;
        let menu = self.shell.card_menu_open;
        let dashboard = self.shell.notifications_open;
        let system_menu = self.shell.system_menu_open;
        let system_page = self.shell.system_page;
        let favorites = self.shell.favorites.clone();
        let editing_favorites = self.shell.editing_favorites;
        let deck: Vec<_> = self
            .shell
            .cards
            .iter()
            .map(|card| (card.id, card.stack))
            .collect();
        let effects = self.shell.apply(command);
        if previous != self.shell.view
            || favorites != self.shell.favorites
            || editing_favorites != self.shell.editing_favorites
            || selected != self.shell.selected
            || menu != self.shell.card_menu_open
            || dashboard != self.shell.notifications_open
            || system_menu != self.shell.system_menu_open
            || system_page != self.shell.system_page
            || deck
                != self
                    .shell
                    .cards
                    .iter()
                    .map(|card| (card.id, card.stack))
                    .collect::<Vec<_>>()
        {
            self.cancel_input_sequences();
        }
        if system_menu != self.shell.system_menu_open
            && let Some(server) = &self.system_services
        {
            server.watch(self.shell.system_menu_open);
        }
        self.apply_effects(effects);
        if (system_menu != self.shell.system_menu_open || system_page != self.shell.system_page)
            && let Some(network) = &self.network
        {
            network.watch(
                self.shell.system_menu_open
                    && self.shell.system_page == tarot_core::SystemPage::Wifi,
            );
        }
        self.ui_dirty = true;
        self.redraw = true;
        self.refresh_keyboard();
    }

    pub fn advance_presentation(&mut self) {
        if let Some(started) = self.gesture_started {
            self.gestures.tick_elapsed(started.elapsed());
        }
        if let Some(delay) = self.gestures.wake_after() {
            let deadline = Instant::now() + delay;
            self.repaint_at = Some(self.repaint_at.map_or(deadline, |old| old.min(deadline)));
        }
        let (frame, moving) = self.animation.advance(
            &self.shell,
            self.layout,
            self.gestures.preview(),
            self.start_time.elapsed(),
        );
        if frame != self.presentation {
            self.presentation = frame;
            self.ui_dirty = true;
        }
        if moving {
            let deadline = Instant::now() + std::time::Duration::from_millis(16);
            self.repaint_at = Some(self.repaint_at.map_or(deadline, |old| old.min(deadline)));
        }
    }

    pub fn apply_effects(&mut self, effects: Vec<Effect>) {
        for effect in effects {
            if self.protected()
                && !matches!(
                    effect,
                    Effect::Network(tarot_core::network::UiAction::CancelSecrets { .. })
                )
            {
                continue;
            }
            match effect {
                Effect::IdleTimeouts(settings) => {
                    if self.shell.display.idle_configurable && settings.valid() {
                        if let Some(policy) = &mut self.idle.policy {
                            policy.set_timeouts(settings, self.shell.display.panel_power_available);
                        }
                        self.shell.display.idle = settings;
                        self.persist_display_preferences();
                    }
                }
                Effect::RotationLock(locked) => {
                    if locked {
                        self.output_geometry.requested = None;
                    }
                    self.persist_display_preferences();
                }
                Effect::Lock => {
                    if let Err(error) = self.request_lock() {
                        tracing::warn!(%error,"lock request rejected");
                    }
                }
                Effect::Network(action) => {
                    let reply = matches!(
                        action,
                        tarot_core::network::UiAction::Secrets { .. }
                            | tarot_core::network::UiAction::CancelSecrets { .. }
                    );
                    if let Some(server) = &self.network {
                        match server.action(action) {
                            Ok(()) => {
                                if !reply {
                                    self.shell.network.busy = true;
                                }
                            }
                            Err(error) => self.shell.network.action_error = Some(error.to_string()),
                        }
                    }
                }
                Effect::Audio(action) => {
                    if let Some(server) = &self.audio {
                        match server.action(action.clone()) {
                            Ok(()) => self.shell.audio.pending = Some(action),
                            Err(error) => self.shell.audio.action_error = Some(error.to_string()),
                        }
                    }
                }
                Effect::Power(action) => {
                    if let Some(server) = &self.system_services {
                        match server.action(action.clone()) {
                            Ok(()) => {
                                let tarot_core::power::UiAction::Brightness { percent } = action;
                                self.shell.power.brightness.pending_percent = Some(percent);
                            }
                            Err(error) => {
                                self.shell.power.brightness.action_error = Some(error.to_string())
                            }
                        }
                    }
                }
                Effect::Notification(action) => {
                    if let Some(server) = &self.notifications
                        && let Err(error) = server.action(action)
                    {
                        self.shell.notifications.error = Some(error.to_string());
                    }
                }
                Effect::ToggleKeyboard => crate::input_method_process::toggle(self),
                Effect::ShellKey(action) => {
                    match action {
                        tarot_core::KeyboardAction::Text(text) => {
                            self.ui_events.push(egui::Event::Text(text))
                        }
                        tarot_core::KeyboardAction::Backspace
                        | tarot_core::KeyboardAction::Enter => {
                            let key = if action == tarot_core::KeyboardAction::Backspace {
                                egui::Key::Backspace
                            } else {
                                egui::Key::Enter
                            };
                            for pressed in [true, false] {
                                self.ui_events.push(egui::Event::Key {
                                    key,
                                    physical_key: None,
                                    pressed,
                                    repeat: false,
                                    modifiers: egui::Modifiers::NONE,
                                });
                            }
                        }
                        tarot_core::KeyboardAction::Hide => {}
                    }
                    self.ui_dirty = true;
                    self.redraw = true;
                }
                Effect::Focus(id) => {
                    let id = if self.shell.notifications_open || self.shell.system_menu_open {
                        None
                    } else {
                        id
                    };
                    let windows: Vec<_> = self.space.elements().cloned().collect();
                    let focused = id.and_then(|id| self.focused_window(id));
                    for window in &windows {
                        window.set_activated(focused.as_ref() == Some(window));
                        if let Some(toplevel) = window.toplevel()
                            && toplevel.is_initial_configure_sent()
                        {
                            toplevel.send_pending_configure();
                        }
                    }
                    let surface = focused.and_then(|window| {
                        window
                            .toplevel()
                            .map(|toplevel| toplevel.wl_surface().clone())
                    });
                    if let Some(keyboard) = self.seat.get_keyboard() {
                        keyboard.set_focus(
                            self,
                            surface,
                            smithay::utils::SERIAL_COUNTER.next_serial(),
                        );
                    }
                }
                Effect::RequestClose(id) => {
                    if let Some(window) = self
                        .space
                        .elements()
                        .find(|window| window.user_data().get::<WindowId>() == Some(&id))
                        && let Some(toplevel) = window.toplevel()
                    {
                        toplevel.send_close();
                    }
                }
                Effect::Launch(id) => {
                    if let Some(sender) = &self.app_events
                        && let Some(feedback) = &self.shell.launch_feedback
                    {
                        let _ = sender.send(crate::apps::AppEvent::Launch {
                            id,
                            generation: feedback.generation,
                        });
                    }
                }
                Effect::Favorites(favorites) => {
                    if let Some(server) = &self.preferences {
                        server.handle.favorites(favorites);
                        self.refresh_preferences_status();
                    }
                }
            }
        }
        self.refresh_keyboard();
    }

    pub fn configure_windows(&mut self) {
        self.configure_keyboard();
        let content = self.layout.content();
        let size = (content.width as i32, content.height as i32).into();
        let windows: Vec<_> = self.space.elements().cloned().collect();
        for window in windows {
            self.space
                .map_element(window.clone(), (content.x as i32, content.y as i32), false);
            if let Some(toplevel) = window.toplevel() {
                toplevel.with_pending_state(|state| {
                    let transient = window
                        .user_data()
                        .get::<WindowId>()
                        .is_some_and(|id| self.families.parent(*id).is_some());
                    state.size = if transient { None } else { Some(size) };
                    state.bounds = Some(size);
                });
                // Parent/size hints arrive between role creation and the
                // initial bufferless commit. Sending here earlier can make a
                // natural Qt dialog adopt the app's fullscreen dimensions.
                if toplevel.is_initial_configure_sent() {
                    toplevel.send_pending_configure();
                }
            }
        }
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
    pub trusted_input_method: bool,
    pub locker: Option<Arc<crate::lock::Permission>>,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {
        if let Some(permission) = &self.locker {
            permission
                .allowed
                .store(false, std::sync::atomic::Ordering::Release);
            permission
                .connected
                .store(false, std::sync::atomic::Ordering::Release);
            permission.ping.ping();
        }
    }
}
