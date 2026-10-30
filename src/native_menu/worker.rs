//! One owned foreground-export worker. Signals invalidate choices immediately;
//! main-thread publications contain no transport parameters or addresses.
use super::transport::{self, Authority, Binding, Dispatch, Export, Item, Lease, Snapshot};
use crate::process_identity::Process;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
};
use tarot_core::app_menu::{Kind, Node, Status};
use zbus::blocking::MessageIterator;

pub struct Update {
    pub revision: u64,
    pub status: Status,
    pub nodes: Vec<Node>,
}
enum Command {
    Dirty,
    Activate { revision: u64, id: u64 },
    Stop,
}
struct Shared {
    processed: Mutex<zbus::message::Sequence>,
    progressed: Condvar,
    done: AtomicBool,
    stop: Arc<AtomicBool>,
    cancel: Arc<transport::Cancellation>,
    broken: AtomicBool,
    dirty: AtomicBool,
    revision: AtomicU64,
    authority: Mutex<Option<Authority>>,
    latest: Mutex<Option<Update>>,
    notify: Arc<dyn Fn() + Send + Sync>,
}
impl Shared {
    fn publish(&self, update: Update) {
        *self.latest.lock().unwrap() = Some(update);
        (self.notify)();
    }
    fn invalidate(&self) {
        self.authority.lock().unwrap().take();
        let revision = self.revision.fetch_add(1, Ordering::AcqRel) + 1;
        self.publish(Update {
            revision,
            status: Status::Loading,
            nodes: vec![],
        });
    }
}
pub struct Watch {
    shared: Arc<Shared>,
    commands: mpsc::SyncSender<Command>,
    worker: Option<JoinHandle<()>>,
}
impl Watch {
    pub fn start(
        export: Export,
        peer: Arc<Process>,
        lease: Lease,
        epoch: Arc<Mutex<Option<String>>>,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Shared {
            processed: Mutex::new(Default::default()),
            progressed: Condvar::new(),
            done: AtomicBool::new(false),
            cancel: Arc::new(transport::Cancellation::new(stop.clone())),
            stop,
            broken: AtomicBool::new(false),
            dirty: AtomicBool::new(false),
            revision: AtomicU64::new(1),
            authority: Mutex::new(None),
            latest: Mutex::new(None),
            notify,
        });
        let (send, receive) = mpsc::sync_channel(16);
        let state = shared.clone();
        let signals = send.clone();
        let worker = std::thread::Builder::new()
            .name("tarot-app-menu".into())
            .spawn(move || {
                let result = (|| -> anyhow::Result<()> {
                    let connection =
                        transport::session_connection_cancellable(state.cancel.clone())?;
                    if state.stop.load(Ordering::Acquire) {
                        connection.close()?;
                        return Ok(());
                    }
                    let id_reply = connection.call_method(
                        Some("org.freedesktop.DBus"),
                        "/org/freedesktop/DBus",
                        Some("org.freedesktop.DBus"),
                        "GetId",
                        &(),
                    )?;
                    anyhow::ensure!(
                        id_reply.body().len() <= 4096
                            && id_reply.data().fds().is_empty()
                            && id_reply
                                .header()
                                .sender()
                                .is_some_and(|s| s.as_str() == "org.freedesktop.DBus"),
                        "invalid menu bus identity"
                    );
                    let id: String = id_reply.body().deserialize()?;
                    anyhow::ensure!(
                        id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()),
                        "invalid bus GUID"
                    );
                    {
                        let mut original = epoch.lock().unwrap();
                        if let Some(original) = original.as_ref() {
                            anyhow::ensure!(
                                *original == id,
                                "surface claim belongs to an earlier bus"
                            );
                        } else {
                            *original = Some(id);
                        }
                    }
                    let mut stream = MessageIterator::from(&connection);
                    // Filter broadcasts at the broker before the frame budget
                    // sees unrelated traffic. Keep the unfiltered local stream
                    // so all method replies still advance the ordering barrier.
                    for rule in export.signal_rules()? {
                        connection.call_method(
                            Some("org.freedesktop.DBus"),
                            "/org/freedesktop/DBus",
                            Some("org.freedesktop.DBus"),
                            "AddMatch",
                            &(rule,),
                        )?;
                    }
                    let observed = state.clone();
                    let target = export.clone();
                    let watcher = std::thread::Builder::new()
                        .name("tarot-menu-signals".into())
                        .spawn(move || {
                            for message in &mut stream {
                                if observed.stop.load(Ordering::Acquire) {
                                    break;
                                }
                                let Ok(message) = message else {
                                    observed.broken.store(true, Ordering::Release);
                                    break;
                                };
                                let h = message.header();
                                let owner_lost = h
                                    .sender()
                                    .is_some_and(|s| s.as_str() == "org.freedesktop.DBus")
                                    && h.interface().map(|s| s.as_str())
                                        == Some("org.freedesktop.DBus")
                                    && h.member().map(|s| s.as_str()) == Some("NameOwnerChanged")
                                    && message.body().len() <= 4096
                                    && message.data().fds().is_empty()
                                    && message
                                        .body()
                                        .deserialize::<(String, String, String)>()
                                        .is_ok_and(|(name, old, new)| {
                                            name == target.owner() && old != new
                                        });
                                if owner_lost {
                                    observed.broken.store(true, Ordering::Release);
                                }
                                if owner_lost || target.relevant_signal(&message) {
                                    if message.body().len() > 64 * 1024
                                        || !message.data().fds().is_empty()
                                    {
                                        observed.broken.store(true, Ordering::Release);
                                    }
                                    observed.invalidate();
                                    if !observed.dirty.swap(true, Ordering::AcqRel)
                                        && signals.try_send(Command::Dirty).is_err()
                                    {
                                        observed.broken.store(true, Ordering::Release);
                                    }
                                }
                                *observed.processed.lock().unwrap() = message.recv_position();
                                observed.progressed.notify_all();
                            }
                            observed.progressed.notify_all();
                            if !observed.stop.load(Ordering::Acquire) {
                                observed.broken.store(true, Ordering::Release);
                                observed.invalidate();
                                let _ = signals.try_send(Command::Dirty);
                            }
                        })?;
                    let result = (|| -> anyhow::Result<()> {
                        let ordered = state.clone();
                        let binding = Binding::new(connection.clone(), export, peer, lease)?
                            .with_barrier(Arc::new(move |position| {
                                let last = ordered.processed.lock().unwrap();
                                let (last, _) = ordered
                                    .progressed
                                    .wait_timeout_while(
                                        last,
                                        std::time::Duration::from_millis(500),
                                        |last| {
                                            *last < position
                                                && !ordered.broken.load(Ordering::Acquire)
                                                && !ordered.stop.load(Ordering::Acquire)
                                        },
                                    )
                                    .unwrap();
                                anyhow::ensure!(
                                    *last >= position && !ordered.broken.load(Ordering::Acquire),
                                    "menu signal reader did not reach reply"
                                );
                                Ok(())
                            }));
                        let mut snapshot: Option<Snapshot> = None;
                        let mut paths = BTreeMap::new();
                        let mut generation = None;
                        let mut load = true;
                        loop {
                            if state.stop.load(Ordering::Acquire) {
                                break;
                            }
                            anyhow::ensure!(
                                !state.broken.load(Ordering::Acquire),
                                "menu connection lost or queue overflow"
                            );
                            if load {
                                state.dirty.store(false, Ordering::Release);
                                let revision = state.revision.load(Ordering::Acquire);
                                let candidate = binding.watched_snapshot()?;
                                let mut authority = state.authority.lock().unwrap();
                                if state.revision.load(Ordering::Acquire) == revision {
                                    let mut next = 0;
                                    paths.clear();
                                    let nodes = present(
                                        &candidate.items,
                                        &mut vec![],
                                        &mut next,
                                        &mut paths,
                                    );
                                    let owner = Authority::default();
                                    generation = Some(owner.lease());
                                    *authority = Some(owner);
                                    snapshot = Some(candidate);
                                    state.publish(Update {
                                        revision,
                                        status: Status::Available,
                                        nodes,
                                    });
                                }
                            }
                            load = false;
                            match receive.recv() {
                                Ok(Command::Stop) | Err(_) => break,
                                Ok(Command::Dirty) => load = true,
                                Ok(Command::Activate { revision, id }) => {
                                    if revision != state.revision.load(Ordering::Acquire) {
                                        continue;
                                    }
                                    if let (Some(snapshot), Some(path), Some(guard)) =
                                        (&snapshot, paths.get(&id), &generation)
                                        && guard.is_active()
                                    {
                                        match binding.activate_guarded(snapshot, path, guard) {
                                            Ok(Dispatch::Replied) => {}
                                            Ok(Dispatch::Uncertain) => {
                                                state.authority.lock().unwrap().take();
                                                state.publish(Update {
                                                    revision,
                                                    status: Status::Uncertain,
                                                    nodes: vec![],
                                                });
                                            }
                                            Err(_) => {
                                                state.invalidate();
                                                load = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Ok(())
                    })();
                    let _ = connection.close();
                    let _ = watcher.join();
                    result
                })();
                if result.is_err() && !state.stop.load(Ordering::Acquire) {
                    state.authority.lock().unwrap().take();
                    let revision = state.revision.fetch_add(1, Ordering::AcqRel) + 1;
                    state.publish(Update {
                        revision,
                        status: Status::Unavailable,
                        nodes: vec![],
                    });
                }
                state.done.store(true, Ordering::Release);
                (state.notify)();
            })?;
        Ok(Self {
            shared,
            commands: send,
            worker: Some(worker),
        })
    }
    pub fn take(&self) -> Option<Update> {
        let update = self.shared.latest.lock().unwrap().take()?;
        let revision = self.shared.revision.load(Ordering::Acquire);
        Some(if update.revision == revision {
            update
        } else {
            Update {
                revision,
                status: Status::Loading,
                nodes: vec![],
            }
        })
    }
    pub fn activate(&self, revision: u64, id: u64) -> bool {
        revision == self.shared.revision.load(Ordering::Acquire)
            && !self.shared.stop.load(Ordering::Acquire)
            && self
                .commands
                .try_send(Command::Activate { revision, id })
                .is_ok()
    }
    pub fn stop(&self) {
        self.shared.cancel.cancel();
        self.shared.authority.lock().unwrap().take();
        let _ = self.commands.try_send(Command::Stop);
    }
    pub fn finished(&self) -> bool {
        self.shared.done.load(Ordering::Acquire)
    }
}
impl Drop for Watch {
    fn drop(&mut self) {
        self.stop();
        if self.worker.as_ref().is_some_and(JoinHandle::is_finished)
            && let Some(worker) = self.worker.take()
        {
            let _ = worker.join();
        }
    }
}
fn label(value: &str) -> String {
    let mut result = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '_' {
            if let Some(next) = chars.next() {
                result.push(next);
            }
        } else {
            result.push(ch);
        }
    }
    result
}
fn present(
    items: &[Item],
    path: &mut Vec<usize>,
    next: &mut u64,
    paths: &mut BTreeMap<u64, Vec<usize>>,
) -> Vec<Node> {
    items
        .iter()
        .enumerate()
        .map(|(at, item)| {
            path.push(at);
            *next += 1;
            let id = *next;
            let node =
                match item {
                    Item::Action {
                        label: text,
                        action,
                    } => {
                        paths.insert(id, path.clone());
                        Node {
                            id,
                            label: label(text),
                            enabled: action.enabled
                                && action.supported
                                && !label(text).trim().is_empty(),
                            checked: action.state.as_ref().is_some_and(|state| {
                                action.target.as_ref().map_or(
                                    matches!(state, transport::Scalar::Bool(true)),
                                    |target| target == state,
                                )
                            }),
                            kind: Kind::Action,
                        }
                    }
                    Item::Submenu { label: text, items } => Node {
                        id,
                        label: label(text),
                        enabled: !items.is_empty(),
                        checked: false,
                        kind: Kind::Submenu(present(items, path, next, paths)),
                    },
                    Item::Section { label: text, items } => Node {
                        id,
                        label: text.as_deref().map(label).unwrap_or_default(),
                        enabled: false,
                        checked: false,
                        kind: Kind::Section(present(items, path, next, paths)),
                    },
                    Item::Unavailable { label: text } => Node {
                        id,
                        label: label(text),
                        enabled: false,
                        checked: false,
                        kind: Kind::Action,
                    },
                };
            path.pop();
            node
        })
        .collect()
}
