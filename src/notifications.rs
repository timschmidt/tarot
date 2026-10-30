//! Session notification service. D-Bus and expiry work stay off the renderer thread.
use crate::{CalloopData, Compositor, preferences};
use anyhow::{Context, Result};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    collections::HashMap,
    ffi::OsString,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot_core::notifications::{Action, Error, Incoming, Signal, Snapshot, Store, UiAction};
use zbus::{
    blocking::{Connection, connection::Builder, fdo::DBusProxy},
    fdo,
    message::Header,
    zvariant::OwnedValue,
};

const NAME: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

enum Event {
    Wake,
    Ui(UiAction),
    Shutdown,
}

#[derive(Default)]
struct Status {
    available: bool,
    error: Option<String>,
}

struct Shared {
    store: Mutex<Store>,
    preferences: preferences::Handle,
    start: Instant,
    pending: Arc<Mutex<Option<Snapshot>>>,
    ping: Ping,
    wake: mpsc::SyncSender<Event>,
    bus_lost: AtomicBool,
    stopping: AtomicBool,
    active_epoch: AtomicU64,
    generation: AtomicU64,
    status: Mutex<Status>,
    wayland_display: OsString,
    feedback: Option<crate::feedback::Handle>,
}

impl Shared {
    fn publish(&self) {
        let mut pending = self.pending.lock().unwrap();
        let mut snapshot = {
            let store = self.store.lock().unwrap();
            let snapshot = store.snapshot();
            if let Some(feedback) = &self.feedback {
                let (dnd, live) = store.feedback_state();
                feedback.retain(dnd, &live);
            }
            snapshot
        };
        let status = self.status.lock().unwrap();
        snapshot.generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        snapshot.available = status.available;
        snapshot.error = status.error.clone();
        let preferences = self.preferences.status();
        snapshot.settings_error = preferences.error;
        snapshot.settings_saved = preferences.saved;
        snapshot.settings_available = true;
        if !snapshot.available {
            snapshot.banner = None;
        }
        *pending = Some(snapshot);
        drop(pending);
        self.ping.ping();
    }
    fn changed(&self) {
        self.publish();
        let _ = self.wake.try_send(Event::Wake);
    }
    fn unavailable(&self, error: String) {
        self.store.lock().unwrap().disconnected();
        {
            let mut status = self.status.lock().unwrap();
            status.available = false;
            status.error = Some(error);
        }
        self.publish();
    }
    fn ui(&self, action: UiAction) {
        let settings = if let UiAction::Settings { dnd, hide_previews } = &action {
            Some(preferences::Notifications {
                dnd: *dnd,
                hide_previews: *hide_previews,
            })
        } else {
            None
        };
        {
            let mut store = self.store.lock().unwrap();
            store.ui(action);
            if let Some(feedback) = &self.feedback {
                let (dnd, live) = store.feedback_state();
                feedback.retain(dnd, &live);
            }
        }
        if let Some(settings) = settings {
            self.preferences.notifications(settings);
        }
        self.publish();
    }
}

struct Api {
    shared: Arc<Shared>,
    epoch: u64,
}

impl Api {
    fn active(&self) -> fdo::Result<()> {
        if self.shared.stopping.load(Ordering::Acquire)
            || self.shared.active_epoch.load(Ordering::Acquire) != self.epoch
        {
            return Err(fdo::Error::Failed(
                "Notification service is reconnecting".into(),
            ));
        }
        Ok(())
    }
}

