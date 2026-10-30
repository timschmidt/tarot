//! Session feedbackd adapter. Only static events and opaque identities cross the
//! service boundary; notification content and arbitrary sound files never do.
use crate::{CalloopData, Compositor, preferences};
use anyhow::{Context, Result, bail, ensure};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot_core::feedback::{Key, Mailbox, Profile, Snapshot, TestResult, Ticket, UiAction};
use zbus::{
    blocking::{Connection, MessageIterator, Proxy, connection::Builder, fdo::DBusProxy},
    zvariant::{OwnedValue, Value},
};

const NAME: &str = "org.sigxcpu.Feedback";
const PATH: &str = "/org/sigxcpu/Feedback";
const TIMEOUT: Duration = Duration::from_secs(3);

enum Event {
    Wake,
    Profile {
        epoch: u64,
        profile: Profile,
    },
    Ended {
        owner: String,
        id: u32,
        generation: u64,
        reason: u32,
    },
}
struct Shared {
    mailbox: Mutex<Mailbox>,
    latest: Mutex<Snapshot>,
    updates: Mutex<Option<Snapshot>>,
    preferences: preferences::Handle,
    wayland_display: std::ffi::OsString,
    sender: mpsc::SyncSender<Event>,
    ping: Ping,
    available: AtomicBool,
    busy: AtomicBool,
    wake_queued: AtomicBool,
    owner_events: AtomicU64,
    lost: AtomicBool,
    stop: AtomicBool,
}
impl Shared {
    fn wake(&self) {
        if !self.wake_queued.swap(true, Ordering::AcqRel) {
            let _ = self.sender.try_send(Event::Wake);
        }
    }
    fn publish(&self, mut value: Snapshot) {
        value.enabled = true;
        value.notifications_enabled = self.mailbox.lock().unwrap().notifications_enabled();
        let settings = self.preferences.status();
        value.settings_available = true;
        value.settings_saved = settings.saved;
        value.settings_error = settings.error;
        let mut latest = self.latest.lock().unwrap();
        value.generation = latest.generation;
        if *latest == value {
            return;
        }
        value.generation = value.generation.saturating_add(1);
        *latest = value.clone();
        *self.updates.lock().unwrap() = Some(value);
        self.ping.ping();
    }
}

