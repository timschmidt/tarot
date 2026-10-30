//! Session-owned idle hints. No Suspend call or manager configuration mutation.
//! An idle hint is persistent manager state, not a descriptor-owned lease.
use crate::local_bus::{Cancellation, connect};
use anyhow::{Result, ensure};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot_core::session_idle::Publisher;
use zbus::{
    blocking::{Connection, MessageIterator, Proxy},
    zvariant::OwnedObjectPath,
};

const NAME: &str = "org.freedesktop.login1";
const PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";

struct Shared {
    demand: AtomicU64,
    stop: AtomicBool,
    connected: AtomicBool,
    sender: mpsc::SyncSender<()>,
    cancel: Mutex<Option<Arc<Cancellation>>>,
    last: Mutex<Option<Publisher>>,
    update: Box<dyn Fn(Publisher) + Send + Sync>,
}
impl Shared {
    fn wake(&self) {
        let _ = self.sender.try_send(());
    }
    fn publish(&self, value: Publisher) {
        let mut last = self.last.lock().unwrap();
        if last.as_ref() != Some(&value) {
            *last = Some(value.clone());
            (self.update)(value);
        }
    }
}

pub struct Worker {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}
impl Worker {
    /// Coalesces readiness changes and retires late acknowledgements.
    pub fn ready(&self, ready: bool) {
        if self
            .shared
            .demand
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                ((old & 1 != 0) != ready).then(|| {
                    old.checked_add(2).expect("idle demand exhausted") & !1 | u64::from(ready)
                })
            })
            .is_ok()
        {
            self.shared.wake();
        }
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.ready(false);
        self.shared.stop.store(true, Ordering::Release);
        // Connected workers get one bounded best-effort false write. Setup has
        // no session state to clear and can be canceled immediately.
        if !self.shared.connected.load(Ordering::Acquire)
            && let Some(cancel) = self.shared.cancel.lock().unwrap().as_ref()
        {
            cancel.cancel();
        }
        self.shared.wake();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn start(
    address: String,
    physical: bool,
    update: impl Fn(Publisher) + Send + Sync + 'static,
) -> Result<Worker> {
    ensure!(
        rustix::process::getuid().as_raw() != 0,
        "Idle publishing belongs to an ordinary GUI user"
    );
    let (sender, receiver) = mpsc::sync_channel(1);
    let shared = Arc::new(Shared {
        demand: AtomicU64::new(0),
        stop: AtomicBool::new(false),
        connected: AtomicBool::new(false),
        sender,
        cancel: Mutex::new(None),
        last: Mutex::new(None),
        update: Box::new(update),
    });
    let state = shared.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-session-idle".into())
        .spawn(move || {
            while !state.stop.load(Ordering::Acquire) {
                let cancel = Arc::new(Cancellation::new(Arc::new(AtomicBool::new(false))));
                {
                    let mut slot = state.cancel.lock().unwrap();
                    if state.stop.load(Ordering::Acquire) {
                        break;
                    }
                    *slot = Some(cancel.clone());
                }
                let result = run(&state, &receiver, &address, physical, cancel.clone());
                state.connected.store(false, Ordering::Release);
                cancel.cancel();
                state.cancel.lock().unwrap().take();
                if result.is_err() && !state.stop.load(Ordering::Acquire) {
                    // A failed/timed-out write can still take effect. Never
                    // claim that closing this connection cleared a hint.
                    state.publish(Publisher {
                        error: Some("Session idle state unknown; reconnecting".into()),
                        ..Publisher::default()
                    });
                }
                let deadline = Instant::now() + Duration::from_secs(2);
                while !state.stop.load(Ordering::Acquire) && Instant::now() < deadline {
                    let _ =
                        receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()));
                }
            }
        })?;
    Ok(Worker {
        shared,
        worker: Some(worker),
    })
}

fn proxy<'a>(
    connection: &'a Connection,
    owner: &'a str,
    path: &'a str,
    interface: &'a str,
) -> Result<Proxy<'a>> {
    Ok(zbus::blocking::proxy::Builder::<Proxy<'_>>::new(connection)
        .destination(owner)?
        .path(path)?
        .interface(interface)?
        .cache_properties(zbus::proxy::CacheProperties::No)
        .build()?)
}

/// Resolve the actual caller process every time; environment variables and a
/// remembered session object are insufficient authority for a hint write.
fn session<'a>(
    connection: &'a Connection,
    owner: &'a str,
    path: &'a OwnedObjectPath,
) -> Result<(Proxy<'a>, bool)> {
    let manager = proxy(connection, owner, PATH, MANAGER)?;
    let current: OwnedObjectPath = manager.call("GetSessionByPID", &(std::process::id(),))?;
    ensure!(&current == path, "GUI session identity changed");
    let session = proxy(connection, owner, path.as_str(), SESSION)?;
    let (uid, _): (u32, OwnedObjectPath) = session.get_property("User")?;
    let (seat, _): (String, OwnedObjectPath) = session.get_property("Seat")?;
    let class: String = session.get_property("Class")?;
    let kind: String = session.get_property("Type")?;
    let remote: bool = session.get_property("Remote")?;
    let vt: u32 = session.get_property("VTNr")?;
    ensure!(
        uid == rustix::process::getuid().as_raw()
            && seat == "seat0"
            && class == "user"
            && kind == "wayland"
            && !remote
            && vt > 0,
        "Expected the caller's local Wayland user session"
    );
    let active: bool = session.get_property("Active")?;
    Ok((session, active))
}

