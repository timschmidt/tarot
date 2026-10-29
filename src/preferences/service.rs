//! One writer for all preference sections. Renderer access only takes a short
//! in-memory mutex; file transactions never hold it.
use super::{Notifications, Preferences, Store};
use crate::{CalloopData, Compositor};
use anyhow::{Context, Result};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
};

#[derive(Clone, Debug)]
pub struct Status {
    pub saved: bool,
    pub error: Option<String>,
}

#[derive(Default)]
struct State {
    desired: Preferences,
    revision: u64,
    dirty: bool,
    saved: bool,
    error: Option<String>,
}

struct Shared {
    state: Mutex<State>,
    stop: AtomicBool,
    wake: mpsc::SyncSender<()>,
    ping: Ping,
}

#[derive(Clone)]
pub struct Handle(Arc<Shared>);
impl Handle {
    pub fn status(&self) -> Status {
        let state = self.0.state.lock().unwrap();
        Status {
            saved: state.saved,
            error: state.error.clone(),
        }
    }
    pub fn initial_notifications(&self) -> Notifications {
        self.0.state.lock().unwrap().desired.notifications.clone()
    }
    fn change(&self, update: impl FnOnce(&mut Preferences)) {
        let mut state = self.0.state.lock().unwrap();
        update(&mut state.desired);
        state.revision = state
            .revision
            .checked_add(1)
            .expect("settings revision exhausted");
        state.dirty = true;
        state.saved = false;
        drop(state);
        self.0.ping.ping();
        let _ = self.0.wake.try_send(());
    }
    pub fn notifications(&self, settings: Notifications) {
        self.change(|p| p.notifications = settings);
    }
    pub fn display(&self, settings: tarot_core::display::Preferences) {
        self.change(|p| p.display = settings);
    }
    pub fn favorites(&self, settings: tarot_core::quick_launch::Favorites) {
        self.change(|p| p.quick_launch = Some(settings));
    }
    pub fn launcher(&self, settings: tarot_core::launcher::Settings) {
        self.change(|p| p.launcher = Some(settings));
    }
}

