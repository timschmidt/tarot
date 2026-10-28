//! Restricted keyboard layer policy. It never takes the application's keyboard focus.
use crate::{Compositor, focus::SurfaceFocus};
use smithay::{
    backend::renderer::utils::with_renderer_surface_state,
    delegate_layer_shell, delegate_virtual_keyboard_manager,
    desktop::{LayerSurface as DesktopLayer, WindowSurfaceType},
    reexports::wayland_server::protocol::{wl_output::WlOutput, wl_surface::WlSurface},
    utils::{Logical, Point},
    wayland::shell::wlr_layer::{
        Anchor, KeyboardInteractivity, Layer, LayerSurface, WlrLayerShellHandler,
        WlrLayerShellState,
    },
};
use tarot_core::{View, WindowId};

pub struct KeyboardSurface {
    pub layer: DesktopLayer,
    pub owner: Option<WindowId>,
    pub height: i32,
    pub mapped: bool,
}

impl Compositor {
    pub fn keyboard_visible(&self) -> bool {
        self.shell_keyboard_visible() || self.native_keyboard_visible()
    }

    pub fn shell_keyboard_visible(&self) -> bool {
        if self.protected() {
            return false;
        }
        self.shell.credential_keyboard_visible()
            || (self.shell.view == View::Launcher
                && self.shell.shell_text_focus
                && !self.shell.system_menu_open
                && !self.shell.notifications_open)
    }

    pub fn native_keyboard_visible(&self) -> bool {
        if self.protected() {
            return false;
        }
        !self.shell.notifications_open
            && !self.shell.system_menu_open
            && self.keyboard.as_ref().is_some_and(|keyboard| {
                keyboard.mapped
                    && keyboard
                        .owner
                        .is_some_and(|owner| self.shell.view == View::App(owner))
            })
    }

    pub fn keyboard_surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(SurfaceFocus, Point<f64, Logical>)> {
        if !self.native_keyboard_visible() {
            return None;
        }
        let rect = self.layout.keyboard();
        if !rect.contains(pos.x as f32, pos.y as f32) {
            return None;
        }
        let origin: Point<f64, Logical> = (f64::from(rect.x), f64::from(rect.y)).into();
        self.keyboard
            .as_ref()?
            .layer
            .surface_under(pos - origin, WindowSurfaceType::ALL)
            .map(|(surface, _)| (surface.into(), (0.0, 0.0).into()))
    }

    pub fn configure_keyboard(&self) {
        if let Some(keyboard) = &self.keyboard {
            keyboard.layer.layer_surface().with_pending_state(|state| {
                state.size = Some((self.layout.width as i32, keyboard.height).into());
            });
            keyboard.layer.layer_surface().send_pending_configure();
        }
    }

    pub fn commit_keyboard(&mut self, surface: &WlSurface) {
        let Some(keyboard) = self
            .keyboard
            .as_mut()
            .filter(|keyboard| keyboard.layer.wl_surface() == surface)
        else {
            return;
        };
        let requested = keyboard.layer.cached_state();
        if requested.keyboard_interactivity != KeyboardInteractivity::None
            || requested.anchor != (Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT)
            || requested.size.h <= 0
        {
            keyboard.layer.layer_surface().send_close();
            keyboard.mapped = false;
            self.refresh_keyboard();
            return;
        }
        keyboard.height = requested
            .size
            .h
            .clamp(1, (self.layout.height * 0.5).max(1.0) as i32);
        let mapped =
            with_renderer_surface_state(surface, |state| state.buffer().is_some()).unwrap_or(false);
        if mapped && !keyboard.mapped {
            keyboard.owner = match self.shell.view {
                View::App(owner) => Some(owner),
                _ => None,
            };
        }
        keyboard.mapped = mapped;
        self.configure_keyboard();
        self.refresh_keyboard();
    }

    pub fn refresh_keyboard(&mut self) {
        let height = if self.shell_keyboard_visible() {
            240.0_f32.min(self.layout.height * 0.5)
        } else if self.native_keyboard_visible() {
            self.keyboard.as_ref().unwrap().height as f32
        } else {
            0.0
        };
        if self.layout.keyboard_height != height {
            self.layout.keyboard_height = height;
            self.configure_windows();
            self.ui_dirty = true;
            self.redraw = true;
        }
    }
}

impl WlrLayerShellHandler for Compositor {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }
    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        _output: Option<WlOutput>,
        layer: Layer,
        namespace: String,
    ) {
        if namespace != "tarot-keyboard" || layer != Layer::Overlay || self.keyboard.is_some() {
            surface.send_close();
            return;
        }
        self.keyboard = Some(KeyboardSurface {
            layer: DesktopLayer::new(surface, namespace),
            owner: None,
            height: 240,
            mapped: false,
        });
    }
    fn layer_destroyed(&mut self, surface: LayerSurface) {
        if self
            .keyboard
            .as_ref()
            .is_some_and(|keyboard| keyboard.layer.layer_surface() == &surface)
        {
            if !self.protected() {
                self.cancel_native_touch_sequence();
            }
            self.keyboard = None;
            self.refresh_keyboard();
            self.redraw = true;
        }
    }
}

delegate_layer_shell!(Compositor);
delegate_virtual_keyboard_manager!(Compositor);