fn run(
    shared: &Arc<Shared>,
    receiver: &mpsc::Receiver<()>,
    address: &str,
    physical: bool,
    cancel: Arc<Cancellation>,
) -> Result<()> {
    let connection = connect(address, cancel.clone())?;
    shared.connected.store(true, Ordering::Release);
    let rule = format!(
        "type='signal',sender='org.freedesktop.DBus',path='/org/freedesktop/DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='{NAME}'"
    );
    let mut changes = MessageIterator::for_match_rule(rule.as_str(), &connection, Some(8))?;
    let lost = Arc::new(AtomicBool::new(false));
    let loss = lost.clone();
    let state = shared.clone();
    let interrupted = cancel.clone();
    let watcher = std::thread::spawn(move || {
        let _ = changes.next();
        loss.store(true, Ordering::Release);
        interrupted.cancel();
        state.wake();
    });
    let mut properties = None;
    let mut validated: Option<(String, OwnedObjectPath)> = None;
    let result = (|| -> Result<()> {
        let dbus = proxy(
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
            "Untrusted idle manager"
        );
        let manager = proxy(&connection, &owner, PATH, MANAGER)?;
        let path: OwnedObjectPath = manager.call("GetSessionByPID", &(std::process::id(),))?;
        session(&connection, &owner, &path)?;
        validated = Some((owner.clone(), path.clone()));
        let rule = format!(
            "type='signal',sender='{owner}',path_namespace='{PATH}',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged'"
        );
        let changes = MessageIterator::for_match_rule(rule.as_str(), &connection, Some(8))?;
        let state = shared.clone();
        let loss = lost.clone();
        properties = Some(std::thread::spawn(move || {
            for change in changes {
                if change.is_err() {
                    loss.store(true, Ordering::Release);
                    state.wake();
                    return;
                }
                state.wake();
            }
        }));
        // Hints outlive D-Bus connections. Clear any previous value before
        // considering current readiness, including reconnect after uncertainty.
        let (session, _) = session(&connection, &owner, &path)?;
        let _: () = session.call("SetIdleHint", &(false,))?;
        ensure!(
            !session.get_property::<bool>("IdleHint")?,
            "Idle clear was not observed"
        );
        loop {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            ensure!(!lost.load(Ordering::Acquire), "Idle manager changed");
            let demand = shared.demand.load(Ordering::Acquire);
            let (session, active) = self::session(&connection, &owner, &path)?;
            let action: String = manager.get_property("IdleAction")?;
            ensure!(
                matches!(action.as_str(), "ignore" | "suspend"),
                "Unsupported manager idle action"
            );
            let delay: u64 = manager.get_property("IdleActionUSec")?;
            ensure!(
                action == "ignore" || delay >= 5_000_000,
                "Manager idle delay is too short"
            );
            let wanted = demand & 1 != 0 && active;
            if session.get_property::<bool>("IdleHint")? != wanted {
                let _: () = session.call("SetIdleHint", &(wanted,))?;
            }
            let observed: bool = session.get_property("IdleHint")?;
            if shared.demand.load(Ordering::Acquire) != demand {
                continue;
            }
            ensure!(observed == wanted, "Session idle hint was not observed");
            shared.publish(Publisher {
                available: true,
                observed_idle: Some(observed),
                manager_action: Some(action),
                error: None,
            });
            let _ = receiver.recv();
        }
    })();
    let cleared = if let Some((owner, path)) = &validated {
        (|| -> Result<()> {
            let (session, _) = session(&connection, owner, path)?;
            let _: () = session.call("SetIdleHint", &(false,))?;
            ensure!(
                !session.get_property::<bool>("IdleHint")?,
                "Idle clear was not observed"
            );
            Ok(())
        })()
    } else {
        Ok(())
    };
    if shared.stop.load(Ordering::Acquire) && validated.is_some() {
        let certain = result.is_ok() && cleared.is_ok();
        shared.publish(Publisher {
            observed_idle: certain.then_some(false),
            error: (!certain).then(|| "Session idle state unknown at shutdown".into()),
            ..Publisher::default()
        });
    }
    cancel.cancel();
    let _ = connection.close();
    let _ = watcher.join();
    if let Some(properties) = properties {
        let _ = properties.join();
    }
    result.and(cleared)
}
