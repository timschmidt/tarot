//! Popup ownership and grab lifetime. Protocol hierarchy remains with Smithay.

use crate::{Compositor, focus::SurfaceFocus};
use smithay::{
    desktop::{
        PopupKeyboardGrab, PopupKind, PopupPointerGrab, PopupUngrabStrategy,
        find_popup_root_surface,
    },
    input::{Seat, pointer::Focus},
    reexports::wayland_server::protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
    utils::{SERIAL_COUNTER, Serial},
    wayland::shell::xdg::PopupSurface,
};
use std::time::Duration;
use tarot_core::{View, WindowId};

pub struct UserInput {
    serial: Serial,
    owner: WindowId,
    when: Duration,
}

impl Compositor {
    fn root_for_window(&self, id: WindowId) -> Option<WlSurface> {
        self.space
            .elements()
            .find(|window| window.user_data().get::<WindowId>() == Some(&id))
            .and_then(|window| window.toplevel())
            .map(|top| top.wl_surface().clone())
    }

    fn owner_of_surface(&self, surface: &WlSurface) -> Option<WindowId> {
        if let Some(placement) = self.presented_surfaces.get(surface) {
            return Some(placement.window);
        }
        let root = self
            .popups
            .find_popup(surface)
            .and_then(|popup| find_popup_root_surface(&popup).ok())
            .unwrap_or_else(|| surface.clone());
        self.space
            .elements()
            .find(|window| {
                window
                    .toplevel()
                    .is_some_and(|top| top.wl_surface() == &root)
            })
            .and_then(|window| window.user_data().get::<WindowId>().copied())
    }

    pub fn remember_popup_input(&mut self, serial: Serial, surface: &WlSurface) {
        if self.protected() {
            return;
        }
        let Some(owner) = self.owner_of_surface(surface) else {
            return;
        };
        if self.shell.view != View::App(owner) {
            return;
        }
        self.popup_input.push_back(UserInput {
            serial,
            owner,
            when: self.start_time.elapsed(),
        });
        while self.popup_input.len() > 64 {
            self.popup_input.pop_front();
        }
    }

    pub fn popup_allows_surface(&self, surface: Option<&SurfaceFocus>) -> bool {
        self.popup_owner.is_none()
            || surface.and_then(|surface| self.owner_of_surface(&surface.surface))
                == self.popup_owner
    }

    pub fn start_popup_grab(
        &mut self,
        surface: PopupSurface,
        seat_resource: WlSeat,
        serial: Serial,
    ) {
        if self.protected() {
            surface.send_popup_done();
            return;
        }
        let kind = PopupKind::Xdg(surface.clone());
        let root = find_popup_root_surface(&kind).ok();
        let owner = root.as_ref().and_then(|root| self.owner_of_surface(root));
        let seat = Seat::<Self>::from_resource(&seat_resource);
        let now = self.start_time.elapsed();
        let recent_input = owner.is_some_and(|owner| {
            self.popup_input.iter().any(|input| {
                input.serial == serial
                    && input.owner == owner
                    && now.saturating_sub(input.when) <= Duration::from_secs(5)
            })
        });
        // A nested menu may reuse the initiating action while its parent grab is active.
        let nested_input = owner == self.popup_owner
            && self
                .popup_grab
                .as_ref()
                .is_some_and(|grab| !grab.has_ended() && grab.serial() == serial);
        let valid_owner = owner.is_some_and(|owner| self.shell.view == View::App(owner));
        let valid_seat = seat.as_ref().is_some_and(|seat| seat == &self.seat);
        let own_grab = owner == self.popup_owner && self.popup_grab.is_some();
        let other_keyboard_grab = self
            .seat
            .get_keyboard()
            .is_some_and(|keyboard| keyboard.is_grabbed())
            && !own_grab;
        let other_pointer_grab = self
            .seat
            .get_pointer()
            .is_some_and(|pointer| pointer.is_grabbed() && !pointer.has_grab(serial))
            && !own_grab;
        if !valid_owner
            || !valid_seat
            || !(recent_input || nested_input)
            || other_keyboard_grab
            || other_pointer_grab
        {
            tracing::debug!(?serial, ?owner, "popup grab denied");
            surface.send_popup_done();
            return;
        }
        let root = root.expect("validated popup root");
        let seat = seat.expect("validated popup seat");
        match self.popups.grab_popup::<Self>(root, kind, &seat, serial) {
            Ok(grab) => {
                // The old pointer grab's unset callback restores the keyboard
                // root if its keyboard grab is still installed. Release the
                // owned keyboard grab first to avoid a transient root focus.
                if let Some(previous) = self.popup_grab.as_ref().cloned() {
                    self.release_popup_grabs(&previous);
                }
                if let Some(pointer) = seat.get_pointer() {
                    pointer.set_grab(self, PopupPointerGrab::new(&grab), serial, Focus::Keep);
                }
                if let Some(keyboard) = seat.get_keyboard() {
                    keyboard.set_grab(self, PopupKeyboardGrab::new(&grab), serial);
                    keyboard.set_focus(self, grab.current_grab(), serial);
                }
                self.popup_owner = owner;
                self.popup_grab = Some(grab);
                self.redraw = true;
            }
            Err(error) => {
                tracing::debug!(%error, "popup grab rejected by protocol hierarchy");
            }
        }
    }

