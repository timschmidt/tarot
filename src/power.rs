//! UPower and login1 display control. Platform calls never run in the renderer.
use crate::{CalloopData, Compositor};
use anyhow::{Context, Result, bail, ensure};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot_core::power::{
    Battery, Brightness, ChargeState, Snapshot, UiAction, brightness_value, coarse_level,
    percentage,
};
use zbus::{
    blocking::{Connection, MessageIterator, Proxy, connection::Builder},
    zvariant::OwnedValue,
};

const UP: &str = "org.freedesktop.UPower";
const DEVICE: &str = "org.freedesktop.UPower.Device";
const LOGIN: &str = "org.freedesktop.login1";
const SESSION: &str = "org.freedesktop.login1.Session";

pub struct Options {
    pub bus: Option<String>,
    pub backlights: PathBuf,
}
enum Event {
    Wake,
    Watch(bool),
    Action(UiAction),
    Stop,
}

struct Shared {
    pending: Mutex<Option<Snapshot>>,
    latest: Mutex<Snapshot>,
    ping: Ping,
    sender: mpsc::SyncSender<Event>,
    stop: AtomicBool,
    lost: AtomicBool,
    target: AtomicU8,
}

impl Shared {
    fn publish(&self, mut value: Snapshot) {
        self.publish_result(&mut value, false);
    }
    fn publish_result(&self, value: &mut Snapshot, force: bool) {
        value.enabled = true;
        value.brightness.pending_percent = match self.target.load(Ordering::Acquire) {
            0 => None,
            p => Some(p),
        };
        let mut latest = self.latest.lock().unwrap();
        value.generation = latest.generation;
        if *latest == *value && !force {
            return;
        }
        value.generation += 1;
        *latest = value.clone();
        *self.pending.lock().unwrap() = Some(value.clone());
        self.ping.ping();
    }
    fn wake(&self) {
        let _ = self.sender.try_send(Event::Wake);
    }
}

