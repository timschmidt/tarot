// Adapted from Smithay smallvil v0.7.0 (MIT).
// Copyright (c) 2017 Victor Berger and Victoria Brekenfeld.
// See third-party/Smithay-MIT.txt for the license.

use crate::{Compositor, grabs::resize_grab, state::ClientState};
use smithay::{
    backend::allocator::dmabuf::Dmabuf,
    backend::renderer::utils::{on_commit_buffer_handler, with_renderer_surface_state},
    delegate_compositor, delegate_dmabuf, delegate_shm,
    reexports::wayland_server::{
        Client,
        protocol::{wl_buffer, wl_surface::WlSurface},
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{
            CompositorClientState, CompositorHandler, CompositorState, get_parent,
            is_sync_subsurface,
        },
        dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier},
        shm::{ShmHandler, ShmState},
    },
};

use super::xdg_shell;

impl CompositorHandler for Compositor {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        self.redraw = true;
        on_commit_buffer_handler::<Self>(surface);
        self.commit_keyboard(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            let window = self
                .space
                .elements()
                .find(|w| {
                    w.toplevel()
                        .is_some_and(|toplevel| toplevel.wl_surface() == &root)
                })
                .cloned();
            if let Some(window) = window {
                window.on_commit();
                if let Some(id) = window.user_data().get::<tarot_core::WindowId>().copied() {
                    let mapped =
                        with_renderer_surface_state(&root, |state| state.buffer().is_some())
                            .unwrap_or(false);
                    let listed = self.shell.cards.iter().any(|card| card.id == id);
                    if mapped
                        && !listed
                        && let Some(toplevel) = window.toplevel()
                    {
                        let (title, app_id) = xdg_shell::metadata(toplevel);
                        if !self.protected() {
                            self.cancel_input_sequences();
                        }
                        let effects = if self.protected() {
                            self.shell.add_window_in_background(id, title, app_id);
                            Vec::new()
                        } else {
                            self.shell.add_window(id, title, app_id)
                        };
                        self.apply_effects(effects);
                        self.ui_dirty = true;
                    } else if !mapped && listed {
                        if !self.protected() {
                            self.cancel_input_sequences();
                        }
                        let effects = self.shell.remove_window(id);
                        self.apply_effects(effects);
                        self.ui_dirty = true;
                    }
                }
            }
        };

        xdg_shell::handle_commit(&mut self.popups, &self.space, surface);
        resize_grab::handle_commit(&mut self.space, surface);
    }
}

impl BufferHandler for Compositor {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for Compositor {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(Compositor);
delegate_shm!(Compositor);

impl DmabufHandler for Compositor {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }
    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        dmabuf: Dmabuf,
        notifier: ImportNotifier,
    ) {
        if self
            .dmabuf_importer
            .as_mut()
            .is_some_and(|import| import(&dmabuf))
        {
            self.metrics.dmabuf_imports = self.metrics.dmabuf_imports.saturating_add(1);
            let _ = notifier.successful::<Self>();
        } else {
            self.metrics.dmabuf_rejections = self.metrics.dmabuf_rejections.saturating_add(1);
            notifier.failed();
        }
    }
}
delegate_dmabuf!(Compositor);
