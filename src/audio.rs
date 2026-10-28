//! Event-driven output controls. Only the worker calls libpulse; UI gets snapshots.
mod pulse;
use crate::{CalloopData, Compositor};
use anyhow::{Result, bail, ensure};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot_core::audio::{Output, Snapshot, UiAction};

enum Event {
    Wake,
    Action {
        action: UiAction,
        default_epoch: Option<u64>,
    },
    Stop,
}
struct Shared {
    pending: Mutex<Option<Snapshot>>,
    latest: Mutex<Snapshot>,
    sender: mpsc::SyncSender<Event>,
    ping: Ping,
    stop: AtomicBool,
    dirty: AtomicBool,
    busy: AtomicBool,
    requested: Mutex<Option<UiAction>>,
}
impl Shared {
    fn finish(&self) {
        *self.requested.lock().unwrap() = None;
        self.busy.store(false, Ordering::Release);
    }
    fn wake(&self) {
        if !self.dirty.swap(true, Ordering::AcqRel) {
            let _ = self.sender.try_send(Event::Wake);
        }
    }
    fn publish(&self, value: Snapshot) {
        self.publish_result(value, false);
    }
    fn publish_result(&self, mut value: Snapshot, force: bool) {
        value.enabled = true;
        value.pending = self.requested.lock().unwrap().clone();
        let mut latest = self.latest.lock().unwrap();
        value.generation = latest.generation;
        if *latest == value && !force {
            return;
        }
        value.generation += 1;
        *latest = value.clone();
        *self.pending.lock().unwrap() = Some(value);
        self.ping.ping();
    }
}
pub struct Server {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn action(&self, action: UiAction) -> Result<()> {
        self.send_action(action, None)
    }
    pub fn default_action(&self, action: UiAction, epoch: u64) -> Result<()> {
        self.send_action(action, Some(epoch))
    }
    pub fn busy(&self) -> bool {
        self.shared.busy.load(Ordering::Acquire)
    }
    fn send_action(&self, action: UiAction, default_epoch: Option<u64>) -> Result<()> {
        ensure!(action.valid(), "Invalid audio control");
        let latest = self.shared.latest.lock().unwrap();
        ensure!(latest.available, "Audio service unavailable");
        ensure!(
            default_epoch
                .is_none_or(|epoch| latest.default_identity() == Some((epoch, action.output()))),
            "Default audio output changed"
        );
        ensure!(
            latest.outputs.iter().any(|o| o.id == action.output()),
            "Audio output is no longer available"
        );
        ensure!(
            !self.shared.busy.swap(true, Ordering::AcqRel),
            "Audio change is still pending"
        );
        *self.shared.requested.lock().unwrap() = Some(action.clone());
        if self
            .shared
            .sender
            .try_send(Event::Action {
                action,
                default_epoch,
            })
            .is_err()
        {
            self.shared.finish();
            bail!("Audio controls are busy or unavailable");
        }
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        let _ = self.shared.sender.try_send(Event::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
    address: Option<String>,
) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let (sender, receiver) = mpsc::sync_channel(64);
    let shared = Arc::new(Shared {
        pending: Mutex::new(None),
        latest: Mutex::new(Snapshot::default()),
        sender,
        ping,
        stop: AtomicBool::new(false),
        dirty: AtomicBool::new(false),
        busy: AtomicBool::new(false),
        requested: Mutex::new(None),
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |(), _, data| {
            if let Some(snapshot) = updates.pending.lock().unwrap().take() {
                let mut snapshot = snapshot;
                snapshot.pending = updates.requested.lock().unwrap().clone();
                data.state.shell.audio = snapshot;
                data.state.ui_dirty = true;
                data.state.redraw = true;
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    state.shell.audio.enabled = true;
    let worker_shared = shared.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-audio".into())
        .spawn(move || supervise(worker_shared, receiver, address))?;
    Ok(Server {
        shared,
        worker: Some(worker),
    })
}
struct Session {
    ids: HashMap<u32, (std::ffi::CString, u64)>,
    sinks: HashMap<u64, pulse::Sink>,
    snapshot: Snapshot,
    pending: Option<(UiAction, Instant)>,
}
impl Session {
    fn refresh(&mut self, connection: &pulse::Connection, next_id: &mut u64) -> Result<()> {
        let (sinks, default) = connection.snapshot()?;
        let mut ids = HashMap::new();
        let mut actual = HashMap::new();
        let mut outputs = Vec::new();
        for sink in sinks {
            let id = match self.ids.get(&sink.index) {
                Some((name, id)) if *name == sink.name => *id,
                _ => {
                    *next_id = next_id
                        .checked_add(1)
                        .ok_or_else(|| anyhow::anyhow!("Audio identity exhausted"))?;
                    *next_id
                }
            };
            outputs.push(Output {
                id,
                label: sink.label.clone(),
                percent: sink.percent(),
                muted: sink.muted,
                default: default.as_ref() == Some(&sink.name),
            });
            ids.insert(sink.index, (sink.name.clone(), id));
            actual.insert(id, sink);
        }
        outputs.sort_by_key(|o| (!o.default, o.id));
        self.ids = ids;
        self.sinks = actual;
        self.snapshot.outputs = outputs;
        self.snapshot.available = true;
        if let Some((action, deadline)) = &self.pending {
            if action.confirmed(&self.snapshot) {
                self.pending = None;
                self.snapshot.action_error = None;
            } else if !self.sinks.contains_key(&action.output()) {
                self.pending = None;
                self.snapshot.action_error = Some("Audio output disappeared".into());
            } else if Instant::now() >= *deadline {
                self.pending = None;
                self.snapshot.action_error = Some("Audio change was not confirmed".into());
            }
        }
        self.snapshot.pending = self.pending.as_ref().map(|(a, _)| a.clone());
        Ok(())
    }
}
fn supervise(shared: Arc<Shared>, receiver: mpsc::Receiver<Event>, address: Option<String>) {
    let mut epoch = 0;
    let mut next_id = 0;
    let mut failures = 0u32;
    while !shared.stop.load(Ordering::Acquire) {
        epoch += 1;
        shared.dirty.store(false, Ordering::Release);
        let started = Instant::now();
        let result = run(&shared, &receiver, address.as_deref(), epoch, &mut next_id);
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        shared.finish();
        shared.publish(Snapshot {
            epoch,
            error: Some("Audio service unavailable".into()),
            action_error: result.err().map(|e| e.to_string()),
            ..Default::default()
        });
        if started.elapsed() >= Duration::from_secs(30) {
            failures = 0;
        }
        let deadline = Instant::now() + Duration::from_millis(250 << failures.min(4));
        failures = failures.saturating_add(1);
        while !shared.stop.load(Ordering::Acquire) && Instant::now() < deadline {
            match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Event::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Ok(Event::Action { .. }) => {
                    shared.finish();
                }
                Ok(Event::Wake) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
}
fn run(
    shared: &Arc<Shared>,
    receiver: &mpsc::Receiver<Event>,
    address: Option<&str>,
    epoch: u64,
    next_id: &mut u64,
) -> Result<()> {
    let connection = pulse::Connection::connect(shared.clone(), address)?;
    let mut session = Session {
        ids: HashMap::new(),
        sinks: HashMap::new(),
        snapshot: Snapshot {
            epoch,
            ..Default::default()
        },
        pending: None,
    };
    session.refresh(&connection, next_id)?;
    shared.publish(session.snapshot.clone());
    loop {
        if shared.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        if shared.dirty.swap(false, Ordering::AcqRel) {
            ensure!(connection.ready(), "Audio service disconnected");
            let was_pending = session.pending.is_some();
            session.refresh(&connection, next_id)?;
            if was_pending && session.pending.is_none() {
                shared.finish();
            }
            shared.publish(session.snapshot.clone());
        }
        let event = if let Some((_, deadline)) = &session.pending {
            receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        } else {
            receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        match event {
            Ok(Event::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Ok(Event::Wake) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                session.refresh(&connection, next_id)?;
                if session.pending.is_none() {
                    shared.finish();
                }
                shared.publish(session.snapshot.clone());
            }
            Ok(Event::Action {
                action,
                default_epoch,
            }) => {
                shared.dirty.store(false, Ordering::Release);
                // Re-query immediately before dispatch: a removed/replaced output
                // must never inherit an action for an earlier snapshot identity.
                session.refresh(&connection, next_id)?;
                let result = if default_epoch.is_some_and(|e| {
                    session.snapshot.default_identity() != Some((e, action.output()))
                }) {
                    Err(anyhow::anyhow!("Default audio output changed"))
                } else if let Some(sink) = session.sinks.get(&action.output()) {
                    match action {
                        UiAction::Volume { percent, .. } => connection.volume(sink, percent),
                        UiAction::Mute { muted, .. } => connection.mute(sink, muted),
                        UiAction::Default { .. } => connection.default(sink),
                    }
                } else {
                    Err(anyhow::anyhow!("Audio output is no longer available"))
                };
                match result {
                    Ok(()) => {
                        session.pending = Some((action, Instant::now() + Duration::from_secs(3)));
                        session.snapshot.action_error = None;
                    }
                    Err(error) => {
                        session.snapshot.action_error = Some(error.to_string());
                        session.pending = None;
                    }
                }
                session.refresh(&connection, next_id)?;
                if session.pending.is_none() {
                    shared.finish();
                }
                // An already-current target may leave every server field
                // unchanged. Still retire the compositor's local reservation.
                shared.publish_result(session.snapshot.clone(), true);
            }
        }
    }
}