    pub fn dismiss_popup_grab(&mut self) {
        let Some(mut grab) = self.popup_grab.take() else {
            return;
        };
        self.popup_owner = None;
        grab.ungrab(PopupUngrabStrategy::All);
        self.release_popup_grabs(&grab);
        self.restore_app_keyboard_focus();
        self.redraw = true;
    }

    fn release_popup_grabs(&mut self, grab: &smithay::desktop::PopupGrab<Self>) {
        if let Some(keyboard) = self.seat.get_keyboard()
            && (keyboard.has_grab(grab.serial())
                || grab
                    .previous_serial()
                    .is_some_and(|serial| keyboard.has_grab(serial)))
        {
            keyboard.unset_grab(self);
        }
        if let Some(pointer) = self.seat.get_pointer()
            && (pointer.has_grab(grab.serial())
                || grab
                    .previous_serial()
                    .is_some_and(|serial| pointer.has_grab(serial)))
        {
            let serial = SERIAL_COUNTER.next_serial();
            pointer.unset_grab(self, serial, self.start_time.elapsed().as_millis() as u32);
        }
    }

    fn restore_app_keyboard_focus(&mut self) {
        if self.protected() {
            self.refresh_lock_focus();
            return;
        }
        let target = match self.shell.view {
            View::App(id) => self.root_for_window(id),
            _ => None,
        };
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, target, SERIAL_COUNTER.next_serial());
        }
    }

    pub fn refresh_popup_grab(&mut self) {
        if self.protected() {
            self.dismiss_popup_grab();
            return;
        }
        self.popups.cleanup();
        let Some(grab) = self.popup_grab.as_ref().cloned() else {
            return;
        };
        if self
            .popup_owner
            .is_none_or(|owner| self.shell.view != View::App(owner))
        {
            self.dismiss_popup_grab();
        } else if grab.has_ended() {
            self.popup_grab = None;
            self.popup_owner = None;
            self.release_popup_grabs(&grab);
            self.restore_app_keyboard_focus();
        } else if let Some(keyboard) = self.seat.get_keyboard()
            && keyboard.current_focus() != grab.current_grab()
        {
            keyboard.set_focus(self, grab.current_grab(), SERIAL_COUNTER.next_serial());
        }
    }

    pub fn refresh_pointer_focus(&mut self) {
        if !self.suppressed_pointer_buttons.is_empty() {
            return;
        }
        let Some(pointer) = self.seat.get_pointer() else {
            return;
        };
        if pointer.is_grabbed() && self.popup_grab.is_none() {
            return;
        }
        let location = pointer.current_location();
        let target = self.surface_under(location);
        if pointer.current_focus().as_ref() != target.as_ref().map(|(surface, _)| surface) {
            pointer.motion(
                self,
                target,
                &smithay::input::pointer::MotionEvent {
                    location,
                    serial: SERIAL_COUNTER.next_serial(),
                    time: self.start_time.elapsed().as_millis() as u32,
                },
            );
            pointer.frame(self);
        }
    }
}
