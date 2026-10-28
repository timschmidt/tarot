//! Standard SensorProxy adapter. D-Bus calls and signals stay off the renderer.
use crate::{CalloopData, Compositor};
use anyhow::{Result, ensure};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Duration,
};
use tarot_core::orientation::SensorSnapshot;
use zbus::blocking::{Connection, MessageIterator, Proxy, connection::Builder};

const NAME: &str = "net.hadess.SensorProxy";
const PATH: &str = "/net/hadess/SensorProxy";

fn validate_owner(uid: u32, current_uid: u32, diagnostic: bool) -> Result<()> {
    ensure!(
        if diagnostic {
            current_uid != 0 && uid == current_uid
        } else {
            uid == 0
        },
        "Unexpected sensor service owner"
    );
    Ok(())
}

struct Shared {
    pending: Mutex<Option<SensorSnapshot>>,
    latest: Mutex<SensorSnapshot>,
    ping: Ping,
    wake: mpsc::SyncSender<()>,
    wanted: AtomicBool,
    stop: AtomicBool,
    lost: AtomicBool,
    active_epoch: AtomicU64,
}

impl Shared {
    fn wake(&self) {
        let _ = self.wake.try_send(());
    }
    fn publish(&self, snapshot: SensorSnapshot) {
        let mut latest = self.latest.lock().unwrap();
        if *latest == snapshot {
            return;
        }
        *latest = snapshot.clone();
        *self.pending.lock().unwrap() = Some(snapshot);
        self.ping.ping();
    }
}

pub struct Server {
    shared: Arc<Shared>,
    connection: Arc<Mutex<Option<Connection>>>,
    worker: Option<JoinHandle<()>>,
}

