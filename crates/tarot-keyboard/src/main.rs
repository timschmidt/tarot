//! Native egui keyboard. Privileged protocols require Tarot's inherited connection.
use tarot_wayland_egui as graphics;
mod terminal;
mod ui;
use anyhow::{Context, Result, ensure};
use calloop::{EventLoop, Interest, Mode, PostAction, generic::Generic};
use calloop_wayland_source::WaylandSource;
use std::{
    collections::HashSet,
    io::{Read, Write},
    os::{
        fd::{AsFd, FromRawFd},
        unix::net::UnixStream,
    },
    time::{Duration, Instant},
};
use tarot_ui::keyboard::terminal::{Controls as TerminalControls, Key as TerminalKey, Modifiers};
use unicode_segmentation::UnicodeSegmentation;
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop,
    protocol::{
        wl_callback, wl_compositor, wl_output, wl_pointer, wl_registry, wl_seat, wl_surface,
        wl_touch,
    },
};
use wayland_protocols_misc::{
    zwp_input_method_v2::client::{zwp_input_method_manager_v2, zwp_input_method_v2},
    zwp_virtual_keyboard_v1::client::{zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1},
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

struct Surrounding {
    text: String,
    cursor: usize,
    anchor: usize,
}
impl Surrounding {
    fn deletion(&self) -> Option<(usize, usize)> {
        if !self.text.is_char_boundary(self.cursor) || !self.text.is_char_boundary(self.anchor) {
            return None;
        }
        if self.cursor != self.anchor {
            Some((
                self.cursor.saturating_sub(self.anchor),
                self.anchor.saturating_sub(self.cursor),
            ))
        } else {
            self.text[..self.cursor]
                .graphemes(true)
                .next_back()
                .map(|text| (text.len(), 0))
        }
    }
}
struct Keyboard {
    connection: Connection,
    qh: QueueHandle<Self>,
    compositor: Option<wl_compositor::WlCompositor>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    seat: Option<wl_seat::WlSeat>,
    method_manager: Option<zwp_input_method_manager_v2::ZwpInputMethodManagerV2>,
    virtual_manager: Option<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1>,
    method: Option<zwp_input_method_v2::ZwpInputMethodV2>,
    virtual_keyboard: Option<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1>,
    surface: Option<wl_surface::WlSurface>,
    layer: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    graphics: Option<graphics::Graphics>,
    context: egui::Context,
    ui: ui::KeyboardUi,
    terminal_controls: TerminalControls,
    terminal_masks: terminal::Masks,
    width: u32,
    height: u32,
    scale: i32,
    configured: bool,
    active: bool,
    hidden: bool,
    serial: u32,
    pending_active: Option<bool>,
    purpose: u32,
    hint: u32,
    pending_type: Option<(u32, u32)>,
    surrounding: Option<Surrounding>,
    pending_surrounding: Option<Surrounding>,
    position: egui::Pos2,
    contacts: HashSet<i32>,
    touch_owner: Option<i32>,
    touch_cancelled: bool,
    events: Vec<egui::Event>,
    dirty: bool,
    frame_pending: bool,
    repaint: bool,
    repaint_at: Option<Instant>,
    warmup_frames: u8,
    generation: u64,
    start: Instant,
    closed: bool,
}

impl Keyboard {
    fn terminal_mode(&self) -> bool {
        self.purpose == 13 && !self.secure()
    }
    fn secure(&self) -> bool {
        matches!(self.purpose, 8 | 9) || self.hint & 0xc0 != 0
    }
    fn show_surface(&mut self) {
        if self.surface.is_some() || !self.active || self.hidden {
            return;
        }
        let surface = self
            .compositor
            .as_ref()
            .unwrap()
            .create_surface(&self.qh, ());
        let layer = self.layer_shell.as_ref().unwrap().get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Overlay,
            "tarot-keyboard".into(),
            &self.qh,
            (),
        );
        let height = if self.terminal_mode() { 288 } else { 240 };
        layer.set_size(0, height);
        layer.set_anchor(
            zwlr_layer_surface_v1::Anchor::Bottom
                | zwlr_layer_surface_v1::Anchor::Left
                | zwlr_layer_surface_v1::Anchor::Right,
        );
        layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
        layer.set_exclusive_zone(height as i32);
        layer.set_margin(0, 0, 28, 0);
        surface.set_buffer_scale(self.scale);
        surface.commit();
        self.surface = Some(surface);
        self.layer = Some(layer);
        self.generation = self.generation.wrapping_add(1);
    }
    fn hide_surface(&mut self) {
        self.terminal_controls.reset();
        // EGL owns a wl_egl_window wrapping this surface. Destroy it first.
        self.graphics = None;
        if let Some(layer) = self.layer.take() {
            layer.destroy();
        }
        if let Some(surface) = self.surface.take() {
            surface.destroy();
        }
        self.configured = false;
        self.frame_pending = false;
        self.repaint = false;
        self.repaint_at = None;
        self.warmup_frames = 0;
        self.events.clear();
        self.contacts.clear();
        self.touch_owner = None;
        self.touch_cancelled = false;
        self.context = egui::Context::default();
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
    fn virtual_key(&self, code: u32) {
        let keyboard = self.virtual_keyboard.as_ref().unwrap();
        let time = self.start.elapsed().as_millis() as u32;
        keyboard.key(time, code, 1);
        keyboard.key(time, code, 0);
    }
    fn virtual_chord(&self, code: u32, modifiers: Modifiers, shift: bool) {
        let mask = if modifiers.control {
            self.terminal_masks.control
        } else {
            0
        } | if modifiers.alt {
            self.terminal_masks.alt
        } else {
            0
        } | if shift { self.terminal_masks.shift } else { 0 };
        let keyboard = self.virtual_keyboard.as_ref().unwrap();
        keyboard.modifiers(mask, 0, 0, 0);
        self.virtual_key(code);
        keyboard.modifiers(0, 0, 0, 0);
    }
    fn terminal_key(&mut self, key: TerminalKey) {
        if !self.active || !self.terminal_mode() {
            return;
        }
        let modifiers = self.terminal_controls.take_modifiers();
        match key {
            TerminalKey::Paste => self.virtual_chord(
                47,
                Modifiers {
                    control: true,
                    alt: false,
                },
                true,
            ),
            other => self.virtual_chord(
                match other {
                    TerminalKey::Tab => 15,
                    TerminalKey::Escape => 1,
                    TerminalKey::Up => 103,
                    TerminalKey::Down => 108,
                    TerminalKey::Left => 105,
                    TerminalKey::Right => 106,
                    TerminalKey::Home => 102,
                    TerminalKey::End => 107,
                    TerminalKey::Paste => unreachable!(),
                },
                modifiers,
                false,
            ),
        }
        self.dirty = true;
    }
    fn action(&mut self, action: ui::Action) {
        if !self.active {
            return;
        }
        if self.terminal_mode() && !matches!(action, ui::Action::Hide) {
            let modifiers = self.terminal_controls.take_modifiers();
            self.dirty = true;
            if modifiers.any() {
                match &action {
                    ui::Action::Text(text) => {
                        if let Some(keys) = text
                            .chars()
                            .map(terminal::stroke)
                            .collect::<Option<Vec<_>>>()
                        {
                            for key in keys {
                                self.virtual_chord(key.code, modifiers, key.shift);
                            }
                        } else {
                            self.terminal_controls.unsupported();
                        }
                    }
                    ui::Action::Backspace => self.virtual_chord(14, modifiers, false),
                    ui::Action::Enter => self.virtual_chord(28, modifiers, false),
                    ui::Action::Hide => unreachable!(),
                }
                return;
            }
        }
        let method = self.method.as_ref().unwrap();
        match action {
            ui::Action::Text(text) => {
                method.commit_string(text);
                method.commit(self.serial);
            }
            ui::Action::Backspace => {
                let lengths = self.surrounding.as_ref().and_then(Surrounding::deletion);
                if let Some((before, after)) = lengths {
                    method.delete_surrounding_text(before as u32, after as u32);
                    method.commit(self.serial);
                } else {
                    self.virtual_key(14);
                }
            }
            ui::Action::Enter => self.virtual_key(28),
            ui::Action::Hide => {
                self.hidden = true;
                self.hide_surface();
            }
        }
    }
    fn paint(&mut self) -> Result<()> {
        if !self.configured || !self.dirty || self.frame_pending {
            return Ok(());
        }
        let Some(surface) = self.surface.as_ref() else {
            return Ok(());
        };
        surface.set_buffer_scale(self.scale);
        let (width, height) = (
            self.width * self.scale as u32,
            self.height * self.scale as u32,
        );
        if self.graphics.is_none() {
            self.graphics = Some(graphics::Graphics::new(
                &self.connection,
                surface,
                width,
                height,
            )?);
        }
        self.graphics.as_mut().unwrap().resize(width, height);
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.width as f32, self.height as f32),
            )),
            events: tarot_ui::keyboard::take_pointer_frame(&mut self.events),
            time: Some(self.start.elapsed().as_secs_f64()),
            focused: true,
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.scale as f32);
        let numeric = matches!(self.purpose, 2 | 3 | 4 | 9);
        let secure = self.secure();
        let terminal = self.terminal_mode();
        let mut actions = Vec::new();
        let mut terminal_keys = Vec::new();
        tarot_ui::theme::install(&self.context);
        let output = self.context.run(input, |ctx| {
            if terminal {
                (actions, terminal_keys) = self.ui.show_terminal(ctx, &mut self.terminal_controls);
            } else {
                actions = self.ui.show(ctx, numeric, secure);
            }
        });
        let mut delay = output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map_or(Duration::MAX, |output| output.repaint_delay);
        // Settle measurement and font/scale initialization before idle. Later
        // frames follow egui's deadlines rather than repainting at a fixed rate.
        if self.warmup_frames < 2 {
            self.warmup_frames += 1;
            delay = Duration::ZERO;
        }
        self.repaint = delay == Duration::ZERO;
        self.repaint_at = if delay > Duration::ZERO && delay < Duration::from_secs(60) {
            Instant::now().checked_add(delay)
        } else {
            None
        };
        surface.frame(&self.qh, self.generation);
        self.frame_pending = true;
        self.dirty = !self.events.is_empty();
        self.graphics
            .as_mut()
            .unwrap()
            .paint(&self.context, output)?;
        for key in terminal_keys {
            self.terminal_key(key);
        }
        for action in actions {
            self.action(action);
        }
        Ok(())
    }
    fn done(&mut self) {
        let previous_terminal = self.terminal_mode();
        self.serial = self.serial.wrapping_add(1);
        if let Some(active) = self.pending_active.take() {
            self.hide_surface();
            self.active = active;
            self.hidden = false;
            self.ui.reset();
            self.surrounding = None;
            self.hint = 0;
            self.purpose = 0;
        }
        if let Some((hint, purpose)) = self.pending_type.take() {
            self.hint = hint;
            self.purpose = purpose;
        }
        if previous_terminal != self.terminal_mode() {
            self.hide_surface();
            self.ui.reset();
        }
        if let Some(surrounding) = self.pending_surrounding.take() {
            self.surrounding = Some(surrounding);
        }
        if !self.active || self.secure() {
            self.surrounding = None;
        }
        self.show_surface();
        self.dirty = true;
    }
}

