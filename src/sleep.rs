//! login1 sleep-delay ownership. Bus I/O stays off the compositor thread; the
//! owned inhibitor is closed only by a matching protected presentation callback.
use crate::{CalloopData, Compositor};
use anyhow::{Context, Result, bail};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    collections::VecDeque,
    os::fd::OwnedFd,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot_core::sleep::{Barrier, Gate, Purpose};
use zbus::blocking::{Connection, MessageIterator, Proxy, connection::Builder};

const LOGIN: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";

#[derive(Default, serde::Serialize)]
pub struct State {
    pub enabled: bool,
    pub available: bool,
    pub gate: Gate,
    pub error: Option<String>,
    pub physical_presentation: bool,
    pub initializing: bool,
    pub rearming: bool,
    #[serde(skip)]
    inhibitor: Option<OwnedFd>,
}
impl State {
    pub fn blocks_unlock(&self) -> bool {
        self.enabled && (self.initializing || self.rearming || self.gate.blocks_unlock())
    }
    pub fn armed(&self) -> bool {
        self.inhibitor.is_some()
    }
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({"enabled":self.enabled,"available":self.available,"armed":self.armed(),"gate":self.gate,"error":self.error,"physical_presentation":self.physical_presentation,"initializing":self.initializing,"rearming":self.rearming})
    }
}
enum Update {
    Connected(u64),
    Armed(u64, OwnedFd),
    Prepare(u64, bool),
    Initial(u64, bool),
    Lost(u64),
}
struct Shared {
    updates: Mutex<VecDeque<Update>>,
    ping: Ping,
    stop: AtomicBool,
    overflow: AtomicBool,
    reconnect: AtomicBool,
}
impl Shared {
    fn publish(&self, update: Update) {
        let mut queue = self.updates.lock().unwrap();
        if queue.len() < 64 {
            queue.push_back(update);
        } else {
            self.overflow.store(true, Ordering::Release);
            self.reconnect.store(true, Ordering::Release);
        }
        self.ping.ping();
    }
}
pub struct Server {
    shared: Arc<Shared>,
    connection: Arc<Mutex<Option<Connection>>>,
    worker: Option<JoinHandle<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(connection) = self.connection.lock().unwrap().take() {
            let _ = connection.close();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
    bus: Option<String>,
    physical: bool,
) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let shared = Arc::new(Shared {
        updates: Mutex::new(VecDeque::new()),
        ping,
        stop: AtomicBool::new(false),
        overflow: AtomicBool::new(false),
        reconnect: AtomicBool::new(false),
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |(), _, data| {
            let pending = std::mem::take(&mut *updates.updates.lock().unwrap());
            for update in pending {
                data.state.sleep_update(update);
            }
            if updates.overflow.swap(false, Ordering::AcqRel) {
                data.state.sleep.available = false;
                data.state.sleep.initializing = true;
                data.state.sleep.error =
                    Some("Sleep event queue overflow; coordination unavailable".into());
                let _ = data.state.request_lock();
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state.sleep.enabled = true;
    state.sleep.initializing = true;
    state.sleep.physical_presentation = physical;
    let connection = Arc::new(Mutex::new(None));
    let saved = connection.clone();
    let worker_shared = shared.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-sleep".into())
        .spawn(move || {
            let mut epoch = 0u64;
            let mut failures = 0u32;
            while !worker_shared.stop.load(Ordering::Acquire) {
                epoch = epoch.checked_add(1).expect("sleep epoch exhausted");
                let started = std::time::Instant::now();
                let result = run(&worker_shared, &saved, bus.as_deref(), epoch, physical);
                if worker_shared.stop.load(Ordering::Acquire) {
                    break;
                }
                if let Err(error) = result {
                    tracing::warn!(%error,"sleep coordination unavailable");
                }
                worker_shared.publish(Update::Lost(epoch));
                if let Some(c) = saved.lock().unwrap().take() {
                    let _ = c.close();
                }
                if started.elapsed() >= Duration::from_secs(30) {
                    failures = 0;
                }
                let delay = Duration::from_millis(250 << failures.min(4));
                failures = failures.saturating_add(1);
                // Bounded shutdown without a compositor polling timer.
                let deadline = std::time::Instant::now() + delay;
                while !worker_shared.stop.load(Ordering::Acquire)
                    && std::time::Instant::now() < deadline
                {
                    std::thread::sleep(Duration::from_millis(25));
                }
            }
        })?;
    Ok(Server {
        shared,
        connection,
        worker: Some(worker),
    })
}

enum Event {
    Prepare(bool),
    Lost,
}
fn run(
    shared: &Arc<Shared>,
    saved: &Arc<Mutex<Option<Connection>>>,
    bus: Option<&str>,
    epoch: u64,
    physical: bool,
) -> Result<()> {
    let builder = if let Some(address) = bus {
        Builder::address(address)?
    } else {
        Builder::system()?
    };
    let connection = builder.method_timeout(Duration::from_secs(3)).build()?;
    *saved.lock().unwrap() = Some(connection.clone());
    let dbus = Proxy::new(
        &connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )?;
    let owner: String = dbus.call("GetNameOwner", &(LOGIN,))?;
    if !physical {
        let uid: u32 = dbus.call("GetConnectionUnixUser", &(owner.as_str(),))?;
        anyhow::ensure!(
            uid != 0 && uid == rustix::process::getuid().as_raw(),
            "Nested sleep diagnostics require a same-user fixture, never a root system manager"
        );
    }
    let (sender, receiver) = mpsc::sync_channel(64);
    let lost = Arc::new(AtomicBool::new(false));
    let mut watchers = Vec::new();
    let result = (|| -> Result<()> {
        for (rule, manager) in [
            (
                format!(
                    "type='signal',sender='{owner}',path='{PATH}',interface='{MANAGER}',member='PrepareForSleep'"
                ),
                true,
            ),
            (
                format!(
                    "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='{LOGIN}'"
                ),
                false,
            ),
        ] {
            let messages = MessageIterator::for_match_rule(rule.as_str(), &connection, Some(64))?;
            let sender = sender.clone();
            let failed = lost.clone();
            watchers.push(
                std::thread::Builder::new()
                    .name("tarot-sleep-events".into())
                    .spawn(move || {
                        for message in messages {
                            let Ok(message) = message else {
                                break;
                            };
                            let event = if manager {
                                match message.body().deserialize::<(bool,)>() {
                                    Ok((start,)) => Event::Prepare(start),
                                    Err(_) => continue,
                                }
                            } else {
                                Event::Lost
                            };
                            if sender.try_send(event).is_err() {
                                break;
                            }
                        }
                        failed.store(true, Ordering::Release);
                        let _ = sender.try_send(Event::Lost);
                    })?,
            );
        }
        let manager = Proxy::new(&connection, owner.as_str(), PATH, MANAGER)?;
        shared.publish(Update::Connected(epoch));
        let inhibit = || -> Result<OwnedFd> {
            let fd: zbus::zvariant::OwnedFd = manager.call(
                "Inhibit",
                &(
                    "sleep",
                    "Tarot",
                    "Present a protected frame before sleep",
                    "delay",
                ),
            )?;
            let fd: OwnedFd = fd.into();
            // Never leak the inhibitor into an application/locker exec.
            rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC)?;
            Ok(fd)
        };
        let mut preparing: bool = manager
            .get_property("PreparingForSleep")
            .context("reading initial sleep preparation")?;
        shared.publish(Update::Initial(epoch, preparing));
        // elogind rejects delay inhibitors while its previous sleep action is
        // still running, including briefly after PrepareForSleep(false). Keep
        // this owner/epoch and retry that specific response for a bounded time.
        // An already-preparing manager cannot grant an initial delay inhibitor.
        let pending = || {
            (
                Instant::now(),
                Instant::now() + Duration::from_secs(3),
                25u64,
            )
        };
        let mut rearm = (!preparing).then(pending);
        loop {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            if lost.load(Ordering::Acquire) {
                bail!("sleep manager or signal stream lost");
            }
            if shared.reconnect.swap(false, Ordering::AcqRel) {
                bail!("sleep event queue overflow");
            }
            // Only a pending re-arm needs a timer. Settled operation blocks
            // without polling; connection closure wakes the signal readers.
            let event = if let Some((next, _, _)) = rearm {
                match receiver.recv_timeout(next.saturating_duration_since(Instant::now())) {
                    Ok(event) => Some(event),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
                }
            } else {
                Some(receiver.recv().context("sleep signal stream closed")?)
            };
            match event {
                Some(Event::Lost) => bail!("sleep manager owner changed"),
                Some(Event::Prepare(start)) => {
                    if preparing == start {
                        continue;
                    }
                    preparing = start;
                    shared.publish(Update::Prepare(epoch, start));
                    rearm = (!start).then(pending);
                }
                None => match inhibit() {
                    Ok(fd) => {
                        shared.publish(Update::Armed(epoch, fd));
                        rearm = None;
                    }
                    Err(error) => {
                        let (_, deadline, delay) = rearm.expect("pending inhibitor retry");
                        let busy = matches!(error.downcast_ref::<zbus::Error>(),
                            Some(zbus::Error::MethodError(name, _, _))
                            if name.as_str() == "org.freedesktop.login1.OperationInProgress");
                        if !busy || Instant::now() >= deadline {
                            return Err(error.context("arming sleep delay inhibitor"));
                        }
                        rearm = Some((
                            (Instant::now() + Duration::from_millis(delay)).min(deadline),
                            deadline,
                            (delay * 2).min(400),
                        ));
                    }
                },
            }
        }
    })();
    let _ = connection.close();
    for watcher in watchers {
        let _ = watcher.join();
    }
    result
}

impl Compositor {
    fn sleep_update(&mut self, update: Update) {
        match update {
            Update::Connected(epoch) => {
                if !self.sleep.gate.connect(epoch) {
                    return;
                }
                self.sleep.inhibitor = None;
                self.sleep.available = false;
                self.sleep.initializing = true;
                self.sleep.rearming = false;
                self.sleep.error = None;
                // Service replacement never makes a previously protected session
                // ordinary, even if the previous prepare/resume signal was lost.
                self.idle_system_wake();
                let _ = self.request_lock();
                self.redraw = true;
            }
            Update::Armed(epoch, fd) if epoch == self.sleep.gate.epoch => {
                self.sleep.available = true;
                self.sleep.rearming = false;
                self.sleep.error = None;
                if !self.sleep.gate.preparing || self.sleep.gate.barrier.is_some() {
                    self.sleep.inhibitor = Some(fd);
                }
            }
            Update::Initial(epoch, start) if epoch == self.sleep.gate.epoch => {
                self.sleep.initializing = false;
                self.sleep.available = true;
                self.sleep.rearming = !start && self.sleep.inhibitor.is_none();
                if start {
                    self.sleep_update(Update::Prepare(epoch, true));
                }
            }
            Update::Prepare(epoch, start) => {
                if !self.sleep.gate.prepare(epoch, start) {
                    return;
                }
                self.sleep.rearming = !start;
                self.idle_system_wake();
                if let Err(error) = self.request_lock() {
                    self.sleep.error = Some(error.to_string());
                }
                self.clear_protected_focus();
                self.refresh_lock_focus();
                self.redraw = true;
            }
            Update::Lost(epoch) if epoch >= self.sleep.gate.epoch => {
                self.sleep.inhibitor = None;
                self.sleep.available = false;
                self.sleep.initializing = true;
                self.sleep.rearming = false;
                self.sleep.error = Some("Sleep coordination unavailable".into());
                self.idle_system_wake();
                let _ = self.request_lock();
            }
            _ => {}
        }
    }
    pub fn sleep_frame_presented(&mut self, barrier: Option<Barrier>, generation: Option<u64>) {
        let Some(barrier) = barrier else {
            return;
        };
        if generation.is_some() && generation == self.lock_generation() {
            if self.sleep.gate.presented(barrier) == Some(Purpose::Prepare) {
                self.sleep.inhibitor = None;
            }
        } else if self.sleep.gate.barrier.is_some() {
            self.redraw = true;
        }
    }
}
