use crate::Compositor;
use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, ButtonState, Device, DeviceCapability, Event, InputBackend,
        InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, TouchEvent,
    },
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
        touch::{DownEvent, MotionEvent as TouchMotion, UpEvent},
    },
    utils::{Logical, Point, SERIAL_COUNTER},
};
use tarot_core::{Command, View, touch::Target};
use xkbcommon::xkb::keysyms;

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
                                Some(Command::SystemMenuOpen(false))
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
                                    && !state.shell.notifications_open
                                    && !state.shell.system_menu_open =>
                            {
                                Some(Command::BrowseCards(-1))
                            }
                            keysyms::KEY_Right
                                if state.shell.view == View::Overview
                                    && !state.shell.notifications_open
                                    && !state.shell.system_menu_open =>
                            {
                                Some(Command::BrowseCards(1))
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
                        if !matches!(state.shell.view, View::App(_))
                            || state.shell.notifications_open
                            || state.shell.system_menu_open
                        {
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
                        delta: egui::vec2(horizontal as f32, vertical as f32),
                        modifiers: egui::Modifiers::NONE,
                    });
                    self.ui_dirty = true;
                } else {
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
                            self.ui_events.push(egui::Event::PointerButton {
                                pos: self.shell_pointer_position,
                                button: egui::PointerButton::Primary,
                                pressed: false,
                                modifiers: egui::Modifiers::NONE,
                            });
                        }
                        self.ui_dirty = true;
                    }
                    Target::Application => {
                        if let Some(touch) = self.seat.get_touch() {
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
        // Preserve the last drag position even if down/motion/up arrive before a paint.
        self.advance_presentation();
        let outcome = self.gestures.finish(id, u64::from(time_ms), &self.shell);
        self.gesture_started = None;
        self.animation.end_drag();
        if outcome.consumed {
            tarot_ui::cancel_pointer_input(&mut self.ui_events);
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
        self.shell_pointer_position = egui::pos2(position.x as f32, position.y as f32);
        self.ui_events
            .push(egui::Event::PointerMoved(self.shell_pointer_position));
        self.ui_dirty = true;
    }
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
        keysyms::KEY_Return => egui::Key::Enter,
        keysyms::KEY_Escape => egui::Key::Escape,
        keysyms::KEY_Tab => egui::Key::Tab,
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
