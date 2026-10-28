// Adapted from Smithay smallvil v0.7.0 (MIT).
// Copyright (c) 2017 Victor Berger and Victoria Brekenfeld.
// See third-party/Smithay-MIT.txt for the license.

mod compositor;
pub(crate) mod xdg_shell;

use crate::Compositor;

//
// Wl Seat
//

use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
    set_data_device_focus,
};
use smithay::{delegate_data_device, delegate_output, delegate_seat, delegate_viewporter};

// Cropping/scaling is double-buffered surface state. The normal Smithay
// buffer-commit handler validates it and feeds the shared renderer/input view.
delegate_viewporter!(Compositor);

impl SeatHandler for Compositor {
    type KeyboardFocus = WlSurface;
    type PointerFocus = crate::focus::SurfaceFocus;
    type TouchFocus = crate::focus::SurfaceFocus;

    fn seat_state(&mut self) -> &mut SeatState<Compositor> {
        &mut self.seat_state
    }

    fn allow_virtual_keyboard_input(&mut self, _seat: &Seat<Self>) -> bool {
        !self.protected()
    }

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = if self.protected() {
            None
        } else {
            focused.and_then(|s| dh.get_client(s.id()).ok())
        };
        set_data_device_focus(dh, seat, client);
    }
}

delegate_seat!(Compositor);

//
// Wl Data Device
//

impl SelectionHandler for Compositor {
    type SelectionUserData = ();
    fn allow_selection_access(&mut self) -> bool {
        !self.protected()
    }
}

impl DataDeviceHandler for Compositor {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for Compositor {}
impl ServerDndGrabHandler for Compositor {}

delegate_data_device!(Compositor);

//
// Wl Output & Xdg Output
//

impl OutputHandler for Compositor {}
delegate_output!(Compositor);
