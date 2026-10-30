use crate::Compositor;
use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, ButtonState, Device, DeviceCapability, Event, InputBackend,
        InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, TouchEvent,
    },
    input::{
        keyboard::{FilterResult, KeyboardHandle},
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
        touch::{DownEvent, MotionEvent as TouchMotion, UpEvent},
    },
    reexports::wayland_server::Resource,
    utils::{Logical, Point, SERIAL_COUNTER},
};
use tarot_core::{Command, View, touch::Target};
use xkbcommon::xkb::keysyms;

/// Keys delivered to shell widgets belong to that UI until their release.
/// Retiring a widget must release egui's state and consume the physical tail,
/// even if a native client gains focus before the person lets go.
#[derive(Default)]
pub(crate) struct ShellKeys(std::collections::HashMap<u32, Option<egui::Key>>);

impl ShellKeys {
    fn record(&mut self, code: u32, key: Option<egui::Key>, pressed: bool) {
        if pressed {
            self.0.insert(code, key);
        } else {
            self.0.remove(&code);
        }
    }

    fn retire(
        &mut self,
        events: &mut Vec<egui::Event>,
        intercepted: &mut std::collections::HashSet<u32>,
    ) {
        for (code, key) in self.0.drain() {
            intercepted.insert(code);
            if let Some(key) = key {
                events.push(egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
    }
}

impl Compositor {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        if self.hardware_input(&event) {
            return;
        }
        if self.idle_input(&event) {
            return;
        }
        if self.recovery_visible() && self.process_recovery_pointer_touch(&event) {
            return;
        }
        match event {
            InputEvent::DeviceRemoved { device }
                if device.has_capability(DeviceCapability::Touch) =>
            {
                // libinput device removal need not deliver up for every slot.
                // This single-touchscreen backend must release both protocol
                // grabs and routing IDs before a replacement device can act.
                self.cancel_input_sequences();
                self.touches.reset();
            }
            InputEvent::Keyboard { event } => {
                let pressed = event.state() == KeyState::Pressed;
                let raw_code = event.key_code().raw();
                let Some(keyboard) = self.seat.get_keyboard() else {
                    return;
                };
                let serial = SERIAL_COUNTER.next_serial();
                let key_focus = keyboard.current_focus();
                let action = keyboard.input::<Option<Command>, _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    event.time_msec(),
                    |state, modifiers, key| {
                        let symbol = key.modified_sym().raw();
                        if !pressed && state.intercepted_keys.remove(&raw_code) {
                            return FilterResult::Intercept(None);
                        }
                        if pressed && state.intercepted_keys.contains(&raw_code) {
                            return FilterResult::Intercept(None);
                        }
                        if state.protected() {
                            if state.recovery_visible() {
                                if pressed {
                                    state.intercepted_keys.insert(raw_code);
                                    if state.recovery_ready()
                                        && matches!(
                                            symbol,
                                            keysyms::KEY_Return
                                                | keysyms::KEY_KP_Enter
                                                | keysyms::KEY_space
                                        )
                                    {
                                        state.protection.recovery_retry_key = true;
                                    }
                                }
                                return FilterResult::Intercept(None);
                            }
                            return if state.active_lock_surface().is_some() {
                                FilterResult::Forward
                            } else {
                                FilterResult::Intercept(None)
                            };
                        }
                        // Tab transfers selection to egui's actual focused widget,
                        // including submenu Back buttons and detail-page controls.
                        if pressed
                            && matches!(symbol, keysyms::KEY_Tab | keysyms::KEY_ISO_Left_Tab)
                            && state.shell.system_menu_open
                        {
                            state.shell.menus.selected = None;
                        }
                        let menu_rows = state.shell.system_menu_open
                            && matches!(
                                state.shell.system_page,
                                tarot_core::SystemPage::Device | tarot_core::SystemPage::Activity
                            );
                        let command = match symbol {
                            keysyms::KEY_Escape if state.shell.network_input.is_some() => {
                                Some(Command::CredentialCancel)
                            }
                            keysyms::KEY_BackSpace if state.shell.network_input.is_some() => {
                                Some(Command::CredentialBackspace)
                            }
                            keysyms::KEY_Return | keysyms::KEY_KP_Enter
                                if state.shell.network_input.is_some() =>
                            {
                                Some(Command::CredentialSubmit)
                            }
                            keysyms::KEY_Escape if state.shell.system_menu_open => {
                                Some(Command::Menu(tarot_core::menu::Action::Back))
                            }
                            keysyms::KEY_Up | keysyms::KEY_Down if menu_rows => {
                                Some(Command::Menu(tarot_core::menu::Action::Move(
                                    if symbol == keysyms::KEY_Up { -1 } else { 1 },
                                )))
                            }
                            keysyms::KEY_Return | keysyms::KEY_KP_Enter
                                if menu_rows && state.shell.menus.selected.is_some() =>
                            {
                                Some(Command::Menu(tarot_core::menu::Action::Submit))
                            }
                            keysyms::KEY_Escape if state.shell.notifications_open => {
                                Some(Command::NotificationsOpen(false))
                            }
                            keysyms::KEY_Escape if state.gestures.is_arranging() => {
                                Some(Command::CancelGesture)
                            }
                            keysyms::KEY_F1 => Some(Command::Overview),
                            keysyms::KEY_F2 => Some(Command::Launcher),
                            keysyms::KEY_Left
                                if state.shell.view == View::Overview
                                    && !state.shell.system_menu_open =>
                            {
                                Some(Command::BrowseCards(-1))
                            }
                            keysyms::KEY_Right
                                if state.shell.view == View::Overview
                                    && !state.shell.system_menu_open =>
                            {
                                Some(Command::BrowseCards(1))
                            }
                            keysyms::KEY_Up | keysyms::KEY_Down
                                if state.shell.just_type_active()
                                    && !state.shell.system_menu_open
                                    && !state.shell.launcher.menu_open =>
                            {
                                Some(Command::JustType(tarot_core::search::Action::Move(
                                    if symbol == keysyms::KEY_Up { -1 } else { 1 },
                                )))
                            }
                            keysyms::KEY_Return | keysyms::KEY_KP_Enter
                                if state.shell.just_type_active()
                                    && !state.shell.system_menu_open
                                    && !state.shell.launcher.menu_open =>
                            {
                                Some(Command::JustType(tarot_core::search::Action::Submit))
                            }
                            keysyms::KEY_Escape
                                if state.shell.view == View::Launcher
                                    && (state.shell.launcher.menu_open
                                        || state.shell.launcher.search_open
                                        || state.shell.launcher.rename.is_some()) =>
                            {
                                Some(Command::LauncherAction(
                                    tarot_core::launcher::Action::Dismiss,
                                ))
                            }
                            keysyms::KEY_Escape if !matches!(state.shell.view, View::App(_)) => {
                                Some(Command::Home)
                            }
                            _ => None,
                        };
                        let command = command.or_else(|| {
                            (state.shell.network_input.is_some()
                                && !modifiers.ctrl
                                && !modifiers.alt)
                                .then(|| {
                                    key.modified_sym().key_char().filter(|ch| !ch.is_control())
                                })
                                .flatten()
                                .map(|ch| {
                                    Command::CredentialText(tarot_core::network::Secret::new(
                                        ch.to_string(),
                                    ))
                                })
                        });
                        if pressed && command.is_some() {
                            state.intercepted_keys.insert(raw_code);
                            return FilterResult::Intercept(command);
                        }
                        if state.shell.menu_visual().closing {
                            if pressed {
                                state.intercepted_keys.insert(raw_code);
                            }
                            return FilterResult::Intercept(None);
                        }
                        if !matches!(state.shell.view, View::App(_)) || state.shell.system_menu_open
                        {
                            state.shell_keys.record(raw_code, egui_key(symbol), pressed);
                            let modifiers = egui::Modifiers {
                                alt: modifiers.alt,
                                ctrl: modifiers.ctrl,
                                shift: modifiers.shift,
                                mac_cmd: false,
                                command: modifiers.ctrl,
                            };
                            if let Some(key) = egui_key(symbol) {
                                state.ui_events.push(egui::Event::Key {
                                    key,
                                    physical_key: None,
                                    pressed,
                                    repeat: false,
                                    modifiers,
                                });
                            }
                            if pressed
                                && !modifiers.ctrl
                                && !modifiers.alt
                                && let Some(character) =
                                    key.modified_sym().key_char().filter(|ch| !ch.is_control())
                            {
                                state
                                    .ui_events
                                    .push(egui::Event::Text(character.to_string()));
                            }
                            state.ui_dirty = true;
                            return FilterResult::Intercept(None);
                        }
                        if pressed && let Some(surface) = &key_focus {
                            state.remember_popup_input(serial, surface);
                        }
                        FilterResult::Forward
                    },
                );
                if std::mem::take(&mut self.protection.recovery_retry_key) {
                    self.retry_recovery();
                }
                if let Some(Some(command)) = action {
                    self.apply_command(command);
                }
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let pos = self.oriented_position(event.position_transformed((1, 1).into()));
                let Some(pointer) = self.seat.get_pointer() else {
                    return;
                };
                if !self.suppressed_pointer_buttons.is_empty() {
                    pointer.motion(
                        self,
                        None,
                        &MotionEvent {
                            location: pos,
                            serial: SERIAL_COUNTER.next_serial(),
                            time: event.time_msec(),
                        },
                    );
                    pointer.frame(self);
                    return;
                }
                let shell = self
                    .pointer_shell_owned
                    .unwrap_or_else(|| self.shell_owns(pos));
                if shell {
                    self.shell_pointer_motion(pos);
                    self.gestures.update(
                        -2,
                        pos.x as f32,
                        pos.y as f32,
                        u64::from(event.time_msec()),
                    );
                } else {
                    self.leave_shell_pointer();
                }
                let under = if shell { None } else { self.surface_under(pos) };
                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: pos,
                        serial: SERIAL_COUNTER.next_serial(),
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerButton { event } => {
                let code = event.button_code();
                let pressed = event.state() == ButtonState::Pressed;
                if pressed {
                    self.pointer_buttons.insert(code);
                } else {
                    self.pointer_buttons.remove(&code);
                }
                if self.suppressed_pointer_buttons.contains(&code) {
                    if !pressed {
                        self.suppressed_pointer_buttons.remove(&code);
                    }
                    return;
                }
                let Some(pointer) = self.seat.get_pointer() else {
                    return;
                };
                let position = pointer.current_location();
                let pressed = event.state() == ButtonState::Pressed;
                if pressed
                    && !self.popup_allows_surface(
                        self.surface_under(position)
                            .as_ref()
                            .map(|(focus, _)| focus),
                    )
                {
                    self.dismiss_popup_grab();
                }
                let shell = self
                    .pointer_shell_owned
                    .unwrap_or_else(|| self.shell_owns(position));
                if pressed && self.pointer_shell_owned.is_none() {
                    self.pointer_shell_owned = Some(shell);
                    if shell && event.button_code() == 0x110 {
                        self.begin_shell_gesture(-2, position, event.time_msec());
                    }
                }
                if shell {
                    self.shell_pointer_motion(position);
                    let consumed = !pressed
                        && event.button_code() == 0x110
                        && self.finish_shell_gesture(-2, event.time_msec());
                    if !consumed && let Some(button) = pointer_button(event.button_code()) {
                        self.ui_events.push(egui::Event::PointerButton {
                            pos: egui::pos2(position.x as f32, position.y as f32),
                            button,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        });
                    }
                } else {
                    self.leave_shell_pointer();
                    let serial = SERIAL_COUNTER.next_serial();
                    if pressed && let Some(focus) = pointer.current_focus() {
                        self.remember_popup_input(serial, &focus.surface);
                    }
                    pointer.button(
                        self,
                        &ButtonEvent {
                            button: event.button_code(),
                            state: event.state(),
                            serial,
                            time: event.time_msec(),
                        },
                    );
                    pointer.frame(self);
                }
                if !pressed {
                    self.pointer_shell_owned = None;
                }
            }
            InputEvent::PointerAxis { event } => {
                let Some(pointer) = self.seat.get_pointer() else {
                    return;
                };
                let horizontal = event
                    .amount(Axis::Horizontal)
                    .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) / 8.0);
                let vertical = event
                    .amount(Axis::Vertical)
                    .unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) / 8.0);
                if self.shell_owns(pointer.current_location()) {
                    self.ui_events.push(egui::Event::MouseWheel {
                        unit: egui::MouseWheelUnit::Point,
                        // Linux/Wayland positive axes move toward the bottom
                        // or right. egui describes the content's movement.
                        delta: egui::vec2(-horizontal as f32, -vertical as f32),
                        modifiers: egui::Modifiers::NONE,
                    });
                    self.ui_dirty = true;
                } else {
                    self.leave_shell_pointer();
                    let frame = AxisFrame::new(event.time_msec())
                        .source(event.source())
                        .value(Axis::Horizontal, horizontal)
                        .value(Axis::Vertical, vertical);
                    pointer.axis(self, frame);
                    pointer.frame(self);
                }
            }
            InputEvent::TouchDown { event } => {
                let slot = event.slot();
                let pos = self.oriented_position(event.position_transformed((1, 1).into()));
                let shell = self.shell_owns(pos);
                let focus = self.surface_under(pos);
                if !self.popup_allows_surface(focus.as_ref().map(|(focus, _)| focus)) {
                    self.dismiss_popup_grab();
                    self.cancel_input_sequences();
                    if !shell {
                        self.touches.begin(i32::from(slot), true);
                        self.touches.cancel();
                        return;
                    }
                }
                // Coordinates on the native keyboard can reveal typed text.
                tracing::debug!(?slot, shell, "touch sequence started");
                let route = self.touches.begin(i32::from(slot), shell);
                if route.cancel_shell {
                    self.cancel_shell_pointer();
                }
                if route.target == Target::Shell {
                    self.begin_shell_gesture(i32::from(slot), pos, event.time_msec());
                    self.shell_pointer_motion(pos);
                    self.ui_events.push(egui::Event::PointerButton {
                        pos: egui::pos2(pos.x as f32, pos.y as f32),
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    });
                } else if route.target == Target::Application
                    && let Some(touch) = self.seat.get_touch()
                {
                    self.native_contacts.insert(slot, (pos, event.time_msec()));
                    let serial = SERIAL_COUNTER.next_serial();
                    if !self.protected() {
                        tracing::debug!(target: "tarot::input::metadata",
                            slot = i32::from(slot), input_time_ms = event.time_msec(),
                            surface = focus.as_ref().map_or(0, |(focus, _)| focus.surface.id().protocol_id()),
                            keyboard = focus.as_ref().is_some_and(|(focus, _)| self.keyboard.as_ref().is_some_and(|k| k.layer.wl_surface() == &focus.surface)),
                            "native touch down");
                    }
                    if let Some((focus, _)) = &focus {
                        self.remember_popup_input(serial, &focus.surface);
                    }
                    touch.down(
                        self,
                        focus,
                        &DownEvent {
                            slot,
                            location: pos,
                            serial,
                            time: event.time_msec(),
                        },
                    );
                    touch.frame(self);
                }
            }
            InputEvent::TouchMotion { event } => {
                let slot = event.slot();
                let pos = self.oriented_position(event.position_transformed((1, 1).into()));
                if self.touches.target(i32::from(slot)) == Target::Shell {
                    self.gestures.update(
                        i32::from(slot),
                        pos.x as f32,
                        pos.y as f32,
                        u64::from(event.time_msec()),
                    );
                    self.shell_pointer_motion(pos);
                } else if self.touches.target(i32::from(slot)) == Target::Application
                    && let Some(touch) = self.seat.get_touch()
                {
                    self.native_contacts.insert(slot, (pos, event.time_msec()));
                    touch.motion(
                        self,
                        self.surface_under(pos),
                        &TouchMotion {
                            slot,
                            location: pos,
                            time: event.time_msec(),
                        },
                    );
                    touch.frame(self);
                }
            }
            InputEvent::TouchUp { event } => {
                let slot = event.slot();
                self.native_contacts.remove(&slot);
                tracing::debug!(?slot, "touch contact released");
                match self.touches.end(i32::from(slot)) {
                    Target::Shell => {
                        if !self.finish_shell_gesture(i32::from(slot), event.time_msec()) {
                            release_touch_pointer(&mut self.ui_events, self.shell_pointer_position);
                            self.shell_pointer_present = false;
                        }
                        self.ui_dirty = true;
                    }
                    Target::Application => {
                        if let Some(touch) = self.seat.get_touch() {
                            if !self.protected() {
                                tracing::debug!(target: "tarot::input::metadata",
                                    slot = i32::from(slot), input_time_ms = event.time_msec(),
                                    "native touch up");
                            }
                            touch.up(
                                self,
                                &UpEvent {
                                    slot,
                                    serial: SERIAL_COUNTER.next_serial(),
                                    time: event.time_msec(),
                                },
                            );
                            touch.frame(self);
                        }
                    }
                    Target::Suppressed => {}
                }
            }
            InputEvent::TouchCancel { .. } => {
                self.cancel_input_sequences();
                self.touches.reset();
            }
            _ => {}
        }
    }

    pub fn cancel_input_sequences(&mut self) {
        self.shell_keys
            .retire(&mut self.ui_events, &mut self.intercepted_keys);
        self.hardware.cancel();
        if self.protected() {
            tarot_ui::cancel_pointer_input(&mut self.protection.recovery_events);
        }
        self.dismiss_popup_grab();
        if self.touches.has_shell() || self.pointer_shell_owned == Some(true) {
            self.cancel_shell_pointer();
        }
        self.cancel_native_touch_sequence();
        self.gestures.cancel();
        self.gesture_started = None;
        self.animation.end_drag();
        self.redraw = true;
        self.pointer_shell_owned = None;
    }

    /// Retire held keys before a shell surface or protection owns input.
    /// Smithay seeds the next client's enter event from forwarded keys, which
    /// an intercepted physical release would otherwise leave remembered.
    pub(crate) fn retire_keyboard_input(&mut self, keyboard: &KeyboardHandle<Self>) {
        self.shell_keys
            .retire(&mut self.ui_events, &mut self.intercepted_keys);
        keyboard.unset_grab(self);
        keyboard.set_focus(self, None, SERIAL_COUNTER.next_serial());
        for code in keyboard.pressed_keys() {
            self.intercepted_keys.insert(code.raw());
            // No client is focused: update seat/forwarded state without sending
            // a synthetic release to an old, replacement or protected client.
            keyboard.input::<(), _>(
                self,
                code,
                KeyState::Released,
                SERIAL_COUNTER.next_serial(),
                0,
                |_, _, _| FilterResult::Forward,
            );
        }
        self.ui_dirty = true;
        self.redraw = true;
    }

    pub fn cancel_native_touch_sequence(&mut self) {
        if self.touches.has_application()
            && let Some(touch) = self.seat.get_touch()
        {
            // Smithay 0.7's TouchInternal::cancel skips contacts whose pending
            // frame was already sent. Requeue their last position, without a
            // frame, so cancel both notifies the client and removes every focus.
            // A client may see a duplicate final motion immediately before cancel.
            for (slot, (location, time)) in std::mem::take(&mut self.native_contacts) {
                touch.motion(
                    self,
                    None,
                    &TouchMotion {
                        slot,
                        location,
                        time,
                    },
                );
            }
            touch.cancel(self);
        }
        self.native_contacts.clear();
        self.touches.cancel();
    }

    fn cancel_shell_pointer(&mut self) {
        self.advance_presentation();
        tarot_ui::cancel_pointer_input(&mut self.ui_events);
        self.shell_pointer_present = false;
        self.gestures.cancel();
        self.gesture_started = None;
        self.animation.end_drag();
        self.ui_dirty = true;
        self.redraw = true;
    }

    fn begin_shell_gesture(&mut self, id: i32, pos: Point<f64, Logical>, time_ms: u32) {
        self.gesture_started = Some(std::time::Instant::now());
        self.animation.begin_drag(self.start_time.elapsed());
        self.gestures.begin(
            tarot_core::gesture::Start {
                id,
                position: (pos.x as f32, pos.y as f32),
                time_ms: u64::from(time_ms),
            },
            self.layout,
            &self.shell,
            &self.presentation,
        );
    }

    fn finish_shell_gesture(&mut self, id: i32, time_ms: u32) -> bool {
        let previous_target = self.presentation.arrange.as_ref().and_then(|a| a.target);
        // Preserve the last drag position even if down/motion/up arrive before a paint.
        self.advance_presentation();
        let outcome = self.gestures.finish(id, u64::from(time_ms), &self.shell);
        if !self.protected()
            && let Some(Command::PlaceCard { card, target }) = &outcome.command
        {
            tracing::debug!(target: "tarot::input::metadata",
                card = card.0, input_time_ms = time_ms,
                previous_target = ?previous_target, drop_target = ?target,
                "card drop");
        }
        self.gesture_started = None;
        self.animation.end_drag();
        if outcome.consumed {
            tarot_ui::cancel_pointer_input(&mut self.ui_events);
            self.shell_pointer_present = false;
        }
        if let Some(command) = outcome.command {
            self.apply_command(command);
        }
        self.ui_dirty = true;
        self.redraw = true;
        outcome.consumed
    }

    fn shell_owns(&self, position: Point<f64, Logical>) -> bool {
        if self.protected() {
            return false;
        }
        if self.shell.launch_feedback.is_some()
            && self.shell.view != View::Launcher
            && self
                .layout
                .launch_feedback()
                .contains(position.x as f32, position.y as f32)
        {
            return true;
        }
        if self.shell.volume_feedback.is_some()
            && self
                .layout
                .volume_feedback()
                .contains(position.x as f32, position.y as f32)
        {
            return true;
        }
        if tarot_core::notifications::overlay_contains(
            &self.shell,
            self.layout,
            position.x as f32,
            position.y as f32,
        ) {
            return true;
        }
        if self.keyboard_surface_under(position).is_some() {
            return false;
        }
        !matches!(self.shell.view, View::App(_))
            || !self
                .active_input_area()
                .contains(position.x as f32, position.y as f32)
    }

    fn shell_pointer_motion(&mut self, position: Point<f64, Logical>) {
        self.shell_pointer_present = true;
        self.shell_pointer_position = egui::pos2(position.x as f32, position.y as f32);
        self.ui_events
            .push(egui::Event::PointerMoved(self.shell_pointer_position));
        self.ui_dirty = true;
    }

    fn leave_shell_pointer(&mut self) {
        // The native route owns this motion. Retire the old shell hover once
        // so tooltips and hover styles cannot remain over client content.
        // An active shell press/touch retains its egui pointer sequence.
        if self.shell_pointer_present
            && self.pointer_shell_owned != Some(true)
            && !self.touches.has_shell()
        {
            self.shell_pointer_present = false;
            self.ui_events.push(egui::Event::PointerGone);
            self.ui_dirty = true;
        }
    }
}