pub struct Server {
    pub handle: Handle,
    pub initial: Preferences,
    worker: Option<JoinHandle<()>>,
    _isolated_directory: Option<tempfile::TempDir>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.handle.0.stop.store(true, Ordering::Release);
        let _ = self.handle.0.wake.try_send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Compositor {
    pub fn persist_display_preferences(&mut self) {
        if let Some(server) = &self.preferences {
            server.handle.display(tarot_core::display::Preferences {
                rotation_locked: self.shell.rotation_locked,
                locked_orientation: self.output_geometry.orientation,
                idle: self.shell.display.idle,
            });
            self.refresh_preferences_status();
        }
    }
    pub fn refresh_preferences_status(&mut self) {
        if let Some(server) = &self.preferences {
            let status = server.handle.status();
            self.shell.display.settings_available = true;
            self.shell.display.settings_saved = status.saved;
            self.shell.display.settings_error = status.error.clone();
            self.shell.notifications.settings_available = true;
            self.shell.notifications.settings_saved = status.saved;
            self.shell.notifications.settings_error = status.error.clone();
            self.shell.favorites_saved = status.saved;
            self.shell.favorites_error = status.error.clone();
            self.shell.launcher.available = true;
            self.shell.launcher.saved = status.saved;
            self.shell.launcher.save_error = status.error;
        }
    }
}

pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    path: PathBuf,
    isolated_directory: Option<tempfile::TempDir>,
) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let (wake, receiver) = mpsc::sync_channel(1);
    let shared = Arc::new(Shared {
        state: Mutex::new(State::default()),
        stop: AtomicBool::new(false),
        wake,
        ping,
    });
    handle
        .insert_source(source, move |(), _, data| {
            data.state.refresh_preferences_status();
            data.state.ui_dirty = true;
            data.state.redraw = true;
        })
        .context("registering settings updates")?;
    let (ready, initialized) = mpsc::sync_channel(1);
    let worker_shared = shared.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-settings".into())
        .spawn(move || {
            let mut store = Store::load(path);
            {
                let mut state = worker_shared.state.lock().unwrap();
                state.desired = store.current.clone();
                state.saved = store.saved;
                state.error = store.error.clone();
            }
            if ready.send(store.current.clone()).is_err() {
                return;
            }
            loop {
                let next = {
                    let mut state = worker_shared.state.lock().unwrap();
                    if state.dirty {
                        state.dirty = false;
                        Some((state.desired.clone(), state.revision))
                    } else {
                        None
                    }
                };
                if let Some((preferences, revision)) = next {
                    store.replace(preferences);
                    {
                        let mut state = worker_shared.state.lock().unwrap();
                        // A completed older write cannot claim newer UI changes
                        // were saved. The next pass writes the latest whole file.
                        if state.revision == revision {
                            state.saved = store.saved;
                            state.error = store.error.clone();
                        }
                    }
                    worker_shared.ping.ping();
                    continue;
                }
                if worker_shared.stop.load(Ordering::Acquire) || receiver.recv().is_err() {
                    break;
                }
            }
        })?;
    // This happens before backend initialization, never on an active renderer.
    // Notification privacy and the locked orientation are ready for frame one.
    let initial = initialized.recv().context("loading startup preferences")?;
    Ok(Server {
        handle: Handle(shared),
        initial,
        worker: Some(worker),
        _isolated_directory: isolated_directory,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_sections_are_merged_and_shutdown_flushes_the_latest_change() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        let event_loop = smithay::reexports::calloop::EventLoop::<CalloopData>::try_new().unwrap();
        let server = start(event_loop.handle(), path.clone(), None).unwrap();
        let notifications = server.handle.clone();
        let display = server.handle.clone();
        let favorites = server.handle.clone();
        let launcher = server.handle.clone();
        let notification_thread = std::thread::spawn(move || {
            for n in 0..100 {
                notifications.notifications(Notifications {
                    dnd: n % 2 == 1,
                    hide_previews: true,
                });
            }
        });
        let display_thread = std::thread::spawn(move || {
            for n in 0..100 {
                display.display(tarot_core::display::Preferences {
                    rotation_locked: true,
                    locked_orientation: tarot_core::orientation::Orientation::LeftUp,
                    idle: tarot_core::idle::Settings {
                        lock_after_seconds: 30 + n,
                        off_after_seconds: 180,
                        screen_off_enabled: true,
                    },
                });
            }
        });
        let favorites_thread = std::thread::spawn(move || {
            for n in 0..100 {
                favorites.favorites(tarot_core::quick_launch::Favorites {
                    applications: vec![format!("favorite{n}.desktop")],
                });
            }
        });
        let launcher_thread = std::thread::spawn(move || {
            for n in 0..100 {
                let mut pages = tarot_core::launcher::Settings::default();
                pages
                    .edit(tarot_core::launcher::Edit::Create(format!("Page {n}")), &[])
                    .unwrap();
                launcher.launcher(pages);
            }
        });
        launcher_thread.join().unwrap();
        notification_thread.join().unwrap();
        display_thread.join().unwrap();
        favorites_thread.join().unwrap();
        drop(server);
        let loaded = Store::load(path);
        assert!(loaded.saved && loaded.error.is_none());
        assert!(loaded.current.notifications.dnd && loaded.current.notifications.hide_previews);
        assert!(loaded.current.display.rotation_locked);
        assert_eq!(
            loaded.current.display.locked_orientation,
            tarot_core::orientation::Orientation::LeftUp
        );
        assert_eq!(loaded.current.display.idle.lock_after_seconds, 129);
        let pages = loaded.current.launcher.as_ref().unwrap();
        assert_eq!(pages.page(pages.selected).unwrap().name, "Page 99");
        assert_eq!(
            loaded.current.quick_launch.unwrap().applications,
            ["favorite99.desktop"]
        );
    }
}
