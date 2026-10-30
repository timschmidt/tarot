//! GTK surface claims, foreground ownership and the native menu/UI boundary.
use crate::{
    CalloopData, Compositor,
    gtk_protocol::{gtk_shell1, gtk_surface1},
};
use smithay::reexports::{
    calloop::{
        LoopHandle,
        ping::{Ping, make_ping},
    },
    wayland_server::{
        Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
        backend::ClientId, protocol::wl_surface::WlSurface,
    },
};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};
use tarot::native_menu::{
    transport::{Authority, Export},
    worker::Watch,
};
use tarot_core::{
    View, WindowId,
    app_menu::{self, Stamp, Status},
};

struct Surface {
    extension: gtk_surface1::GtkSurface1,
    modal: bool,
    claim: Option<Claim>,
}
struct Claim {
    id: u64,
    export: Export,
    authority: Authority,
    epoch: Arc<Mutex<Option<String>>>,
}
struct Active {
    surface: WlSurface,
    claim: u64,
    owner: WindowId,
    source: u64,
    watch: Option<Watch>,
}
pub struct State {
    surfaces: HashMap<WlSurface, Surface>,
    xdg_modal: HashSet<WlSurface>,
    next: u64,
    active: Option<Active>,
    retired: Option<Watch>,
    ping: Ping,
}
impl State {
    pub fn new(dh: &DisplayHandle, handle: LoopHandle<'_, CalloopData>) -> Self {
        dh.create_global::<Compositor, gtk_shell1::GtkShell1, _>(4, ());
        let (ping, source) = make_ping().expect("native menu event source");
        handle
            .insert_source(source, |_, _, data| data.state.sync_app_menu())
            .expect("native menu events");
        Self {
            surfaces: HashMap::new(),
            xdg_modal: HashSet::new(),
            next: 0,
            active: None,
            retired: None,
            ping,
        }
    }
    fn next(&mut self) -> u64 {
        self.next = self.next.wrapping_add(1);
        self.next
    }
}
impl Compositor {
    fn publish_app_menu(&mut self, update: app_menu::State) {
        if self.shell.app_menu != update
            && self.shell.system_menu_open
            && self.shell.system_page == tarot_core::SystemPage::Activity
        {
            self.cancel_input_sequences();
        }
        self.shell.update_app_menu(update);
    }
    pub fn sync_app_menu(&mut self) {
        // Dead resources retire their claim authority before any worker can
        // publish or activate a former surface's choice.
        self.app_menus.surfaces.retain(|s, _| s.is_alive());
        self.app_menus.xdg_modal.retain(Resource::is_alive);
        if self.app_menus.retired.as_ref().is_some_and(Watch::finished) {
            self.app_menus.retired.take();
        }
        let desired = if !self.protected() {
            if let View::App(owner) = self.shell.view {
                self.focused_window(owner)
                    .and_then(|w| w.toplevel().map(|t| t.wl_surface().clone()))
                    .filter(|s| self.native_id(s).is_some_and(|id| self.families.mapped(id)))
                    .and_then(|surface| {
                        self.app_menus
                            .surfaces
                            .get(&surface)
                            .and_then(|s| s.claim.as_ref())
                            .map(|c| (owner, surface, c.id))
                    })
            } else {
                None
            }
        } else {
            None
        };
        let matches = self.app_menus.active.as_ref().is_some_and(|a| {
            desired.as_ref().is_some_and(|(owner, s, id)| {
                a.owner == *owner && a.surface == *s && a.claim == *id
            })
        });
        if !matches && let Some(active) = self.app_menus.active.take() {
            if let Some(watch) = active.watch {
                watch.stop();
                self.app_menus.retired = Some(watch);
            }
            self.publish_app_menu(Default::default());
            self.ui_dirty = true;
            self.redraw = true;
        }
        if self.app_menus.active.is_none()
            && self.app_menus.retired.is_none()
            && let Some((owner, surface, claim_id)) = desired
            && let Some((peer, client_lease)) = self.native_export_peer(&surface)
            && let Some(claim) = self
                .app_menus
                .surfaces
                .get(&surface)
                .and_then(|s| s.claim.as_ref())
            && let Ok(lease) = client_lease.intersect(&claim.authority.lease())
        {
            let export = claim.export.clone();
            let epoch = claim.epoch.clone();
            let source = self.app_menus.next();
            let ping = self.app_menus.ping.clone();
            let watch =
                Watch::start(export, peer, lease, epoch, Arc::new(move || ping.ping())).ok();
            let status = if watch.is_some() {
                Status::Loading
            } else {
                Status::Unavailable
            };
            self.publish_app_menu(app_menu::State {
                owner: Some(owner),
                stamp: Stamp {
                    source,
                    revision: 1,
                },
                status,
                nodes: vec![],
            });
            self.app_menus.active = Some(Active {
                surface,
                claim: claim_id,
                owner,
                source,
                watch,
            });
            self.ui_dirty = true;
            self.redraw = true;
        }
        if let Some(active) = &self.app_menus.active
            && let Some(update) = active.watch.as_ref().and_then(Watch::take)
        {
            self.publish_app_menu(app_menu::State {
                owner: Some(active.owner),
                stamp: Stamp {
                    source: active.source,
                    revision: update.revision,
                },
                status: update.status,
                nodes: update.nodes,
            });
            self.ui_dirty = true;
            self.redraw = true;
        }
    }
    pub fn dispatch_app_menu(&mut self, owner: WindowId, stamp: Stamp, id: u64) {
        if self.protected() || self.shell.view != View::App(owner) {
            return;
        }
        if let Some(active) = &self.app_menus.active
            && active.owner == owner
            && active.source == stamp.source
            && self
                .app_menus
                .surfaces
                .get(&active.surface)
                .and_then(|s| s.claim.as_ref())
                .is_some_and(|c| c.id == active.claim)
            && self.native_export_peer(&active.surface).is_some()
            && self
                .focused_window(owner)
                .and_then(|w| w.toplevel().map(|t| t.wl_surface().clone()))
                .as_ref()
                == Some(&active.surface)
            && let Some(watch) = &active.watch
        {
            let _ = watch.activate(stamp.revision, id);
        }
    }
    pub fn gtk_modal_changed(&mut self, surface: &WlSurface, xdg: Option<bool>) {
        if let Some(modal) = xdg {
            if modal {
                self.app_menus.xdg_modal.insert(surface.clone());
            } else {
                self.app_menus.xdg_modal.remove(surface);
            }
        }
        if let Some(id) = self.native_id(surface) {
            let modal = self.app_menus.xdg_modal.contains(surface)
                || self
                    .app_menus
                    .surfaces
                    .get(surface)
                    .is_some_and(|s| s.modal);
            let previous = self.families.clone();
            self.families.set_modal(id, modal);
            self.reconcile_families(previous, None);
        }
    }
}
impl GlobalDispatch<gtk_shell1::GtkShell1, ()> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        new: New<gtk_shell1::GtkShell1>,
        _: &(),
        init: &mut DataInit<'_, Self>,
    ) {
        let resource = init.init(new, ());
        // Keep toolkit-owned controls until complete installed capability acceptance.
        resource.capabilities(0);
    }
}
impl Dispatch<gtk_shell1::GtkShell1, ()> for Compositor {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &gtk_shell1::GtkShell1,
        request: gtk_shell1::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let gtk_shell1::Request::GetGtkSurface {
            gtk_surface,
            surface,
        } = request
        {
            let extension = init.init(gtk_surface, surface.clone());
            if let Some(previous) = state.app_menus.surfaces.remove(&surface) {
                previous
                    .extension
                    .post_error(0u32, "surface extension replaced");
            }
            state.app_menus.surfaces.insert(
                surface,
                Surface {
                    extension,
                    modal: false,
                    claim: None,
                },
            );
        }
    }
}
impl Dispatch<gtk_surface1::GtkSurface1, WlSurface> for Compositor {
    fn request(
        state: &mut Self,
        _: &Client,
        resource: &gtk_surface1::GtkSurface1,
        request: gtk_surface1::Request,
        surface: &WlSurface,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if !state
            .app_menus
            .surfaces
            .get(surface)
            .is_some_and(|s| s.extension == *resource)
        {
            return;
        }
        match request {
            gtk_surface1::Request::SetDbusProperties {
                app_menu_path,
                menubar_path,
                window_object_path,
                application_object_path,
                unique_bus_name,
                ..
            } => {
                let export = unique_bus_name
                    .as_deref()
                    .zip(app_menu_path.as_deref().or(menubar_path.as_deref()))
                    .and_then(|(owner, menu)| {
                        Export::new(
                            owner,
                            menu,
                            application_object_path.as_deref(),
                            window_object_path.as_deref(),
                        )
                        .ok()
                    });
                let id = state.app_menus.next();
                state.app_menus.surfaces.get_mut(surface).unwrap().claim =
                    export.map(|export| Claim {
                        id,
                        export,
                        authority: Default::default(),
                        epoch: Default::default(),
                    });
                state.sync_app_menu();
            }
            gtk_surface1::Request::SetModal | gtk_surface1::Request::UnsetModal => {
                let modal = matches!(request, gtk_surface1::Request::SetModal);
                state.app_menus.surfaces.get_mut(surface).unwrap().modal = modal;
                state.gtk_modal_changed(surface, None);
            }
            gtk_surface1::Request::Release => {
                state.app_menus.surfaces.remove(surface);
                state.gtk_modal_changed(surface, None);
                state.sync_app_menu();
            }
            // Untrusted startup/focus hints cannot bypass current family/seat ownership.
            _ => {}
        }
    }
    fn destroyed(
        state: &mut Self,
        _: ClientId,
        resource: &gtk_surface1::GtkSurface1,
        surface: &WlSurface,
    ) {
        if state
            .app_menus
            .surfaces
            .get(surface)
            .is_some_and(|s| s.extension == *resource)
        {
            state.app_menus.surfaces.remove(surface);
            state.gtk_modal_changed(surface, None);
            state.sync_app_menu();
        }
    }
}
