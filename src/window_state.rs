//! Mobile xdg window states. Requests negotiate; committed state drives pixels.
use crate::Compositor;
use smithay::{
    desktop::Window,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::{State, WmCapabilities},
    utils::{Logical, Size},
    wayland::shell::xdg::ToplevelSurface,
};
use std::cell::RefCell;
use tarot_core::{Command, Layout, Rect, View, WindowId, motion::Presentation};

#[derive(Default)]
pub struct WindowState {
    maximized: bool,
    fullscreen: bool,
    presented_fullscreen: bool,
}

pub enum Request {
    Maximize(bool),
    Fullscreen(bool),
}

impl Compositor {
    pub fn fullscreen_window(&self, id: WindowId) -> bool {
        self.families.owner(id) == Some(id)
            && self.native_window(id).is_some_and(|window| {
                window
                    .toplevel()
                    .is_some_and(|top| top.current_state().states.contains(State::Fullscreen))
            })
    }

    pub fn fullscreen_area(&self) -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: self.layout.width,
            height: self.layout.height,
        }
    }

    pub fn fullscreen_viewport(&self, owner: WindowId) -> Rect {
        // The input panel reserves space only for its owning card family.
        // Hidden fullscreen clients keep their full-output configuration.
        let reserved = self.native_keyboard_visible()
            && self
                .keyboard
                .as_ref()
                .is_some_and(|keyboard| keyboard.owner == Some(owner));
        Rect {
            height: if reserved {
                self.layout.keyboard().y
            } else {
                self.layout.height
            },
            ..self.fullscreen_area()
        }
    }

    pub fn window_area(&self, id: WindowId) -> Rect {
        if let Some(root) = self.families.owner(id)
            && self.fullscreen_window(root)
        {
            self.fullscreen_viewport(root)
        } else {
            self.layout.content()
        }
    }

    pub fn fullscreen_input_area(&self, owner: WindowId) -> Rect {
        let viewport = self.fullscreen_viewport(owner);
        Rect {
            height: viewport.height.min(self.layout.gesture().y),
            ..viewport
        }
    }

    pub fn active_input_area(&self) -> Rect {
        if let View::App(id) = self.shell.view
            && self.fullscreen_window(id)
        {
            self.fullscreen_input_area(id)
        } else {
            self.layout.content()
        }
    }

    pub fn fullscreen_chrome(&self) -> bool {
        !self.protected()
            && !self.shell.system_menu_open
            && !self.shell.notifications_open
            && self.presentation.overview == 0.0
            && matches!(self.shell.view, View::App(id) if self.fullscreen_window(id))
    }

    pub fn expand_fullscreen_frame(&self, frame: &mut Presentation) {
        let amount = (1.0 - frame.overview).clamp(0.0, 1.0);
        let content = self.layout.content();
        for visual in &mut frame.windows {
            if self.fullscreen_window(visual.id) {
                let ratio = visual.rect.height / content.height.max(1.0);
                visual.rect.y -= Layout::STATUS * ratio * amount;
                visual.rect.height +=
                    (self.fullscreen_viewport(visual.id).height - content.height) * ratio * amount;
            }
        }
    }

    pub fn configure_window_state(&self, window: &Window) {
        let Some(top) = window.toplevel() else { return };
        let id = *window.user_data().get::<WindowId>().unwrap();
        let transient = self.families.parent(id).is_some();
        let requested = window
            .user_data()
            .get::<RefCell<WindowState>>()
            .unwrap()
            .borrow();
        let fullscreen = requested.fullscreen && !transient;
        let area = if fullscreen {
            self.fullscreen_viewport(id)
        } else if transient {
            self.window_area(id)
        } else {
            self.layout.content()
        };
        let size: Size<i32, Logical> = (area.width as i32, area.height as i32).into();
        top.with_pending_state(|state| {
            state.size = (!transient).then_some(size);
            state.bounds = Some(size);
            state.states.unset(State::Maximized);
            state.states.unset(State::Fullscreen);
            if fullscreen {
                state.states.set(State::Fullscreen);
            } else if requested.maximized && !transient {
                state.states.set(State::Maximized);
            }
            state.fullscreen_output = None; // Single selected output; a preference cannot move focus.
            state.capabilities = if transient {
                vec![]
            } else {
                vec![
                    WmCapabilities::Maximize,
                    WmCapabilities::Fullscreen,
                    WmCapabilities::Minimize,
                ]
            }
            .into();
        });
    }

    pub fn window_state_request(&mut self, surface: ToplevelSurface, request: Request) {
        let Some(id) = self.native_id(surface.wl_surface()) else {
            return;
        };
        let window = self.native_window(id).unwrap();
        {
            let mut state = window
                .user_data()
                .get::<RefCell<WindowState>>()
                .unwrap()
                .borrow_mut();
            if self.families.parent(id).is_none() {
                match request {
                    Request::Maximize(value) => state.maximized = value,
                    Request::Fullscreen(value) => state.fullscreen = value,
                }
            }
        }
        self.configure_window_state(&window);
        // Every mapped/handshaken request gets a reply, even when unchanged or
        // rejected. Before the first commit only update pending configuration.
        if surface.is_initial_configure_sent() {
            surface.send_configure();
        }
    }

    pub fn reset_window_state(&mut self, id: WindowId) {
        if let Some(window) = self.native_window(id) {
            *window
                .user_data()
                .get::<RefCell<WindowState>>()
                .unwrap()
                .borrow_mut() = WindowState::default();
        }
        self.ui_dirty = true;
    }

    pub fn commit_window_state(&mut self, id: WindowId) {
        let Some(window) = self.native_window(id) else {
            return;
        };
        let fullscreen = self.fullscreen_window(id);
        let changed = {
            let mut state = window
                .user_data()
                .get::<RefCell<WindowState>>()
                .unwrap()
                .borrow_mut();
            let changed = state.presented_fullscreen != fullscreen;
            state.presented_fullscreen = fullscreen;
            changed
        };
        if changed {
            self.cancel_input_sequences();
            self.ui_dirty = true;
            self.redraw = true;
        }
    }

    pub fn minimize_window(&mut self, surface: ToplevelSurface) {
        let Some(id) = self.native_id(surface.wl_surface()) else {
            return;
        };
        if self.families.owner(id) == Some(id)
            && self.shell.view == View::App(id)
            && !self.shell.system_menu_open
            && !self.shell.notifications_open
        {
            self.apply_command(Command::Overview);
        }
    }
}