pub struct Server {
    shared: Arc<Shared>,
    connection: Arc<Mutex<Option<Connection>>>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn watch(&self, watching: bool) {
        let _ = self.shared.sender.try_send(Event::Watch(watching));
    }
    pub fn action(&self, action: UiAction) -> Result<()> {
        let UiAction::Brightness { percent } = action;
        ensure!(
            (5..=100).contains(&percent),
            "Brightness must be between 5 and 100 percent"
        );
        ensure!(
            self.shared
                .target
                .compare_exchange(0, percent, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "Brightness change is still pending"
        );
        if self
            .shared
            .sender
            .try_send(Event::Action(UiAction::Brightness { percent }))
            .is_err()
        {
            self.shared.target.store(0, Ordering::Release);
            bail!("System controls are busy or unavailable");
        }
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        let connection = self.connection.lock().unwrap().take();
        if let Some(connection) = connection {
            let _ = connection.close();
        }
        let _ = self.shared.sender.try_send(Event::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
    options: Options,
) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let (sender, receiver) = mpsc::sync_channel(64);
    let shared = Arc::new(Shared {
        pending: Mutex::new(None),
        latest: Mutex::new(Snapshot::default()),
        ping,
        sender,
        stop: AtomicBool::new(false),
        lost: AtomicBool::new(false),
        target: AtomicU8::new(0),
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |(), _, data| {
            if let Some(mut snapshot) = updates.pending.lock().unwrap().take() {
                snapshot.brightness.pending_percent = match updates.target.load(Ordering::Acquire) {
                    0 => None,
                    p => Some(p),
                };
                data.state.shell.power = snapshot;
                data.state.ui_dirty = true;
                data.state.redraw = true;
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let connection = Arc::new(Mutex::new(None));
    let saved = connection.clone();
    let worker_shared = shared.clone();
    state.shell.power.enabled = true;
    let worker = std::thread::Builder::new()
        .name("tarot-system".into())
        .spawn(move || supervise(worker_shared, receiver, saved, options))?;
    Ok(Server {
        shared,
        connection,
        worker: Some(worker),
    })
}

fn supervise(
    shared: Arc<Shared>,
    receiver: mpsc::Receiver<Event>,
    saved: Arc<Mutex<Option<Connection>>>,
    options: Options,
) {
    let mut watching = false;
    let mut failures: u32 = 0;
    while !shared.stop.load(Ordering::Acquire) {
        shared.lost.store(false, Ordering::Release);
        let started = Instant::now();
        let result = run(&shared, &receiver, &saved, &options, &mut watching);
        let connection = saved.lock().unwrap().take();
        if let Some(connection) = connection {
            let _ = connection.close();
        }
        if let Err(error) = result {
            let mut snapshot = Snapshot::default();
            snapshot.battery.error = Some(format!("Power service unavailable: {error}"));
            snapshot.brightness = read_backlight(&options.backlights).unwrap_or_default();
            snapshot.brightness.can_set = false;
            snapshot.brightness.error = Some("Brightness service unavailable".into());
            shared.target.store(0, Ordering::Release);
            shared.publish(snapshot);
        }
        if started.elapsed() >= Duration::from_secs(30) {
            failures = 0;
        }
        let deadline = Instant::now() + Duration::from_millis(250 << failures.min(4));
        failures = failures.saturating_add(1);
        while !shared.stop.load(Ordering::Acquire) && Instant::now() < deadline {
            match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Event::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Ok(Event::Watch(open)) => watching = open,
                Ok(Event::Action(_)) => {
                    shared.target.store(0, Ordering::Release);
                    let mut value = shared.latest.lock().unwrap().clone();
                    value.brightness.action_error =
                        Some("Brightness service is unavailable".into());
                    shared.publish_result(&mut value, true);
                }
                Ok(Event::Wake) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

fn watch(
    shared: Arc<Shared>,
    connection: &Connection,
    rule: &str,
    properties: bool,
) -> Result<JoinHandle<()>> {
    let messages = MessageIterator::for_match_rule(rule, connection, Some(64))?;
    Ok(std::thread::Builder::new()
        .name("tarot-power-events".into())
        .spawn(move || {
            for message in messages {
                let Ok(message) = message else {
                    break;
                };
                let relevant = if properties {
                    message.header().path().is_some_and(|p| {
                        p.as_str().starts_with("/org/freedesktop/UPower")
                            || p.as_str().starts_with("/org/freedesktop/login1/session")
                    })
                } else {
                    message
                        .body()
                        .deserialize::<(String, String, String)>()
                        .is_ok_and(|(name, _, _)| name == UP || name == LOGIN)
                };
                if relevant {
                    shared.wake();
                }
            }
            shared.lost.store(true, Ordering::Release);
            shared.wake();
        })?)
}

fn run(
    shared: &Arc<Shared>,
    receiver: &mpsc::Receiver<Event>,
    saved: &Arc<Mutex<Option<Connection>>>,
    options: &Options,
    watching: &mut bool,
) -> Result<()> {
    let builder = if let Some(address) = &options.bus {
        Builder::address(address.as_str())?
    } else {
        Builder::system()?
    };
    let connection = builder.method_timeout(Duration::from_secs(3)).build()?;
    *saved.lock().unwrap() = Some(connection.clone());
    if shared.stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let property_watcher = watch(
        shared.clone(),
        &connection,
        "type='signal',interface='org.freedesktop.DBus.Properties',path_namespace='/org/freedesktop'",
        true,
    )?;
    let owner_watcher = match watch(
        shared.clone(),
        &connection,
        "type='signal',interface='org.freedesktop.DBus',member='NameOwnerChanged'",
        false,
    ) {
        Ok(watcher) => watcher,
        Err(error) => {
            let _ = connection.close();
            let _ = property_watcher.join();
            return Err(error);
        }
    };
    let result = (|| -> Result<()> {
        let mut snapshot = refresh(&connection, &options.backlights);
        let mut action_error = None;
        shared.publish(snapshot.clone());
        loop {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let timeout = if *watching {
                Some(Duration::from_secs(1))
            } else if !snapshot.battery.available || !snapshot.brightness.can_set {
                Some(Duration::from_secs(4))
            } else {
                None
            };
            let event = if let Some(timeout) = timeout {
                match receiver.recv_timeout(timeout) {
                    Ok(event) => Some(event),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => Some(Event::Stop),
                }
            } else {
                receiver.recv().ok()
            };
            if shared.lost.load(Ordering::Acquire) {
                bail!("system bus disconnected");
            }
            match event {
                Some(Event::Stop) => return Ok(()),
                Some(Event::Watch(open)) => *watching = open,
                Some(Event::Action(UiAction::Brightness { percent })) => {
                    snapshot = refresh(&connection, &options.backlights);
                    action_error = None;
                    shared.publish(snapshot.clone());
                    let result = set_brightness(
                        &connection,
                        &options.backlights,
                        &snapshot.brightness,
                        percent,
                    );
                    shared.target.store(0, Ordering::Release);
                    snapshot = refresh(&connection, &options.backlights);
                    if let Err(error) = result {
                        action_error = Some(format!("Brightness change failed: {error}"));
                    }
                    snapshot.brightness.action_error = action_error.clone();
                    shared.publish_result(&mut snapshot, true);
                    continue;
                }
                Some(Event::Wake) | None => {}
            }
            snapshot = refresh(&connection, &options.backlights);
            snapshot.brightness.action_error = action_error.clone();
            shared.publish(snapshot.clone());
        }
    })();
    let _ = connection.close();
    let _ = property_watcher.join();
    let _ = owner_watcher.join();
    result
}

fn properties(
    connection: &Connection,
    destination: &str,
    path: &str,
    interface: &str,
) -> Result<HashMap<String, OwnedValue>> {
    Ok(Proxy::new(
        connection,
        destination,
        path,
        "org.freedesktop.DBus.Properties",
    )?
    .call("GetAll", &(interface,))?)
}

fn read_battery(connection: &Connection) -> Result<Battery> {
    let values = properties(
        connection,
        UP,
        "/org/freedesktop/UPower/devices/DisplayDevice",
        DEVICE,
    )?;
    let present = bool::try_from(
        values
            .get("IsPresent")
            .context("missing battery presence")?,
    )?;
    let state = u32::try_from(values.get("State").context("missing charge state")?)?;
    let percent = values
        .get("Percentage")
        .map(f64::try_from)
        .transpose()?
        .and_then(percentage);
    let level = values
        .get("BatteryLevel")
        .map(u32::try_from)
        .transpose()?
        .and_then(coarse_level);
    let root = properties(connection, UP, "/org/freedesktop/UPower", UP)?;
    let external_power = root
        .get("OnBattery")
        .map(bool::try_from)
        .transpose()?
        .map(|value| !value);
    Ok(Battery {
        available: true,
        present,
        percent,
        coarse_level: level,
        state: ChargeState::from_upower(state),
        external_power,
        error: None,
    })
}

fn read_number(path: &Path) -> Result<u32> {
    let mut value = String::new();
    File::open(path)?.take(128).read_to_string(&mut value)?;
    Ok(value.trim().parse()?)
}

fn read_backlight(root: &Path) -> Result<Brightness> {
    let entries: Vec<_> = std::fs::read_dir(root)?.collect::<std::io::Result<Vec<_>>>()?;
    ensure!(entries.len() == 1, "A single display backlight is required");
    let name = entries[0]
        .file_name()
        .into_string()
        .map_err(|_| anyhow::anyhow!("Invalid backlight name"))?;
    ensure!(name.len() <= 128, "Invalid backlight name");
    let path = root.join(&name);
    let maximum = read_number(&path.join("max_brightness"))?;
    let actual = read_number(&path.join("actual_brightness"))?;
    let requested = read_number(&path.join("brightness"))?;
    ensure!(
        maximum > 0 && actual <= maximum && requested <= maximum,
        "Invalid backlight range"
    );
    Ok(Brightness {
        device: Some(name),
        maximum,
        actual: Some(actual),
        requested: Some(requested),
        ..Default::default()
    })
}

fn can_set(connection: &Connection) -> Result<()> {
    let session = properties(
        connection,
        LOGIN,
        "/org/freedesktop/login1/session/self",
        SESSION,
    )?;
    let active = bool::try_from(session.get("Active").context("missing active session")?)?;
    let remote = bool::try_from(session.get("Remote").context("missing session type")?)?;
    let seat: (String, zbus::zvariant::OwnedObjectPath) = session
        .get("Seat")
        .context("missing session seat")?
        .try_clone()?
        .try_into()?;
    ensure!(
        active && !remote && !seat.0.is_empty(),
        "Brightness requires an active local login session"
    );
    Ok(())
}

fn refresh(connection: &Connection, root: &Path) -> Snapshot {
    let battery = read_battery(connection).unwrap_or_else(|error| Battery {
        error: Some(format!("Battery unavailable: {error}")),
        ..Default::default()
    });
    let brightness = match read_backlight(root) {
        Ok(mut brightness) => {
            match can_set(connection) {
                Ok(()) => brightness.can_set = true,
                Err(error) => brightness.error = Some(error.to_string()),
            };
            brightness
        }
        Err(error) => Brightness {
            error: Some(format!("Backlight unavailable: {error}")),
            ..Default::default()
        },
    };
    Snapshot {
        battery,
        brightness,
        ..Default::default()
    }
}

fn set_brightness(
    connection: &Connection,
    root: &Path,
    display: &Brightness,
    percent: u8,
) -> Result<()> {
    can_set(connection)?;
    let target =
        brightness_value(percent, display.maximum).context("Invalid brightness request")?;
    let name = display.device.as_deref().context("No display backlight")?;
    Proxy::new(
        connection,
        LOGIN,
        "/org/freedesktop/login1/session/self",
        SESSION,
    )?
    .call::<_, _, ()>("SetBrightness", &("backlight", name, target))?;
    let actual = read_backlight(root)?;
    ensure!(
        actual.actual == Some(target) && actual.requested == Some(target),
        "Backlight did not confirm the requested level"
    );
    Ok(())
}