fn dbus_error(error: Error) -> fdo::Error {
    match error {
        Error::Invalid => {
            fdo::Error::InvalidArgs("Notification fields exceed limits or are invalid".into())
        }
        Error::Unknown => fdo::Error::Failed("Unknown notification".into()),
        Error::NotOwner => {
            fdo::Error::AccessDenied("Notification belongs to another sender".into())
        }
        Error::Full => fdo::Error::LimitsExceeded("Notification capacity reached".into()),
    }
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Api {
    fn get_capabilities(&self) -> Vec<&str> {
        vec!["actions", "body", "persistence"]
    }
    fn get_server_information(&self) -> (&str, &str, &str, &str) {
        ("Tarot", "Tarot", env!("CARGO_PKG_VERSION"), "1.2")
    }

    #[allow(clippy::too_many_arguments)] // Standard D-Bus signature.
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
        #[zbus(header)] header: Header<'_>,
    ) -> fdo::Result<u32> {
        if !actions.len().is_multiple_of(2)
            || actions.len() > 16
            || hints.len() > 32
            || app_icon.len() > 1024
        {
            return Err(dbus_error(Error::Invalid));
        }
        let owner = header
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("Sender required".into()))?
            .as_str();
        let bool_hint = |name: &str| -> fdo::Result<bool> {
            hints
                .get(name)
                .map(bool::try_from)
                .transpose()
                .map(|value| value.unwrap_or(false))
                .map_err(|_| dbus_error(Error::Invalid))
        };
        let urgency = hints
            .get("urgency")
            .map(u8::try_from)
            .transpose()
            .map_err(|_| dbus_error(Error::Invalid))?
            .unwrap_or(1);
        let feedback_profile = match hints
            .get("x-phosh-fb-profile")
            .and_then(|v| <&str>::try_from(v).ok())
        {
            Some("none") => None,
            Some(value) => tarot_core::feedback::Profile::parse(value),
            None => Some(tarot_core::feedback::Profile::Full),
        };
        let feedback_profile = if bool_hint("suppress-sound")? {
            feedback_profile.map(|p| {
                if p == tarot_core::feedback::Profile::Full {
                    tarot_core::feedback::Profile::Quiet
                } else {
                    p
                }
            })
        } else {
            feedback_profile
        };
        let input = Incoming {
            app_name,
            summary,
            body,
            actions: actions
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| Action {
                    key: pair[0].clone(),
                    label: pair[1].clone(),
                })
                .collect(),
            urgency,
            expire_ms: expire_timeout,
            resident: bool_hint("resident")?,
            transient: bool_hint("transient")?,
        };
        let mut store = self.shared.store.lock().unwrap();
        self.active()?;
        let id = store
            .notify(owner, replaces_id, input, self.shared.start.elapsed())
            .map_err(dbus_error)?;
        if let Some(feedback) = &self.shared.feedback
            && let Some(revision) = store.notification_revision(id)
        {
            feedback.notify(id, revision, urgency, feedback_profile);
        }
        drop(store);
        self.shared.changed();
        Ok(id)
    }

    fn close_notification(&self, id: u32, #[zbus(header)] header: Header<'_>) -> fdo::Result<()> {
        let owner = header
            .sender()
            .ok_or_else(|| fdo::Error::AccessDenied("Sender required".into()))?
            .as_str();
        let mut store = self.shared.store.lock().unwrap();
        self.active()?;
        store.close(owner, id).map_err(dbus_error)?;
        drop(store);
        self.shared.changed();
        Ok(())
    }

    #[zbus(signal)]
    async fn notification_closed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;
    #[zbus(signal)]
    async fn action_invoked(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;
}

pub struct Server {
    sender: mpsc::SyncSender<Event>,
    connection: Arc<Mutex<Option<Connection>>>,
    worker: Option<JoinHandle<()>>,
    shared: Arc<Shared>,
}