impl Server {
    pub fn watch(&self, wanted: bool) {
        if self.shared.wanted.swap(wanted, Ordering::AcqRel) != wanted {
            self.shared.wake();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        if let Some(connection) = self.connection.lock().unwrap().take() {
            let _ = connection.close();
        }
        self.shared.wake();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Compositor {
    pub fn sensor_rotation_wanted(&self) -> bool {
        !self.shell.rotation_locked
            && !self.idle.off()
            && !self.idle.seat_inactive
            && !self.sleep.gate.preparing
    }
}

pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
    bus: Option<String>,
) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let (wake, receiver) = mpsc::sync_channel(16);
    let shared = Arc::new(Shared {
        pending: Mutex::new(None),
        latest: Mutex::new(SensorSnapshot::default()),
        ping,
        wake,
        wanted: AtomicBool::new(state.sensor_rotation_wanted()),
        stop: AtomicBool::new(false),
        lost: AtomicBool::new(false),
        active_epoch: AtomicU64::new(0),
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |(), _, data| {
            let Some(snapshot) = updates.pending.lock().unwrap().take() else {
                return;
            };
            if snapshot.available && snapshot.epoch != updates.active_epoch.load(Ordering::Acquire)
            {
                return;
            }
            if data.state.sensor_rotation_wanted()
                && snapshot.claimed
                && let Some(orientation) = snapshot.orientation
            {
                data.state.request_orientation(orientation);
            }
            data.state.shell.sensors = snapshot;
            data.state.ui_dirty = true;
            data.state.redraw = true;
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let connection = Arc::new(Mutex::new(None));
    let saved = connection.clone();
    let worker_shared = shared.clone();
    state.shell.sensors.enabled = true;
    let worker = std::thread::Builder::new()
        .name("tarot-sensors".into())
        .spawn(move || supervise(worker_shared, receiver, saved, bus))?;
    Ok(Server {
        shared,
        connection,
        worker: Some(worker),
    })
}

fn watch(
    shared: Arc<Shared>,
    connection: &Connection,
    rule: &str,
    owner_changes: bool,
) -> Result<JoinHandle<()>> {
    let messages = MessageIterator::for_match_rule(rule, connection, Some(32))?;
    Ok(std::thread::Builder::new()
        .name("tarot-sensor-events".into())
        .spawn(move || {
            for message in messages {
                if message.is_err() {
                    break;
                }
                if owner_changes {
                    shared.active_epoch.store(0, Ordering::Release);
                    shared.lost.store(true, Ordering::Release);
                }
                shared.wake();
            }
            shared.active_epoch.store(0, Ordering::Release);
            shared.lost.store(true, Ordering::Release);
            shared.wake();
        })?)
}

fn run(
    shared: &Arc<Shared>,
    receiver: &mpsc::Receiver<()>,
    saved: &Arc<Mutex<Option<Connection>>>,
    address: Option<&str>,
    epoch: u64,
) -> Result<()> {
    let builder = if let Some(address) = address {
        Builder::address(address)?
    } else {
        Builder::system()?
    };
    let connection = builder.method_timeout(Duration::from_secs(2)).build()?;
    *saved.lock().unwrap() = Some(connection.clone());
    if shared.stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let bus = Proxy::new(
        &connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )?;
    // A real system service can be D-Bus activated. Private QA must explicitly
    // start its fixture; never ask its bus to activate a host service.
    if address.is_none() && shared.wanted.load(Ordering::Acquire) {
        let _: Result<u32, _> = bus.call("StartServiceByName", &(NAME, 0u32));
    }
    let owner: String = bus.call("GetNameOwner", &(NAME,))?;
    let uid: u32 = bus.call("GetConnectionUnixUser", &(owner.as_str(),))?;
    validate_owner(uid, rustix::process::getuid().as_raw(), address.is_some())?;
    let owner_watcher = watch(
        shared.clone(),
        &connection,
        "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='net.hadess.SensorProxy'",
        true,
    )?;
    let property_watcher = watch(
        shared.clone(),
        &connection,
        &format!(
            "type='signal',sender='{owner}',path='{PATH}',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',arg0='{NAME}'"
        ),
        false,
    )?;
    let result = (|| {
        let current: String = bus.call("GetNameOwner", &(NAME,))?;
        ensure!(
            current == owner,
            "Sensor service changed during subscription"
        );
        let proxy = zbus::blocking::proxy::Builder::<Proxy<'_>>::new(&connection)
            .destination(owner.as_str())?
            .path(PATH)?
            .interface(NAME)?
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()?;
        let mut claimed = false;
        shared.active_epoch.store(epoch, Ordering::Release);
        loop {
            if shared.stop.load(Ordering::Acquire) {
                break;
            }
            ensure!(
                !shared.lost.load(Ordering::Acquire),
                "Sensor service changed or disconnected"
            );
            let available: bool = proxy.get_property("HasAccelerometer")?;
            let wanted = shared.wanted.load(Ordering::Acquire) && available;
            if wanted && !claimed {
                proxy.call::<_, _, ()>("ClaimAccelerometer", &())?;
                claimed = true;
            } else if !wanted && claimed {
                proxy.call::<_, _, ()>("ReleaseAccelerometer", &())?;
                claimed = false;
            }
            let orientation = if claimed {
                let value: String = proxy.get_property("AccelerometerOrientation")?;
                value.parse().ok()
            } else {
                None
            };
            ensure!(
                !shared.lost.load(Ordering::Acquire),
                "Sensor service changed during readback"
            );
            shared.publish(SensorSnapshot {
                enabled: true,
                available: true,
                has_accelerometer: available,
                claimed,
                orientation,
                epoch,
                error: None,
            });
            if receiver.recv().is_err() {
                break;
            }
        }
        if claimed {
            let _ = proxy.call::<_, _, ()>("ReleaseAccelerometer", &());
        }
        Ok(())
    })();
    shared.active_epoch.store(0, Ordering::Release);
    let _ = connection.close();
    let _ = owner_watcher.join();
    let _ = property_watcher.join();
    result
}

fn supervise(
    shared: Arc<Shared>,
    receiver: mpsc::Receiver<()>,
    saved: Arc<Mutex<Option<Connection>>>,
    address: Option<String>,
) {
    let mut epoch = 0u64;
    let mut failures = 0u32;
    while !shared.stop.load(Ordering::Acquire) {
        shared.lost.store(false, Ordering::Release);
        epoch = epoch.checked_add(1).expect("Sensor epoch exhausted");
        let result = run(&shared, &receiver, &saved, address.as_deref(), epoch);
        shared.active_epoch.store(0, Ordering::Release);
        if let Some(connection) = saved.lock().unwrap().take() {
            let _ = connection.close();
        }
        if shared.stop.load(Ordering::Acquire) {
            break;
        }
        let error = result
            .err()
            .map(|error| format!("Orientation sensor unavailable: {error}"));
        shared.publish(SensorSnapshot {
            enabled: true,
            error,
            ..Default::default()
        });
        let delay = Duration::from_millis(250 << failures.min(4));
        failures = failures.saturating_add(1);
        match receiver.recv_timeout(delay) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_and_fixture_service_owners_have_distinct_trust_boundaries() {
        assert!(validate_owner(0, 1000, false).is_ok());
        assert!(validate_owner(1000, 1000, false).is_err());
        assert!(validate_owner(1000, 1000, true).is_ok());
        assert!(validate_owner(1001, 1000, true).is_err());
        assert!(validate_owner(0, 1000, true).is_err());
        assert!(validate_owner(0, 0, true).is_err());
    }
}