fn main() -> Result<()> {
    ensure!(
        std::env::var_os("WAYLAND_SOCKET").is_some(),
        "launch through Tarot's --input-method option"
    );
    let connection = Connection::connect_to_env()?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    connection.display().get_registry(&qh, ());
    let mut state = Keyboard {
        connection: connection.clone(),
        qh,
        compositor: None,
        layer_shell: None,
        seat: None,
        method_manager: None,
        virtual_manager: None,
        method: None,
        virtual_keyboard: None,
        surface: None,
        layer: None,
        graphics: None,
        context: egui::Context::default(),
        ui: ui::KeyboardUi::default(),
        terminal_controls: TerminalControls::default(),
        terminal_masks: terminal::Masks::default(),
        width: 360,
        height: 240,
        scale: 1,
        configured: false,
        active: false,
        hidden: false,
        serial: 0,
        pending_active: None,
        purpose: 0,
        hint: 0,
        pending_type: None,
        surrounding: None,
        pending_surrounding: None,
        position: egui::Pos2::ZERO,
        contacts: HashSet::new(),
        touch_owner: None,
        touch_cancelled: false,
        events: Vec::new(),
        dirty: false,
        frame_pending: false,
        repaint: false,
        repaint_at: None,
        warmup_frames: 0,
        generation: 0,
        start: Instant::now(),
        closed: false,
    };
    queue.roundtrip(&mut state)?;
    let seat = state.seat.as_ref().context("no seat")?;
    state.method = Some(
        state
            .method_manager
            .as_ref()
            .context("no trusted input-method interface")?
            .get_input_method(seat, &state.qh, ()),
    );
    state.virtual_keyboard = Some(
        state
            .virtual_manager
            .as_ref()
            .context("no trusted virtual-keyboard interface")?
            .create_virtual_keyboard(seat, &state.qh, ()),
    );
    ensure!(
        state.layer_shell.is_some(),
        "no trusted layer-shell interface"
    );
    let context = xkbcommon::xkb::Context::new(xkbcommon::xkb::CONTEXT_NO_FLAGS);
    let keymap = xkbcommon::xkb::Keymap::new_from_names(
        &context,
        "",
        "",
        "us",
        "",
        None,
        xkbcommon::xkb::KEYMAP_COMPILE_NO_FLAGS,
    )
    .context("cannot create virtual keymap")?;
    state.terminal_masks = terminal::Masks::from_keymap(&keymap)?;
    let mut bytes = keymap
        .get_as_string(xkbcommon::xkb::KEYMAP_FORMAT_TEXT_V1)
        .into_bytes();
    bytes.push(0);
    let mut file = tempfile::tempfile()?;
    file.write_all(&bytes)?;
    state
        .virtual_keyboard
        .as_ref()
        .unwrap()
        .keymap(1, file.as_fd(), bytes.len() as u32);
    let mut event_loop: EventLoop<Keyboard> = EventLoop::try_new()?;
    if let Some(raw) = std::env::var_os("TAROT_KEYBOARD_CONTROL_FD") {
        let descriptor: i32 = raw
            .to_str()
            .context("keyboard control descriptor")?
            .parse()?;
        ensure!(descriptor >= 0, "invalid keyboard control descriptor");
        // SAFETY: Tarot transfers ownership of this inherited socket descriptor
        // once at exec, separately from WAYLAND_SOCKET. No other owner is made.
        let control = unsafe { UnixStream::from_raw_fd(descriptor) };
        rustix::io::fcntl_setfd(&control, rustix::io::FdFlags::CLOEXEC)?;
        control.set_nonblocking(true)?;
        event_loop
            .handle()
            .insert_source(
                Generic::new(control, Interest::READ, Mode::Level),
                |_, source, state| {
                    let mut bytes = [0u8; 64];
                    let mut stream: &UnixStream = source;
                    match stream.read(&mut bytes) {
                        Ok(0) => {
                            state.closed = true;
                            return Ok(PostAction::Remove);
                        }
                        Ok(count) => {
                            for command in &bytes[..count] {
                                match command {
                                    b'S' if state.active => {
                                        state.hidden = false;
                                        state.show_surface();
                                        state.dirty = true;
                                    }
                                    b'H' => {
                                        state.hidden = true;
                                        state.hide_surface();
                                    }
                                    b'Q' => state.closed = true,
                                    _ => {}
                                }
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                        Err(error) => return Err(error),
                    }
                    Ok(PostAction::Continue)
                },
            )
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    }
    WaylandSource::new(connection, queue)
        .insert(event_loop.handle())
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    while !state.closed {
        let timeout = state
            .repaint_at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()));
        event_loop.dispatch(timeout, &mut state)?;
        if state
            .repaint_at
            .is_some_and(|deadline| deadline <= Instant::now())
        {
            state.repaint_at = None;
            state.dirty = true;
        }
        state.paint()?;
        state.connection.flush()?;
    }
    state.hide_surface();
    state.connection.flush()?;
    Ok(())
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        self.hide_surface();
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Keyboard {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(4), qh, ()))
                }
                "wl_seat" => state.seat = Some(registry.bind(name, version.min(7), qh, ())),
                "wl_output" => {
                    let _: wl_output::WlOutput = registry.bind(name, version.min(4), qh, ());
                }
                "zwlr_layer_shell_v1" => {
                    state.layer_shell = Some(registry.bind(name, version.min(4), qh, ()))
                }
                "zwp_input_method_manager_v2" => {
                    state.method_manager = Some(registry.bind(name, 1, qh, ()))
                }
                "zwp_virtual_keyboard_manager_v1" => {
                    state.virtual_manager = Some(registry.bind(name, 1, qh, ()))
                }
                _ => {}
            }
        }
    }
}
impl Dispatch<wl_output::WlOutput, ()> for Keyboard {
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
            state.warmup_frames = 0;
            state.dirty = true;
        }
    }
}
impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for Keyboard {
    fn event(
        state: &mut Self,
        layer: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if state.layer.as_ref() != Some(layer) {
            return;
        }
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                layer.ack_configure(serial);
                state.width = width.max(1);
                state.height = height.max(1);
                state.configured = true;
                state.warmup_frames = 0;
                state.dirty = true;
            }
            zwlr_layer_surface_v1::Event::Closed => {
                state.hidden = true;
                state.hide_surface();
            }
            _ => {}
        }
    }
}
impl Dispatch<wl_callback::WlCallback, u64> for Keyboard {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        generation: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if *generation == state.generation {
            state.frame_pending = false;
            state.dirty |= state.repaint;
            state.repaint = false;
        }
    }
}
impl Dispatch<zwp_input_method_v2::ZwpInputMethodV2, ()> for Keyboard {
    fn event(
        state: &mut Self,
        _: &zwp_input_method_v2::ZwpInputMethodV2,
        event: zwp_input_method_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_input_method_v2::Event::Activate => state.pending_active = Some(true),
            zwp_input_method_v2::Event::Deactivate => state.pending_active = Some(false),
            zwp_input_method_v2::Event::ContentType { hint, purpose } => {
                state.pending_type = Some((u32::from(hint), u32::from(purpose)))
            }
            zwp_input_method_v2::Event::SurroundingText {
                text,
                cursor,
                anchor,
            } => {
                state.pending_surrounding = Some(Surrounding {
                    text,
                    cursor: cursor as usize,
                    anchor: anchor as usize,
                })
            }
            zwp_input_method_v2::Event::Done => state.done(),
            zwp_input_method_v2::Event::Unavailable => state.closed = true,
            _ => {}
        }
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for Keyboard {
    fn event(
        _: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(wl_seat::Capability::Touch) {
                seat.get_touch(qh, ());
            }
            if capabilities.contains(wl_seat::Capability::Pointer) {
                seat.get_pointer(qh, ());
            }
        }
    }
}
impl Dispatch<wl_touch::WlTouch, ()> for Keyboard {
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
impl Dispatch<wl_pointer::WlPointer, ()> for Keyboard {
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
delegate_noop!(Keyboard: ignore wl_compositor::WlCompositor);
delegate_noop!(Keyboard: ignore wl_surface::WlSurface);
delegate_noop!(Keyboard: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Keyboard: ignore zwp_input_method_manager_v2::ZwpInputMethodManagerV2);
delegate_noop!(Keyboard: ignore zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1);
delegate_noop!(Keyboard: ignore zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1);

#[cfg(test)]
mod tests {
    use super::Surrounding;

    #[test]
    fn backspace_deletes_whole_utf8_graphemes() {
        for text in ["é", "e\u{301}", "🙂", "👨‍👩‍👧‍👦"] {
            let surrounding = Surrounding {
                text: format!("prefix{text}"),
                cursor: 6 + text.len(),
                anchor: 6 + text.len(),
            };
            assert_eq!(surrounding.deletion(), Some((text.len(), 0)));
        }
    }

    #[test]
    fn selected_text_uses_byte_offsets_on_either_side_of_cursor() {
        let forward = Surrounding {
            text: "abécd".into(),
            cursor: 4,
            anchor: 2,
        };
        assert_eq!(forward.deletion(), Some((2, 0)));
        let backward = Surrounding {
            text: "abécd".into(),
            cursor: 2,
            anchor: 4,
        };
        assert_eq!(backward.deletion(), Some((0, 2)));
        let invalid = Surrounding {
            text: "é".into(),
            cursor: 1,
            anchor: 1,
        };
        assert_eq!(invalid.deletion(), None);
    }
}