impl Server {
    pub fn action(&self, action: UiAction) -> Result<()> {
        self.sender
            .try_send(Event::Ui(action))
            .map_err(|_| anyhow::anyhow!("Notification service is busy or unavailable"))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::Release);
        let connection = self.connection.lock().unwrap().take();
        if let Some(connection) = connection {
            let _ = connection.close();
        }
        let _ = self.sender.try_send(Event::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
    preferences: preferences::Handle,
    feedback: Option<crate::feedback::Handle>,
) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let pending = Arc::new(Mutex::new(None::<Snapshot>));
    let updates = pending.clone();
    handle
        .insert_source(source, move |(), _, data| {
            if let Some(snapshot) = updates.lock().unwrap().take()
                && snapshot.generation >= data.state.shell.notifications.generation
            {
                data.state.shell.notifications = snapshot;
                data.state.refresh_preferences_status();
                data.state.ui_dirty = true;
                data.state.redraw = true;
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let (sender, receiver) = mpsc::sync_channel(64);
    let shared = Arc::new(Shared {
        store: Mutex::new(Store::default()),
        preferences,
        start: Instant::now(),
        pending,
        ping,
        wake: sender.clone(),
        bus_lost: AtomicBool::new(false),
        stopping: AtomicBool::new(false),
        active_epoch: AtomicU64::new(0),
        generation: AtomicU64::new(0),
        status: Mutex::new(Status::default()),
        wayland_display: state.socket_name.clone(),
        feedback,
    });
    let connection = Arc::new(Mutex::new(None));
    let saved = connection.clone();
    let worker_shared = shared.clone();
    state.shell.notifications.error = Some("Starting notifications".into());
    let worker = std::thread::Builder::new()
        .name("tarot-notifications".into())
        .spawn(move || {
            supervise(worker_shared, receiver, saved);
        })?;
    Ok(Server {
        sender,
        connection,
        worker: Some(worker),
        shared,
    })
}

fn supervise(
    shared: Arc<Shared>,
    receiver: mpsc::Receiver<Event>,
    saved: Arc<Mutex<Option<Connection>>>,
) {
    let settings = shared.preferences.initial_notifications();
    shared.store.lock().unwrap().ui(UiAction::Settings {
        dnd: settings.dnd,
        hide_previews: settings.hide_previews,
    });
    shared.publish();
    let mut failures: u32 = 0;
    let mut epoch = 0;
    while !shared.stopping.load(Ordering::Acquire) {
        epoch += 1;
        shared.bus_lost.store(false, Ordering::Release);
        let started = Instant::now();
        let result = run(shared.clone(), &receiver, saved.clone(), epoch);
        // Setup can fail before run's normal connection/watcher cleanup.
        let connection = saved.lock().unwrap().take();
        if let Some(connection) = connection {
            let _ = connection.close();
        }
        match result {
            Ok(()) => break,
            Err(error) => shared.unavailable(format!("Notification service unavailable: {error}")),
        }
        if started.elapsed() >= Duration::from_secs(30) {
            failures = 0;
        }
        let retry = Instant::now() + Duration::from_millis(250 << failures.min(4));
        failures = failures.saturating_add(1);
        while !shared.stopping.load(Ordering::Acquire) && Instant::now() < retry {
            match receiver.recv_timeout(retry.saturating_duration_since(Instant::now())) {
                Ok(Event::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Ok(Event::Ui(action)) => shared.ui(action),
                Ok(Event::Wake) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

fn run(
    shared: Arc<Shared>,
    receiver: &mpsc::Receiver<Event>,
    saved: Arc<Mutex<Option<Connection>>>,
    epoch: u64,
) -> Result<()> {
    let connection = Builder::session()?
        .method_timeout(Duration::from_secs(3))
        .serve_at(
            PATH,
            Api {
                shared: shared.clone(),
                epoch,
            },
        )?
        .build()?;
    *saved.lock().unwrap() = Some(connection.clone());
    if shared.stopping.load(Ordering::Acquire) {
        let _ = connection.close();
        return Ok(());
    }
    let proxy = DBusProxy::new(&connection)?;
    proxy
        .update_activation_environment(crate::apps::activation_environment(
            &shared.wayland_display,
        )?)
        .context("publishing private session activation environment")?;
    let changes = proxy.receive_name_owner_changed()?;
    let watched = shared.clone();
    let watcher = std::thread::Builder::new()
        .name("notification-owners".into())
        .spawn(move || {
            for signal in changes {
                if let Ok(args) = signal.args()
                    && args.name().as_str().starts_with(':')
                    && args.new_owner().is_none()
                {
                    let changed = watched
                        .store
                        .lock()
                        .unwrap()
                        .owner_gone(args.name().as_str());
                    if changed {
                        watched.changed();
                    }
                }
            }
            watched.bus_lost.store(true, Ordering::Release);
            let _ = watched.wake.try_send(Event::Wake);
        })?;
    let result = (|| -> Result<()> {
        connection
            .request_name_with_flags(NAME, fdo::RequestNameFlags::DoNotQueue.into())
            .context("claiming session notification name")?;
        shared.active_epoch.store(epoch, Ordering::Release);
        {
            let mut status = shared.status.lock().unwrap();
            status.available = true;
            status.error = None;
        }
        shared.publish();
        loop {
            if shared.stopping.load(Ordering::Acquire) {
                break;
            }
            let deadline = shared.store.lock().unwrap().deadline();
            let event = if let Some(deadline) = deadline {
                match receiver.recv_timeout(deadline.saturating_sub(shared.start.elapsed())) {
                    Ok(event) => Some(event),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => Some(Event::Shutdown),
                }
            } else {
                receiver.recv().ok()
            };
            match event {
                Some(Event::Shutdown) => break,
                Some(Event::Ui(action)) => {
                    if let UiAction::Invoke { id, revision, .. } = &action {
                        let owner = shared
                            .store
                            .lock()
                            .unwrap()
                            .action_owner(*id, *revision)
                            .map(str::to_owned);
                        if let Some(owner) = owner
                            && !proxy
                                .name_has_owner(zbus::names::BusName::try_from(owner.as_str())?)?
                        {
                            shared.store.lock().unwrap().owner_gone(&owner);
                        }
                    }
                    shared.ui(action);
                }
                Some(Event::Wake) | None => {}
            }
            if shared.bus_lost.load(Ordering::Acquire) {
                anyhow::bail!("session bus disconnected")
            }
            let mut store = shared.store.lock().unwrap();
            store.tick(shared.start.elapsed());
            let signals = store.signals();
            drop(store);
            shared.publish();
            for signal in signals {
                let result = match signal {
                    Signal::Closed { owner, id, reason } => connection.emit_signal(
                        Some(owner.as_str()),
                        PATH,
                        NAME,
                        "NotificationClosed",
                        &(id, reason),
                    ),
                    Signal::Invoked { owner, id, key } => connection.emit_signal(
                        Some(owner.as_str()),
                        PATH,
                        NAME,
                        "ActionInvoked",
                        &(id, key),
                    ),
                };
                if result.is_err() {
                    tracing::debug!("Notification signal destination unavailable");
                }
            }
        }
        Ok(())
    })();
    shared.active_epoch.store(0, Ordering::Release);
    let _ = connection.close();
    let _ = watcher.join();
    saved.lock().unwrap().take();
    result
}