fn release_touch_pointer(events: &mut Vec<egui::Event>, pos: egui::Pos2) {
    events.push(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    // A lifted finger has no hover position. Keeping its previous position
    // makes the next touch-down's jump look like a drag delta to ScrollArea.
    events.push(egui::Event::PointerGone);
}

fn pointer_button(code: u32) -> Option<egui::PointerButton> {
    match code {
        0x110 => Some(egui::PointerButton::Primary),
        0x111 => Some(egui::PointerButton::Secondary),
        0x112 => Some(egui::PointerButton::Middle),
        _ => None,
    }
}

fn egui_key(symbol: u32) -> Option<egui::Key> {
    Some(match symbol {
        keysyms::KEY_Return | keysyms::KEY_KP_Enter => egui::Key::Enter,
        keysyms::KEY_Escape => egui::Key::Escape,
        keysyms::KEY_Tab | keysyms::KEY_ISO_Left_Tab => egui::Key::Tab,
        keysyms::KEY_BackSpace => egui::Key::Backspace,
        keysyms::KEY_Delete => egui::Key::Delete,
        keysyms::KEY_Left => egui::Key::ArrowLeft,
        keysyms::KEY_Right => egui::Key::ArrowRight,
        keysyms::KEY_Up => egui::Key::ArrowUp,
        keysyms::KEY_Down => egui::Key::ArrowDown,
        keysyms::KEY_space => egui::Key::Space,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_touch_drags_accumulate_without_a_reverse_jump_on_the_next_press() {
        let ctx = egui::Context::default();
        let mut time = 0.0;
        let mut draw = |events| {
            time += 0.055;
            let mut offset = 0.0;
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(time),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(720.0, 360.0),
                    )),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let bounds = egui::Rect::from_min_max(
                            egui::pos2(0.0, 84.0),
                            egui::pos2(720.0, 140.0),
                        );
                        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
                        child.set_clip_rect(bounds);
                        offset = egui::ScrollArea::vertical()
                            .id_salt("touch_results")
                            .auto_shrink([false, false])
                            .max_height(bounds.height())
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                            )
                            .show(&mut child, |ui| {
                                ui.set_min_size(egui::vec2(720.0, 240.0));
                            })
                            .state
                            .offset
                            .y;
                    });
                },
            );
            offset
        };
        for _ in 0..4 {
            draw(Vec::new());
        }
        let start = egui::pos2(700.0, 132.0);
        let end = egui::pos2(700.0, 92.0);
        for attempt in 0..4 {
            let at_press = draw(vec![
                egui::Event::PointerMoved(start),
                egui::Event::PointerButton {
                    pos: start,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ]);
            assert!((at_press - attempt as f32 * 40.0).abs() < 0.1);
            for step in 1..=4 {
                draw(vec![egui::Event::PointerMoved(egui::pos2(
                    start.x,
                    start.y - step as f32 * 10.0,
                ))]);
            }
            let mut events = Vec::new();
            release_touch_pointer(&mut events, end);
            let after_release = draw(events);
            assert!((after_release - (attempt + 1) as f32 * 40.0).abs() < 0.1);
            assert!(!ctx.input(|i| i.pointer.any_down()));
            assert!(ctx.input(|i| i.pointer.hover_pos()).is_none());
        }
    }

    #[test]
    fn retiring_shell_widgets_releases_egui_and_keeps_physical_tails_out_of_clients() {
        let ctx = egui::Context::default();
        let mut keys = ShellKeys::default();
        let mut events = vec![egui::Event::Key {
            key: egui::Key::Space,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }];
        let mut intercepted = std::collections::HashSet::new();
        let frame = |events: Vec<egui::Event>| {
            let _ = ctx.run(
                egui::RawInput {
                    events,
                    focused: true,
                    ..Default::default()
                },
                |_| {},
            );
        };
        keys.record(65, Some(egui::Key::Space), true);
        keys.record(38, None, true); // Text-only keys also own their release.
        keys.record(23, Some(egui::Key::Tab), true);
        keys.record(23, Some(egui::Key::Tab), false);
        frame(std::mem::take(&mut events));
        assert!(ctx.input(|i| i.key_down(egui::Key::Space)));
        keys.retire(&mut events, &mut intercepted);
        assert_eq!(intercepted.len(), 2);
        assert!(intercepted.contains(&65) && intercepted.contains(&38));
        frame(std::mem::take(&mut events));
        assert!(!ctx.input(|i| i.key_down(egui::Key::Space)));
        keys.retire(&mut events, &mut intercepted);
        assert!(events.is_empty(), "retirement is idempotent");
        assert!(intercepted.remove(&65));
        assert!(intercepted.remove(&38));
        assert!(intercepted.is_empty());
        assert_eq!(egui_key(keysyms::KEY_ISO_Left_Tab), Some(egui::Key::Tab));
        assert_eq!(egui_key(keysyms::KEY_KP_Enter), Some(egui::Key::Enter));
    }
}