#[derive(Clone)]
pub struct Handle(Arc<Shared>);
impl Handle {
    pub fn notify(&self, id: u32, revision: u64, urgency: u8, profile: Option<Profile>) {
        if self.0.available.load(Ordering::Acquire) {
            self.0
                .mailbox
                .lock()
                .unwrap()
                .notify(id, revision, urgency, profile);
            self.0.wake();
        }
    }
    pub fn retain(&self, dnd: bool, live: &[(u32, u64)]) {
        self.0.mailbox.lock().unwrap().retain(dnd, live);
        self.0.wake();
    }
    pub fn stop_test(&self) {
        self.0.mailbox.lock().unwrap().stop_test();
        self.0.wake();
    }
}
pub struct Server {
    pub handle: Handle,
    connection: Arc<Mutex<Option<Connection>>>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn action(&self, action: UiAction) -> Result<()> {
        let shared = &self.handle.0;
        match action {
            UiAction::Notifications { enabled } => {
                shared.mailbox.lock().unwrap().settings(enabled);
                shared.preferences.feedback(tarot_core::feedback::Settings {
                    notifications: enabled,
                });
                shared.wake();
            }
            UiAction::StopTest => self.handle.stop_test(),
            UiAction::Test { epoch } => {
                let snapshot = shared.latest.lock().unwrap();
                ensure!(
                    shared.available.load(Ordering::Acquire)
                        && snapshot.available
                        && snapshot.epoch == epoch
                        && !shared.busy.load(Ordering::Acquire),
                    "Feedback service changed or is busy"
                );
                ensure!(
                    shared.mailbox.lock().unwrap().test().is_some(),
                    "A feedback test is already pending"
                );
                shared.wake();
            }
            UiAction::Profile { epoch, profile } => {
                {
                    let snapshot = shared.latest.lock().unwrap();
                    ensure!(
                        shared.available.load(Ordering::Acquire)
                            && snapshot.available
                            && snapshot.epoch == epoch,
                        "Feedback service changed"
                    );
                }
                ensure!(
                    shared
                        .busy
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok(),
                    "Feedback change is still pending"
                );
                if shared
                    .sender
                    .try_send(Event::Profile { epoch, profile })
                    .is_err()
                {
                    shared.busy.store(false, Ordering::Release);
                    bail!("Feedback service is busy");
                }
            }
        }
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let shared = &self.handle.0;
        shared.stop.store(true, Ordering::Release);
        shared.available.store(false, Ordering::Release);
        shared.mailbox.lock().unwrap().clear();
        shared.wake();
        // Allow normal EndFeedback cleanup, then close the connection to abort
        // an unresponsive peer. No daemon call can hold session teardown open.
        let deadline = Instant::now() + Duration::from_secs(1);
        if let Some(worker) = &self.worker {
            while !worker.is_finished() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        if let Some(c) = self.connection.lock().unwrap().take() {
            let _ = c.close();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
    preferences: preferences::Handle,
) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let (sender, receiver) = mpsc::sync_channel(128);
    let settings = preferences.initial_feedback();
    let dnd = preferences.initial_notifications().dnd;
    let shared = Arc::new(Shared {
        mailbox: Mutex::new(Mailbox::new(settings.notifications, dnd)),
        latest: Mutex::new(Snapshot::default()),
        updates: Mutex::new(None),
        preferences,
        wayland_display: state.socket_name.clone(),
        sender,
        ping,
        available: AtomicBool::new(false),
        busy: AtomicBool::new(false),
        wake_queued: AtomicBool::new(false),
        owner_events: AtomicU64::new(0),
        lost: AtomicBool::new(false),
        stop: AtomicBool::new(false),
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |(), _, data| {
            let value = { updates.updates.lock().unwrap().take() };
            if let Some(value) = value {
                data.state.shell.feedback = value;
                data.state.refresh_preferences_status();
                data.state.ui_dirty = true;
                data.state.redraw = true;
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state.shell.feedback.enabled = true;
    let connection = Arc::new(Mutex::new(None));
    let saved = connection.clone();
    let worker_shared = shared.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-feedback".into())
        .spawn(move || {
            let mut epoch = 0u64;
            while !worker_shared.stop.load(Ordering::Acquire) {
                worker_shared.lost.store(false, Ordering::Release);
                let _ = run(&worker_shared, &receiver, &saved, &mut epoch);
                worker_shared.available.store(false, Ordering::Release);
                worker_shared.busy.store(false, Ordering::Release);
                worker_shared.mailbox.lock().unwrap().clear();
                if let Some(c) = saved.lock().unwrap().take() {
                    let _ = c.close();
                }
                worker_shared.publish(Snapshot {
                    epoch,
                    error: Some("Feedback service unavailable; reconnecting".into()),
                    ..Default::default()
                });
                let until = Instant::now() + Duration::from_secs(2);
                while !worker_shared.stop.load(Ordering::Acquire) && Instant::now() < until {
                    match receiver.recv_timeout(until.saturating_duration_since(Instant::now())) {
                        Ok(Event::Profile { .. }) => {
                            worker_shared.busy.store(false, Ordering::Release)
                        }
                        Ok(Event::Wake) => {
                            worker_shared.wake_queued.store(false, Ordering::Release)
                        }
                        _ => {}
                    }
                }
            }
        })?;
    Ok(Server {
        handle: Handle(shared),
        connection,
        worker: Some(worker),
    })
}

struct Active {
    ticket: Ticket,
    deadline: Instant,
}
#[derive(Default)]
struct Observed {
    processed: Mutex<zbus::message::Sequence>,
    progressed: Condvar,
}
impl Observed {
    fn ordered(&self, shared: &Shared, message: &zbus::Message) -> Result<()> {
        let position = message.recv_position();
        let last = self.processed.lock().unwrap();
        let (last, _) = self
            .progressed
            .wait_timeout_while(last, Duration::from_millis(500), |last| {
                *last < position && !shared.lost.load(Ordering::Acquire)
            })
            .unwrap();
        ensure!(
            *last >= position && !shared.lost.load(Ordering::Acquire),
            "Feedback signal reader did not reach reply"
        );
        Ok(())
    }
}
fn watch(shared: Arc<Shared>, connection: &Connection) -> Result<(Arc<Observed>, JoinHandle<()>)> {
    // One local stream preserves order across exporter and broker signals.
    // Exact broker matches exclude unrelated broadcasts while replies advance
    // the barrier used before any endpoint or event ID is accepted.
    let messages = MessageIterator::from(connection);
    for rule in [
        "type='signal',sender='org.sigxcpu.Feedback',path='/org/sigxcpu/Feedback'",
        "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='org.sigxcpu.Feedback'",
    ] {
        connection.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "AddMatch",
            &(rule,),
        )?;
    }
    let observed = Arc::new(Observed::default());
    let progress = observed.clone();
    let thread = std::thread::Builder::new()
        .name("feedback-events".into())
        .spawn(move || {
            for message in messages {
                let Ok(message) = message else {
                    break;
                };
                let h = message.header();
                if h.message_type() == zbus::message::Type::Signal {
                    let owner = h
                        .sender()
                        .is_some_and(|s| s.as_str() == "org.freedesktop.DBus")
                        && h.interface()
                            .is_some_and(|i| i.as_str() == "org.freedesktop.DBus")
                        && h.member().is_some_and(|m| m.as_str() == "NameOwnerChanged");
                    let feedback = h.path().is_some_and(|p| p.as_str() == PATH)
                        && (h.interface().is_some_and(|i| i.as_str() == NAME)
                            || h.interface()
                                .is_some_and(|i| i.as_str() == "org.freedesktop.DBus.Properties"));
                    if (owner || feedback)
                        && (message.body().len() > 4096 || !message.data().fds().is_empty())
                    {
                        break;
                    }
                    if owner
                        && message
                            .body()
                            .deserialize::<(String, String, String)>()
                            .is_ok_and(|(name, old, new)| name == NAME && old != new)
                    {
                        shared.available.store(false, Ordering::Release);
                        shared.owner_events.fetch_add(1, Ordering::AcqRel);
                        shared.mailbox.lock().unwrap().clear();
                        shared.wake();
                    } else if feedback {
                        if h.interface().is_some_and(|i| i.as_str() == NAME)
                            && h.member().is_some_and(|m| m.as_str() == "FeedbackEnded")
                        {
                            if let (Some(source), Ok((id, reason))) =
                                (h.sender(), message.body().deserialize::<(u32, u32)>())
                            {
                                let _ = shared.sender.try_send(Event::Ended {
                                    owner: source.to_string(),
                                    id,
                                    generation: shared.owner_events.load(Ordering::Acquire),
                                    reason,
                                });
                            }
                        } else {
                            shared.wake();
                        }
                    }
                }
                *progress.processed.lock().unwrap() = message.recv_position();
                progress.progressed.notify_all();
            }
            shared.lost.store(true, Ordering::Release);
            progress.progressed.notify_all();
            shared.wake();
        })?;
    Ok((observed, thread))
}
fn rpc<B: serde::Serialize + zbus::zvariant::DynamicType>(
    connection: &Connection,
    observed: &Observed,
    shared: &Shared,
    owner: &str,
    interface: &str,
    method: &str,
    body: &B,
) -> Result<zbus::Message> {
    let path = if owner == "org.freedesktop.DBus" {
        "/org/freedesktop/DBus"
    } else {
        PATH
    };
    let reply = connection.call_method(Some(owner), path, Some(interface), method, body)?;
    observed.ordered(shared, &reply)?;
    ensure!(
        reply.body().len() <= 4096
            && reply.data().fds().is_empty()
            && reply.header().sender().is_some_and(|s| s.as_str() == owner),
        "Invalid feedback service reply"
    );
    Ok(reply)
}
fn current_owner(connection: &Connection, observed: &Observed, shared: &Shared) -> Result<String> {
    let reply = rpc(
        connection,
        observed,
        shared,
        "org.freedesktop.DBus",
        "org.freedesktop.DBus",
        "GetNameOwner",
        &(NAME,),
    )?;
    let owner: String = reply.body().deserialize()?;
    ensure!(
        owner.starts_with(':') && owner.len() <= 255,
        "Invalid feedback exporter"
    );
    Ok(owner)
}

fn run(
    shared: &Arc<Shared>,
    receiver: &mpsc::Receiver<Event>,
    saved: &Arc<Mutex<Option<Connection>>>,
    epoch: &mut u64,
) -> Result<()> {
    let connection = Builder::session()?
        .max_queued(128)
        .method_timeout(TIMEOUT)
        .build()?;
    *saved.lock().unwrap() = Some(connection.clone());
    if shared.stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let (observed, watcher) = watch(shared.clone(), &connection)?;
    let mut active = HashMap::<u32, Active>::new();
    let mut owner = String::new();
    let mut owner_events = shared.owner_events.load(Ordering::Acquire);
    let result = (|| -> Result<()> {
        let broker = DBusProxy::new(&connection)?;
        broker.update_activation_environment(crate::apps::activation_environment(
            &shared.wayland_display,
        )?)?;
        let mut pending: Option<(Profile, Instant)> = None;
        let mut action_error = None;
        let mut test_result = None;
        let mut event = None;
        loop {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            ensure!(
                !shared.lost.load(Ordering::Acquire),
                "Feedback bus disconnected"
            );
            let found = current_owner(&connection, &observed, shared);
            if found.is_err() {
                let _ = broker.start_service_by_name(NAME.try_into()?, 0);
            }
            let fresh = current_owner(&connection, &observed, shared).unwrap_or_default();
            let observed_owner_events = shared.owner_events.load(Ordering::Acquire);
            if fresh != owner || observed_owner_events != owner_events {
                shared.available.store(false, Ordering::Release);
                shared.mailbox.lock().unwrap().clear();
                active.clear();
                pending = None;
                shared.busy.store(false, Ordering::Release);
                *epoch = epoch.checked_add(1).context("Feedback epoch exhausted")?;
                owner = fresh;
                owner_events = observed_owner_events;
                action_error = None;
                test_result = None;
            }
            let profile = if owner.is_empty() {
                None
            } else {
                ensure!(
                    broker.get_connection_unix_user(owner.as_str().try_into()?)?
                        == rustix::process::getuid().as_raw(),
                    "Feedback exporter has another UID"
                );
                let reply = rpc(
                    &connection,
                    &observed,
                    shared,
                    &owner,
                    "org.freedesktop.DBus.Properties",
                    "Get",
                    &(NAME, "Profile"),
                )?;
                let value: OwnedValue = reply.body().deserialize()?;
                let text = String::try_from(value)?;
                Profile::parse(&text)
            };
            if shared.owner_events.load(Ordering::Acquire) != owner_events {
                continue;
            }
            if profile.is_none() {
                shared.mailbox.lock().unwrap().clear();
            }
            shared
                .available
                .store(!owner.is_empty() && profile.is_some(), Ordering::Release);
            if let Some(Event::Ended {
                owner: source,
                id,
                generation,
                reason,
            }) = event.take()
                && source == owner
                && generation == owner_events
                && let Some(finished) = active.remove(&id)
            {
                shared.mailbox.lock().unwrap().finish(finished.ticket);
                if finished.ticket.key == Key::Test {
                    test_result = match reason {
                        u32::MAX => Some(TestResult::Unavailable),
                        0..=2 => Some(TestResult::Ended),
                        _ => {
                            action_error = Some("Feedback test returned an unknown result".into());
                            None
                        }
                    };
                }
            }
            if let Some((target, deadline)) = pending
                && (profile == Some(target) || Instant::now() >= deadline)
            {
                if profile != Some(target) {
                    action_error = Some("Feedback profile change was not observed".into());
                }
                pending = None;
                shared.busy.store(false, Ordering::Release);
            }
            let retired: Vec<u32> = {
                let mailbox = shared.mailbox.lock().unwrap();
                active
                    .iter()
                    .filter(|(_, a)| !mailbox.current(a.ticket) || Instant::now() >= a.deadline)
                    .map(|(id, _)| *id)
                    .collect()
            };
            for id in &retired {
                if let Some(old) = active.remove(id) {
                    shared.mailbox.lock().unwrap().finish(old.ticket);
                    if old.ticket.key == Key::Test {
                        test_result = Some(TestResult::Ended);
                    }
                }
            }
            if !owner.is_empty() && end_all(&connection, &owner, &retired).is_err() {
                action_error = Some("Feedback cancellation was not confirmed".into());
                test_result = None;
            }
            if !owner.is_empty() {
                let ticket = { shared.mailbox.lock().unwrap().take() };
                let ticket = ticket.filter(|ticket| {
                    !shared.stop.load(Ordering::Acquire)
                        && shared.available.load(Ordering::Acquire)
                        && shared.owner_events.load(Ordering::Acquire) == owner_events
                        && shared.mailbox.lock().unwrap().current(*ticket)
                });
                if let Some(ticket) = ticket {
                    if ticket.key == Key::Test {
                        test_result = None;
                    }
                    let seconds = if ticket.key == Key::Test { 1 } else { 5 };
                    let name = if ticket.key == Key::Test {
                        "button-pressed"
                    } else {
                        "message-new-instant"
                    };
                    let reply: Result<u32> = rpc(
                        &connection,
                        &observed,
                        shared,
                        &owner,
                        NAME,
                        "TriggerFeedback",
                        &(
                            "org.tarot.Shell",
                            name,
                            HashMap::from([("profile", Value::from(ticket.profile.name()))]),
                            -1i32, // Play once; positive feedbackd timeouts repeat the event.
                        ),
                    )
                    .and_then(|reply| reply.body().deserialize().map_err(Into::into));
                    match reply {
                        Ok(id) if id != 0 => {
                            let current = { shared.mailbox.lock().unwrap().current(ticket) };
                            if current {
                                active.insert(
                                    id,
                                    Active {
                                        ticket,
                                        deadline: Instant::now()
                                            + Duration::from_secs(seconds as u64 + 1),
                                    },
                                );
                            } else {
                                let _ = end(&connection, &owner, id);
                            }
                        }
                        _ => {
                            shared.mailbox.lock().unwrap().finish(ticket);
                            action_error = Some("Feedback request did not complete".into());
                        }
                    }
                }
            }
            if shared.mailbox.lock().unwrap().has_pending() {
                shared.wake();
            }
            let snapshot = Snapshot {
                available: shared.available.load(Ordering::Acquire),
                epoch: *epoch,
                profile,
                pending: pending.map(|p| p.0),
                test_active: active.values().any(|a| a.ticket.key == Key::Test),
                test_result,
                active_notifications: active
                    .values()
                    .filter(|a| matches!(a.ticket.key, Key::Notification(_)))
                    .count(),
                error: if owner.is_empty() {
                    Some("Feedback service unavailable".into())
                } else if profile.is_none() {
                    Some("Feedback service reported an unsupported profile".into())
                } else {
                    None
                },
                action_error: action_error.clone(),
                ..Default::default()
            };
            shared.publish(snapshot);
            let mut deadline = pending.map(|p| p.1);
            for a in active.values() {
                deadline = Some(deadline.map_or(a.deadline, |d| d.min(a.deadline)));
            }
            if owner.is_empty() {
                deadline = Some(Instant::now() + Duration::from_secs(4));
            }
            event = match deadline {
                Some(until) => receiver
                    .recv_timeout(until.saturating_duration_since(Instant::now()))
                    .ok(),
                None => receiver.recv().ok(),
            };
            if matches!(event, Some(Event::Profile { .. })) {
                let Some(Event::Profile {
                    epoch: requested,
                    profile: target,
                }) = event.take()
                else {
                    unreachable!()
                };
                action_error = None;
                test_result = None;
                let same_owner =
                    current_owner(&connection, &observed, shared).is_ok_and(|n| n == owner);
                if requested != *epoch
                    || owner.is_empty()
                    || !same_owner
                    || shared.owner_events.load(Ordering::Acquire) != owner_events
                {
                    shared.busy.store(false, Ordering::Release);
                    action_error = Some("Feedback service changed; choose again".into());
                } else {
                    // Publish the operation before calling the daemon. Even a
                    // same-value selection needs a completion update to retire
                    // the compositor's immediate pending indicator.
                    let mut value = shared.latest.lock().unwrap().clone();
                    value.pending = Some(target);
                    value.action_error = None;
                    value.test_result = None;
                    shared.publish(value);
                    let changed = rpc(
                        &connection,
                        &observed,
                        shared,
                        &owner,
                        "org.freedesktop.DBus.Properties",
                        "Set",
                        &(NAME, "Profile", Value::from(target.name())),
                    );
                    if changed.is_ok() {
                        pending = Some((target, Instant::now() + TIMEOUT));
                    } else {
                        shared.busy.store(false, Ordering::Release);
                        action_error = Some("Feedback profile change failed".into());
                    }
                }
            }
            if matches!(event, Some(Event::Wake)) {
                shared.wake_queued.store(false, Ordering::Release);
            }
        }
    })();
    shared.available.store(false, Ordering::Release);
    shared.mailbox.lock().unwrap().clear();
    if result.is_ok() {
        let _ = end_all(
            &connection,
            &owner,
            &active.keys().copied().collect::<Vec<_>>(),
        );
    }
    let _ = connection.close();
    let _ = watcher.join();
    result
}
fn end(connection: &Connection, owner: &str, id: u32) -> Result<()> {
    Proxy::new(connection, owner, PATH, NAME)?
        .call::<_, _, ()>("EndFeedback", &(id,))
        .map_err(Into::into)
}
/// Stop a bounded batch concurrently so a bad peer cannot multiply the timeout
/// by the number of active notifications. No IDs cross an exporter epoch.
fn end_all(connection: &Connection, owner: &str, ids: &[u32]) -> Result<()> {
    use std::{
        future::{Future, poll_fn},
        pin::Pin,
        task::Poll,
    };
    if ids.is_empty() {
        return Ok(());
    }
    let mut calls: Vec<_> = ids
        .iter()
        .map(|id| {
            Some(Box::pin(async move {
                connection
                    .inner()
                    .call_method(Some(owner), PATH, Some(NAME), "EndFeedback", &(id,))
                    .await
            }))
        })
        .collect();
    let mut timer = std::pin::pin!(async_io::Timer::after(TIMEOUT));
    let mut failed = false;
    async_io::block_on(poll_fn(|cx| {
        if timer.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Err(anyhow::anyhow!("Feedback stop timed out")));
        }
        for call in &mut calls {
            if let Some(pending) = call
                && let Poll::Ready(result) = Pin::as_mut(pending).poll(cx)
            {
                failed |= result.is_err();
                *call = None;
            }
        }
        if calls.iter().all(Option::is_none) {
            Poll::Ready(if failed {
                Err(anyhow::anyhow!("Feedback stop failed"))
            } else {
                Ok(())
            })
        } else {
            Poll::Pending
        }
    }))
}
