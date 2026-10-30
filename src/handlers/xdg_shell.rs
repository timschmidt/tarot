// Adapted from Smithay smallvil v0.7.0 (MIT).
// Copyright (c) 2017 Victor Berger and Victoria Brekenfeld.
// See third-party/Smithay-MIT.txt for the license.

use smithay::{
    delegate_xdg_shell,
    desktop::{
        PopupKind, PopupManager, Space, Window, find_popup_root_surface, get_popup_toplevel_coords,
    },
    input::{
        Seat,
        pointer::{Focus, GrabStartData as PointerGrabStartData},
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            Resource,
            protocol::{wl_seat, wl_surface::WlSurface},
        },
    },
    utils::{Rectangle, Serial},
    wayland::{
        compositor::with_states,
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceData,
        },
    },
};

use crate::{
    Compositor,
    grabs::{MoveSurfaceGrab, ResizeSurfaceGrab},
};

impl XdgShellHandler for Compositor {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        self.next_window_id += 1;
        let id = tarot_core::WindowId(self.next_window_id);
        self.families.register(id);
        window.user_data().insert_if_missing(|| id);
        window.user_data().insert_if_missing(|| {
            std::cell::RefCell::new(crate::window_state::WindowState::default())
        });
        let gtk_surface = window.toplevel().unwrap().wl_surface().clone();
        self.space.map_element(window, (0, 28), false);
        self.gtk_modal_changed(&gtk_surface, None);
        self.configure_windows();
        self.ui_dirty = true;
        self.redraw = true;
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.gtk_modal_changed(surface.wl_surface(), Some(false));
        let window = self
            .space
            .elements()
            .find(|window| window.toplevel().is_some_and(|top| top == &surface))
            .cloned();
        if let Some(window) = window
            && let Some(id) = window.user_data().get::<tarot_core::WindowId>().copied()
        {
            let previous = self.families.clone();
            self.families.remove(id);
            self.space.unmap_elem(&window);
            self.reconcile_families(previous, None);
        }
        self.ui_dirty = true;
        self.redraw = true;
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        self.update_metadata(&surface);
    }
    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        self.update_metadata(&surface);
    }

    fn parent_changed(&mut self, surface: ToplevelSurface) {
        let Some(id) = self.native_id(surface.wl_surface()) else {
            return;
        };
        let parent = surface.parent().and_then(|parent| self.native_id(&parent));
        let previous = self.families.clone();
        self.families.set_parent(id, parent);
        if self.families.parent(id).is_some() {
            self.reset_window_state(id);
        }
        self.reconcile_families(previous, None);
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        self.window_state_request(surface, crate::window_state::Request::Maximize(true));
    }
    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        self.window_state_request(surface, crate::window_state::Request::Maximize(false));
    }
    fn fullscreen_request(
        &mut self,
        surface: ToplevelSurface,
        _output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
    ) {
        self.window_state_request(surface, crate::window_state::Request::Fullscreen(true));
    }
    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        self.window_state_request(surface, crate::window_state::Request::Fullscreen(false));
    }
    fn minimize_request(&mut self, surface: ToplevelSurface) {
        self.minimize_window(surface);
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let Some(seat) = Seat::from_resource(&seat) else {
            return;
        };

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let Some(pointer) = seat.get_pointer() else {
                return;
            };

            let Some(window) = self
                .space
                .elements()
                .find(|w| {
                    w.toplevel()
                        .is_some_and(|top| top.wl_surface() == wl_surface)
                })
                .cloned()
            else {
                return;
            };
            let Some(initial_window_location) = self.space.element_location(&window) else {
                return;
            };

            let grab = MoveSurfaceGrab {
                start_data,
                window,
                initial_window_location,
            };

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let Some(seat) = Seat::from_resource(&seat) else {
            return;
        };

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let Some(pointer) = seat.get_pointer() else {
                return;
            };

            let Some(window) = self
                .space
                .elements()
                .find(|w| {
                    w.toplevel()
                        .is_some_and(|top| top.wl_surface() == wl_surface)
                })
                .cloned()
            else {
                return;
            };
            let Some(initial_window_location) = self.space.element_location(&window) else {
                return;
            };
            let initial_window_size = window.geometry().size;

            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            surface.send_pending_configure();

            let grab = ResizeSurfaceGrab::start(
                start_data,
                window,
                edges.into(),
                Rectangle::new(initial_window_location, initial_window_size),
            );

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn grab(&mut self, surface: PopupSurface, seat: wl_seat::WlSeat, serial: Serial) {
        self.start_popup_grab(surface, seat, serial);
    }

    fn popup_destroyed(&mut self, surface: PopupSurface) {
        let focused = self
            .seat
            .get_touch()
            .and_then(|touch| touch.grab_start_data())
            .and_then(|grab| grab.focus)
            .is_some_and(|(focus, _)| {
                &focus.surface == surface.wl_surface()
                    || self
                        .presented_surfaces
                        .get(&focus.surface)
                        .is_some_and(|placement| &placement.tree_root == surface.wl_surface())
            });
        if focused {
            self.cancel_native_touch_sequence();
        }
        self.redraw = true;
    }
}

