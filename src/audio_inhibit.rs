//! An owned login1 sleep-block FD for observed uncorked audio streams.
//! Separate from the protected-frame delay inhibitor and display idle policy.
use crate::{CalloopData, Compositor};
use anyhow::{Result, ensure};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    os::fd::OwnedFd,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot::local_bus::{Cancellation, connect as local_connection};
use tarot_core::audio::{Inhibition, Snapshot};
use zbus::blocking::{Connection, MessageIterator, Proxy};

const NAME: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const IFACE: &str = "org.freedesktop.login1.Manager";

struct Shared {
    // Low bit is demand; remaining bits retire grants for older observations.
    demand: AtomicU64,
    stop: AtomicBool,
    sender: mpsc::SyncSender<()>,
    latest: Mutex<Option<Inhibition>>,
    cancel: Mutex<Option<Arc<Cancellation>>>,
    ping: Ping,
}
impl Shared {
    fn wake(&self) {
        let _ = self.sender.try_send(());
    }
    fn publish(&self, available: bool, held: bool, error: Option<String>) {
        let active = self.demand.load(Ordering::Acquire) & 1 != 0;
        *self.latest.lock().unwrap() = Some(Inhibition {
            enabled: true,
            available,
            active,
            held: held && active,
            error,
        });
        self.ping.ping();
    }
}
pub struct Server {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn activity(&self, snapshot: &Snapshot) {
        let active = snapshot.available && snapshot.activity.active();
        let previous = self.shared.demand.load(Ordering::Acquire);
        if (previous & 1 != 0) != active {
            self.shared.demand.store(
                previous.checked_add(2).expect("audio demand exhausted") & !1 | u64::from(active),
                Ordering::Release,
            );
            self.shared.wake();
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(cancel) = self.shared.cancel.lock().unwrap().as_ref() {
            cancel.cancel();
        }
        self.shared.wake();
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
    ensure!(
        physical || bus.is_some(),
        "Nested audio inhibition requires a private bus"
    );
    let address = bus.unwrap_or_else(|| {
        std::env::var("DBUS_SYSTEM_BUS_ADDRESS")
            .unwrap_or_else(|_| "unix:path=/run/dbus/system_bus_socket".into())
    });
    let (ping, source) = make_ping()?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let shared = Arc::new(Shared {
        demand: AtomicU64::new(u64::from(
            state.shell.audio.available && state.shell.audio.activity.active(),
        )),
        stop: AtomicBool::new(false),
        sender,
        latest: Mutex::new(None),
        cancel: Mutex::new(None),
        ping,
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |(), _, data| {
            if let Some(value) = updates.latest.lock().unwrap().take()
                && data.state.shell.audio_inhibition != value
            {
                data.state.shell.audio_inhibition = value;
                data.state.ui_dirty = true;
                data.state.redraw = true;
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state.shell.audio_inhibition.enabled = true;
    let worker_shared = shared.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-audio-awake".into())
        .spawn(move || {
            while !worker_shared.stop.load(Ordering::Acquire) {
                let cancel = Arc::new(Cancellation::new(Arc::new(AtomicBool::new(false))));
                {
                    let mut slot = worker_shared.cancel.lock().unwrap();
                    if worker_shared.stop.load(Ordering::Acquire) {
                        break;
                    }
                    *slot = Some(cancel.clone());
                }
                let _ = run(
                    &worker_shared,
                    &receiver,
                    &address,
                    physical,
                    cancel.clone(),
                );
                cancel.cancel();
                worker_shared.cancel.lock().unwrap().take();
                if worker_shared.stop.load(Ordering::Acquire) {
                    break;
                }
                worker_shared.publish(
                    false,
                    false,
                    Some("Audio sleep coordination unavailable; reconnecting".into()),
                );
                let deadline = Instant::now() + Duration::from_secs(2);
                while !worker_shared.stop.load(Ordering::Acquire) && Instant::now() < deadline {
                    let _ =
                        receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()));
                }
            }
        })?;
    Ok(Server {
        shared,
        worker: Some(worker),
    })
}
fn run(
    shared: &Arc<Shared>,
    receiver: &mpsc::Receiver<()>,
    address: &str,
    physical: bool,
    cancel: Arc<Cancellation>,
) -> Result<()> {
    let connection = local_connection(address, cancel.clone())?;
    // Set up owner observation before querying it: a release/reacquire cannot
    // silently transfer an old grant to the replacement manager.
    let rule = format!(
        "type='signal',sender='org.freedesktop.DBus',path='/org/freedesktop/DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='{NAME}'"
    );
    let mut messages = MessageIterator::for_match_rule(rule.as_str(), &connection, Some(8))?;
    let lost = Arc::new(AtomicBool::new(false));
    let failed = lost.clone();
    let wake = shared.clone();
    let interrupted = cancel.clone();
    let watcher = std::thread::Builder::new()
        .name("audio-awake-owner".into())
        .spawn(move || {
            let _ = messages.next();
            failed.store(true, Ordering::Release);
            interrupted.cancel();
            wake.wake();
        })?;
    let result = (|| -> Result<()> {
        let dbus = Proxy::new(
            &connection,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
        )?;
        let owner: String = dbus.call("GetNameOwner", &(NAME,))?;
        let uid: u32 = dbus.call("GetConnectionUnixUser", &(owner.as_str(),))?;
        ensure!(
            if physical {
                uid == 0
            } else {
                uid != 0 && uid == rustix::process::getuid().as_raw()
            },
            "Untrusted audio sleep manager"
        );
        let mut held: Option<OwnedFd> = None;
        let mut granted = 0;
        shared.publish(true, false, None);
        loop {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            ensure!(!lost.load(Ordering::Acquire), "Audio sleep manager changed");
            let demand = shared.demand.load(Ordering::Acquire);
            let active = demand & 1 != 0;
            if !active || held.is_some() && granted != demand {
                held = None;
            }
            if !active {
                shared.publish(true, false, None);
            } else if held.is_none() {
                shared.publish(true, false, None);
                // An uncertain reply can carry a late FD. Retire this entire
                // owned connection on any error before retrying; never leave
                // unclaimed grant descriptors queued on a healthy-looking bus.
                let fd = acquire(&connection, &owner)?;
                if shared.stop.load(Ordering::Acquire) || lost.load(Ordering::Acquire) {
                    return Ok(());
                }
                if shared.demand.load(Ordering::Acquire) != demand {
                    drop(fd);
                    continue;
                }
                held = Some(fd);
                granted = demand;
                shared.publish(true, true, None);
            }
            let _ = receiver.recv();
        }
    })();
    // Shutdown the owned socket before joining the reader, including setup/call
    // failures and a peer that stops reading. Closing the FD releases inhibition.
    cancel.cancel();
    let _ = connection.close();
    let _ = watcher.join();
    result
}
fn acquire(connection: &Connection, owner: &str) -> Result<OwnedFd> {
    let reply = connection.call_method(
        Some(owner),
        PATH,
        Some(IFACE),
        "Inhibit",
        &(
            "sleep",
            "Tarot",
            "Audio playback or recording is active",
            "block",
        ),
    )?;
    ensure!(
        reply.header().sender().is_some_and(|s| s.as_str() == owner)
            && reply.body().len() <= 64
            && reply.data().fds().len() == 1,
        "Invalid audio inhibition reply"
    );
    let fd: zbus::zvariant::OwnedFd = reply.body().deserialize()?;
    let fd: OwnedFd = fd.into();
    rustix::io::fcntl_setfd(&fd, rustix::io::FdFlags::CLOEXEC)?;
    Ok(fd)
}