// Xdg Shell
delegate_xdg_shell!(Compositor);

impl smithay::wayland::shell::xdg::dialog::XdgDialogHandler for Compositor {
    fn modal_changed(&mut self, surface: ToplevelSurface, modal: bool) {
        self.gtk_modal_changed(surface.wl_surface(), Some(modal));
    }
}
smithay::delegate_xdg_dialog!(Compositor);

fn check_grab(
    seat: &Seat<Compositor>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData<Compositor>> {
    let pointer = seat.get_pointer()?;

    // Check that this surface has a click grab.
    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;
    // If the focus was for a different surface, ignore the request.
    if !focus.surface.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}

/// Should be called on `WlSurface::commit`
pub fn handle_commit(
    popups: &mut PopupManager,
    space: &Space<Window>,
    surface: &WlSurface,
    unmapped_toplevel: bool,
) {
    // Handle toplevel commits.
    if let Some(window) = space
        .elements()
        .find(|w| w.toplevel().unwrap().wl_surface() == surface)
        .cloned()
    {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });

        // A null-buffer unmap discards the old mapping. The client must be
        // able to supply fresh parent/size hints before its next initial commit.
        if !initial_configure_sent && !unmapped_toplevel {
            window.toplevel().unwrap().send_configure();
        }
    }

    // Handle popup commits.
    popups.commit(surface);
    if let Some(popup) = popups.find_popup(surface) {
        match popup {
            PopupKind::Xdg(ref xdg) => {
                if !xdg.is_initial_configure_sent() {
                    // NOTE: This should never fail as the initial configure is always
                    // allowed.
                    xdg.send_configure().expect("initial configure failed");
                }
            }
            PopupKind::InputMethod(ref _input_method) => {}
        }
    }
}

impl Compositor {
    fn update_metadata(&mut self, surface: &ToplevelSurface) {
        let id = self
            .space
            .elements()
            .find(|window| window.toplevel() == Some(surface))
            .and_then(|window| window.user_data().get::<tarot_core::WindowId>().copied());
        if let Some(id) = id {
            let (title, app_id) = metadata(surface);
            self.shell.update_window(id, title, app_id);
            self.ui_dirty = true;
            self.redraw = true;
        }
    }

    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self
            .space
            .elements()
            .find(|w| w.toplevel().unwrap().wl_surface() == &root)
        else {
            return;
        };

        let id = *window.user_data().get::<tarot_core::WindowId>().unwrap();
        let card_area = self.window_area(id);
        let content = if let Some(owner) = self.families.owner(id)
            && self.fullscreen_window(owner)
        {
            self.fullscreen_input_area(owner)
        } else {
            self.layout.content()
        };
        let target = window
            .user_data()
            .get::<tarot_core::WindowId>()
            .map_or(card_area, |id| self.native_rect(*id, card_area));
        let Some(fit) = crate::focus::RenderGeometry::new(
            window.geometry(),
            target,
            1.0,
            f64::from(self.scale),
        ) else {
            return;
        };
        // A client may be fitted rather than obeying its configure size. Constrain
        // in its coordinate system, relative to the parent window geometry.
        let top_left = fit.to_window((f64::from(content.x), f64::from(content.y)).into());
        let bottom_right = fit.to_window(
            (
                f64::from(content.x + content.width),
                f64::from(content.y + content.height),
            )
                .into(),
        );
        let mut target = Rectangle::new(
            (top_left.x.floor() as i32, top_left.y.floor() as i32).into(),
            (
                (bottom_right.x - top_left.x).ceil() as i32,
                (bottom_right.y - top_left.y).ceil() as i32,
            )
                .into(),
        );
        target.loc -= window.geometry().loc;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}

pub(crate) fn metadata(surface: &ToplevelSurface) -> (String, String) {
    with_states(surface.wl_surface(), |states| {
        let attributes = states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .expect("xdg surface has attributes")
            .lock()
            .expect("xdg attributes lock");
        (
            attributes
                .title
                .clone()
                .unwrap_or_else(|| "Application".into()),
            attributes.app_id.clone().unwrap_or_default(),
        )
    })
}
