//! BlueZ adapter and agent supervision. All bus work is off the compositor.
mod agent;
use crate::{CalloopData, Compositor};
use anyhow::{Context, Result, bail, ensure};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::Poll,
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot_core::bluetooth::{Adapter, Device, Pending, Snapshot, UiAction};
use zbus::{
    blocking::{Connection, MessageIterator, connection::Builder},
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
const BLUEZ: &str = "org.bluez";
const ADAPTER: &str = "org.bluez.Adapter1";
const DEVICE: &str = "org.bluez.Device1";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
type Properties = HashMap<String, OwnedValue>;
type Objects = HashMap<OwnedObjectPath, HashMap<String, Properties>>;
const SHORT: Duration = Duration::from_secs(5);

pub(super) fn property<T: TryFrom<OwnedValue>>(props: &Properties, key: &str) -> Option<T> {
    props.get(key)?.try_clone().ok()?.try_into().ok()
}
pub(super) fn label(props: &Properties) -> String {
    property::<String>(props, "Alias")
        .or_else(|| property(props, "Name"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Bluetooth device".into())
        .chars()
        .filter(|c| !c.is_control())
        .take(96)
        .collect()
}
pub(super) async fn timed<F, T, E>(future: F, timeout: Duration) -> Result<T>
where
    F: Future<Output = std::result::Result<T, E>>,
    E: Into<anyhow::Error>,
{
    let mut future = std::pin::pin!(future);
    let mut timer = std::pin::pin!(async_io::Timer::after(timeout));
    std::future::poll_fn(|cx| {
        if let Poll::Ready(result) = future.as_mut().poll(cx) {
            return Poll::Ready(result.map_err(Into::into));
        }
        if timer.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Err(anyhow::anyhow!("Bluetooth request timed out")));
        }
        Poll::Pending
    })
    .await
}
fn rpc<B, R>(
    connection: &Connection,
    owner: &str,
    path: &str,
    interface: &str,
    method: &str,
    body: &B,
    timeout: Duration,
) -> Result<R>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    let message = async_io::block_on(timed(
        connection
            .inner()
            .call_method(Some(owner), path, Some(interface), method, body),
        timeout,
    ))?;
    Ok(message.body().deserialize()?)
}
fn set<B: zbus::zvariant::Type + Into<Value<'static>>>(
    c: &Connection,
    owner: &str,
    path: &str,
    interface: &str,
    key: &str,
    value: B,
) -> Result<()> {
    rpc(
        c,
        owner,
        path,
        PROPERTIES,
        "Set",
        &(interface, key, Value::new(value)),
        SHORT,
    )
}
#[derive(Default)]
struct Registry {
    owner: String,
    epoch: u64,
    next: u64,
    ids: HashMap<String, u64>,
}
impl Registry {
    fn bind(&mut self, owner: &str) -> Result<()> {
        if self.owner != owner {
            self.owner = owner.into();
            self.epoch = self.epoch.checked_add(1).context("Bluetooth epoch limit")?;
            self.ids.clear();
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.owner.clear();
        self.ids.clear();
    }
    fn id(&mut self, path: &str) -> Result<u64> {
        if let Some(id) = self.ids.get(path) {
            return Ok(*id);
        }
        ensure!(self.ids.len() < 512, "Bluetooth inventory limit");
        self.next = self.next.checked_add(1).context("Bluetooth token limit")?;
        self.ids.insert(path.into(), self.next);
        Ok(self.next)
    }
    fn remove(&mut self, path: &str) {
        let prefix = format!("{path}/");
        self.ids
            .retain(|key, _| key != path && !key.starts_with(&prefix));
    }
    fn has(&self, id: u64) -> bool {
        self.ids.values().any(|value| *value == id)
    }
    fn matches(&self, path: &str, id: u64) -> bool {
        self.ids.get(path) == Some(&id)
    }
}
enum Event {
    Wake,
    Action(UiAction),
    Context,
    CancelDevice(u64),
    Stop,
}
struct Shared {
    update: Mutex<Option<Snapshot>>,
    latest: Mutex<Snapshot>,
    ping: Ping,
    sender: mpsc::SyncSender<Event>,
    stop: AtomicBool,
    lost: AtomicBool,
    wake_queued: AtomicBool,
    open: AtomicBool,
    unlocked: AtomicBool,
    agent_released: AtomicBool,
    busy: AtomicBool,
    registry: Mutex<Registry>,
    agent: Arc<agent::State>,
}
impl Shared {
    fn wake(&self) {
        if !self.wake_queued.swap(true, Ordering::AcqRel) {
            let _ = self.sender.try_send(Event::Wake);
        }
    }
    fn publish(&self, mut value: Snapshot) {
        value.enabled = true;
        let mut latest = self.latest.lock().unwrap();
        value.generation = latest.generation;
        if *latest == value {
            return;
        }
        value.generation = value.generation.saturating_add(1);
        *latest = value.clone();
        *self.update.lock().unwrap() = Some(value);
        self.ping.ping();
    }
}
pub struct Server {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn context(&self, open: bool, protected: bool) {
        let open = open && !protected;
        let changed = self.shared.open.swap(open, Ordering::AcqRel) != open;
        let changed =
            (self.shared.unlocked.swap(!protected, Ordering::AcqRel) == protected) || changed;
        if changed {
            if !open || protected {
                let device = self.shared.agent.prompt().map(|p| p.device);
                self.shared.agent.clear();
                if let Some(device) = device {
                    let _ = self.shared.sender.try_send(Event::CancelDevice(device));
                }
            }
            let _ = self.shared.sender.try_send(Event::Context);
            self.shared.wake();
        }
    }
    pub fn action(&self, action: UiAction) -> Result<()> {
        if let UiAction::Reply { request, answer } = action {
            let rejected = answer == tarot_core::bluetooth::Answer::Reject;
            let device = self
                .shared
                .agent
                .prompt()
                .filter(|p| p.id == request)
                .map(|p| p.device);
            let result = self.shared.agent.reply(request, answer);
            if rejected {
                // A late rejection is harmless. Never let it cancel a newer
                // pairing operation after its own prompt has been retired.
                if result.is_ok()
                    && let Some(device) = device
                {
                    let _ = self.shared.sender.try_send(Event::CancelDevice(device));
                }
                return Ok(());
            }
            return result;
        }
        ensure!(
            self.shared.open.load(Ordering::Acquire)
                && self.shared.unlocked.load(Ordering::Acquire),
            "Open Bluetooth to change devices"
        );
        self.shared
            .sender
            .try_send(Event::Action(action))
            .map_err(|_| anyhow::anyhow!("Bluetooth service busy or unavailable"))
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.agent.clear();
        let _ = self.shared.sender.try_send(Event::Stop);
        self.shared.wake();
        if let Some(worker) = self.worker.take().filter(|w| w.is_finished()) {
            let _ = worker.join();
        }
    }
}
pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
    bus: Option<String>,
) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let (sender, receiver) = mpsc::sync_channel(16);
    let shared = Arc::new_cyclic(|weak| Shared {
        update: Mutex::new(None),
        latest: Mutex::new(Snapshot::default()),
        ping,
        sender,
        stop: AtomicBool::new(false),
        lost: AtomicBool::new(false),
        wake_queued: AtomicBool::new(false),
        open: AtomicBool::new(false),
        unlocked: AtomicBool::new(false),
        agent_released: AtomicBool::new(false),
        busy: AtomicBool::new(false),
        registry: Mutex::new(Registry::default()),
        agent: Arc::new(agent::State::new(weak.clone())),
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |_, _, data| {
            if let Some(snapshot) = updates.update.lock().unwrap().take() {
                let revision = data.state.shell.menus.revision;
                let effects = data.state.shell.sync_bluetooth(snapshot);
                if revision != data.state.shell.menus.revision {
                    data.state.cancel_input_sequences();
                }
                data.state.apply_effects(effects);
                data.state.refresh_keyboard();
                data.state.ui_dirty = true;
                data.state.redraw = true;
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state.shell.bluetooth.enabled = true;
    let background = shared.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-bluetooth".into())
        .spawn(move || supervise(background, receiver, bus))?;
    Ok(Server {
        shared,
        worker: Some(worker),
    })
}
struct Inventory {
    snapshot: Snapshot,
    owner: String,
    adapters: HashMap<u64, OwnedObjectPath>,
    devices: HashMap<u64, OwnedObjectPath>,
}
fn service_owner(c: &Connection, activate: bool) -> Result<String> {
    let resolve = || {
        rpc(
            c,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetNameOwner",
            &(BLUEZ,),
            SHORT,
        )
    };
    match resolve() {
        Ok(owner) => Ok(owner),
        Err(error) if !activate => Err(error),
        Err(_) => {
            // Standard system-bus activation, never an init-system command or
            // host-service activation through a diagnostic bus.
            let _: u32 = rpc(
                c,
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "StartServiceByName",
                &(BLUEZ, 0u32),
                SHORT,
            )?;
            resolve()
        }
    }
}
fn inventory(
    c: &Connection,
    shared: &Shared,
    selected: Option<u64>,
    test: bool,
) -> Result<Inventory> {
    let owner = service_owner(c, !test)?;
    let uid: u32 = rpc(
        c,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "GetConnectionUnixUser",
        &(owner.as_str(),),
        SHORT,
    )?;
    ensure!(
        uid == if test { unsafe { libc::geteuid() } } else { 0 },
        "Untrusted Bluetooth service"
    );
    let objects: Objects = rpc(
        c,
        owner.as_str(),
        "/",
        "org.freedesktop.DBus.ObjectManager",
        "GetManagedObjects",
        &(),
        SHORT,
    )?;
    ensure!(objects.len() <= 512, "Bluetooth inventory too large");
    let mut registry = shared.registry.lock().unwrap();
    registry.bind(owner.as_str())?;
    let mut seen = HashSet::new();
    let mut snapshot = Snapshot {
        available: true,
        epoch: registry.epoch,
        ..Default::default()
    };
    let mut adapters = HashMap::new();
    for (path, interfaces) in &objects {
        if let Some(props) = interfaces.get(ADAPTER) {
            ensure!(snapshot.adapters.len() < 8, "Too many Bluetooth adapters");
            let id = registry.id(path.as_str())?;
            seen.insert(path.to_string());
            snapshot.adapters.push(Adapter {
                id,
                label: label(props),
                powered: property(props, "Powered").unwrap_or(false),
                discovering: property(props, "Discovering").unwrap_or(false),
                discoverable: property(props, "Discoverable").unwrap_or(false),
            });
            adapters.insert(id, path.clone());
        }
    }
    snapshot.adapters.sort_by_key(|a| a.id);
    snapshot.selected_adapter = selected
        .filter(|id| adapters.contains_key(id))
        .or_else(|| snapshot.adapters.first().map(|a| a.id));
    let selected_path = snapshot.selected_adapter.and_then(|id| adapters.get(&id));
    let mut devices = HashMap::new();
    for (path, interfaces) in &objects {
        if let Some(props) = interfaces.get(DEVICE) {
            let adapter: Option<OwnedObjectPath> = property(props, "Adapter");
            if adapter.as_ref() != selected_path {
                continue;
            }
            ensure!(snapshot.devices.len() < 128, "Too many Bluetooth devices");
            let id = registry.id(path.as_str())?;
            seen.insert(path.to_string());
            snapshot.devices.push(Device {
                id,
                label: label(props),
                paired: property(props, "Paired").unwrap_or(false),
                trusted: property(props, "Trusted").unwrap_or(false),
                connected: property(props, "Connected").unwrap_or(false),
                blocked: property(props, "Blocked").unwrap_or(false),
            });
            devices.insert(id, path.clone());
        }
    }
    registry.ids.retain(|path, _| seen.contains(path));
    snapshot.devices.sort_by(|a, b| {
        b.paired
            .cmp(&a.paired)
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
            .then(a.id.cmp(&b.id))
    });
    Ok(Inventory {
        snapshot,
        owner: owner.to_string(),
        adapters,
        devices,
    })
}
fn signals(
    shared: Arc<Shared>,
    active: Arc<Mutex<bool>>,
    c: &Connection,
    owner: &str,
) -> Result<JoinHandle<()>> {
    let mut stream = MessageIterator::from(c);
    for rule in [
        format!(
            "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='{BLUEZ}'"
        ),
        format!(
            "type='signal',sender='{owner}',path_namespace='/org/bluez',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged'"
        ),
        format!(
            "type='signal',sender='{owner}',path='/',interface='org.freedesktop.DBus.ObjectManager'"
        ),
    ] {
        let _: () = rpc(
            c,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "AddMatch",
            &(rule,),
            SHORT,
        )?;
    }
    let owner = owner.to_owned();
    Ok(std::thread::Builder::new()
        .name("tarot-bluetooth-events".into())
        .spawn(move || {
            for value in &mut stream {
                // Serialize retirement with each callback, so a reader which
                // was preempted here cannot clear a replacement connection.
                let active_guard = active.lock().unwrap();
                if shared.stop.load(Ordering::Acquire) || !*active_guard {
                    break;
                }
                let Ok(value) = value else { break };
                let h = value.header();
                if h.sender()
                    .is_some_and(|s| s.as_str() == "org.freedesktop.DBus")
                    && h.member().is_some_and(|m| m.as_str() == "NameOwnerChanged")
                {
                    if value
                        .body()
                        .deserialize::<(String, String, String)>()
                        .is_ok_and(|(name, old, new)| name == BLUEZ && old != new)
                    {
                        shared.lost.store(true, Ordering::Release);
                        shared.agent.clear();
                        shared.registry.lock().unwrap().reset();
                        shared.wake();
                    }
                } else if h.sender().is_some_and(|s| s.as_str() == owner)
                    && h.message_type() == zbus::message::Type::Signal
                {
                    if h.member()
                        .is_some_and(|m| m.as_str() == "InterfacesRemoved")
                        && let Ok((path, interfaces)) =
                            value.body().deserialize::<(OwnedObjectPath, Vec<String>)>()
                        && interfaces.iter().any(|v| v == ADAPTER || v == DEVICE)
                    {
                        shared.registry.lock().unwrap().remove(path.as_str());
                        shared.agent.removed(path.as_str());
                    }
                    shared.wake();
                }
            }
            // Closing an old connection can finish its reader after a new
            // connection has started. It must not retire the new agent.
            if *active.lock().unwrap() {
                shared.lost.store(true, Ordering::Release);
                shared.agent.clear();
                shared.wake();
            }
        })?)
}

struct Visibility {
    owner: String,
    path: OwnedObjectPath,
    original: Vec<(&'static str, OwnedValue)>,
    deadline: Instant,
}
impl Visibility {
    fn start(c: &Connection, i: &Inventory, id: u64) -> Result<Self> {
        let path = i.adapters.get(&id).context("Adapter changed")?.clone();
        let props: Properties = rpc(
            c,
            &i.owner,
            path.as_str(),
            PROPERTIES,
            "GetAll",
            &(ADAPTER,),
            SHORT,
        )?;
        let original = [
            "Discoverable",
            "Pairable",
            "DiscoverableTimeout",
            "PairableTimeout",
        ]
        .into_iter()
        .filter_map(|key| {
            props
                .get(key)
                .and_then(|v| v.try_clone().ok())
                .map(|v| (key, v))
        })
        .collect();
        let lease = Self {
            owner: i.owner.clone(),
            path,
            original,
            deadline: Instant::now() + Duration::from_secs(120),
        };
        let result = (|| {
            set(
                c,
                &lease.owner,
                lease.path.as_str(),
                ADAPTER,
                "DiscoverableTimeout",
                120u32,
            )?;
            set(
                c,
                &lease.owner,
                lease.path.as_str(),
                ADAPTER,
                "PairableTimeout",
                120u32,
            )?;
            set(
                c,
                &lease.owner,
                lease.path.as_str(),
                ADAPTER,
                "Pairable",
                true,
            )?;
            set(
                c,
                &lease.owner,
                lease.path.as_str(),
                ADAPTER,
                "Discoverable",
                true,
            )
        })();
        if let Err(error) = result {
            lease.restore(c);
            return Err(error);
        }
        Ok(lease)
    }
    fn restore(&self, c: &Connection) {
        let current: Result<String> = rpc(
            c,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetNameOwner",
            &(BLUEZ,),
            SHORT,
        );
        if !current.is_ok_and(|owner| owner == self.owner) {
            return;
        }
        let props: Result<Properties> = rpc(
            c,
            &self.owner,
            self.path.as_str(),
            PROPERTIES,
            "GetAll",
            &(ADAPTER,),
            SHORT,
        );
        if let Ok(props) = props {
            for (key, value) in &self.original {
                let ours = match *key {
                    "Discoverable" | "Pairable" => property::<bool>(&props, key) == Some(true),
                    _ => property::<u32>(&props, key) == Some(120),
                };
                if ours {
                    let _: Result<()> = rpc(
                        c,
                        &self.owner,
                        self.path.as_str(),
                        PROPERTIES,
                        "Set",
                        &(ADAPTER, *key, value),
                        SHORT,
                    );
                }
            }
        }
    }
}
struct Operation {
    kind: Pending,
    path: OwnedObjectPath,
    owner: String,
    receiver: mpsc::Receiver<Result<()>>,
    thread: Option<JoinHandle<()>>,
    canceled: bool,
}
impl Operation {
    fn finish_thread(&mut self) {
        if let Some(thread) = self.thread.take().filter(|t| t.is_finished()) {
            let _ = thread.join();
        }
    }
}
fn cancel_pair(c: &Connection, operation: &mut Option<Operation>) {
    if let Some(op) = operation
        && matches!(op.kind, Pending::Pair(_))
        && !op.canceled
    {
        op.canceled = true;
        let _: Result<()> = rpc(
            c,
            &op.owner,
            op.path.as_str(),
            DEVICE,
            "CancelPairing",
            &(),
            SHORT,
        );
    }
}
fn cancel_device(c: &Connection, i: &Inventory, device: u64) {
    if let Some(path) = i.devices.get(&device)
        && i.snapshot
            .devices
            .iter()
            .any(|d| d.id == device && !d.paired)
    {
        let _: Result<()> = rpc(
            c,
            &i.owner,
            path.as_str(),
            DEVICE,
            "CancelPairing",
            &(),
            SHORT,
        );
    }
}
fn stop_scan(c: &Connection, owner: &str, scan: &mut Option<OwnedObjectPath>) -> Result<()> {
    if let Some(path) = scan.as_ref() {
        rpc::<_, ()>(
            c,
            owner,
            path.as_str(),
            ADAPTER,
            "StopDiscovery",
            &(),
            SHORT,
        )?;
        // A denied or timed-out stop does not prove our discovery session ended.
        *scan = None;
    }
    Ok(())
}
fn error_label(error: &anyhow::Error) -> String {
    if let Some(zbus::Error::MethodError(name, _, _)) = error.downcast_ref::<zbus::Error>() {
        return match name.as_str() {
            "org.bluez.Error.NotAuthorized" | "org.freedesktop.DBus.Error.AccessDenied" => {
                "Bluetooth action needs system authorization"
            }
            "org.bluez.Error.AuthenticationCanceled" => "Pairing canceled",
            "org.bluez.Error.AuthenticationRejected" => "Pairing rejected by the device",
            "org.bluez.Error.AuthenticationFailed" => {
                "Pairing failed; check the code and try again"
            }
            "org.bluez.Error.AuthenticationTimeout" => "Pairing timed out",
            "org.bluez.Error.InProgress" => "The Bluetooth device is already busy",
            "org.bluez.Error.NotReady" => "Bluetooth is off, blocked or not ready",
            _ => "Bluetooth action failed; check the device and try again",
        }
        .into();
    }
    "Bluetooth action did not complete; check device state before retrying".into()
}
fn begin_operation(
    shared: &Arc<Shared>,
    c: &Connection,
    i: &Inventory,
    action: UiAction,
) -> Result<Operation> {
    let (id, method, kind) = match action {
        UiAction::Pair { device } => (device, "Pair", Pending::Pair(device)),
        UiAction::Connect { device } => (device, "Connect", Pending::Connect(device)),
        UiAction::Disconnect { device } => (device, "Disconnect", Pending::Disconnect(device)),
        UiAction::Forget { device } => (device, "RemoveDevice", Pending::Forget(device)),
        UiAction::Trust { device, .. } => (device, "Set", Pending::Trust(device)),
        _ => bail!("Not a device operation"),
    };
    let path = i
        .devices
        .get(&id)
        .context("Device is no longer available")?
        .clone();
    let device = i
        .snapshot
        .devices
        .iter()
        .find(|d| d.id == id)
        .context("Device changed")?;
    ensure!(!device.blocked, "Device is blocked");
    ensure!(
        i.snapshot.adapter().is_some_and(|a| a.powered),
        "Bluetooth is off"
    );
    if method == "Pair" {
        ensure!(
            !device.paired && i.snapshot.agent_available,
            "Pairing not available"
        );
    }
    let adapter_path = i
        .snapshot
        .selected_adapter
        .and_then(|id| i.adapters.get(&id))
        .cloned()
        .context("Adapter changed")?;
    ensure!(
        shared
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok(),
        "Bluetooth operation still ending"
    );
    if method == "Pair" {
        shared.agent.arm(path.as_str());
    }
    let connection = c.clone();
    let state = shared.clone();
    let owner = i.owner.clone();
    let target = path.clone();
    let (sender, receiver) = mpsc::sync_channel(1);
    let thread = match std::thread::Builder::new()
        .name("tarot-bluetooth-call".into())
        .spawn(move || {
            let result = match action {
                UiAction::Trust { trusted, .. } => set(
                    &connection,
                    &owner,
                    target.as_str(),
                    DEVICE,
                    "Trusted",
                    trusted,
                ),
                UiAction::Forget { .. } => rpc(
                    &connection,
                    &owner,
                    adapter_path.as_str(),
                    ADAPTER,
                    "RemoveDevice",
                    &(target.clone(),),
                    SHORT,
                ),
                _ => rpc(
                    &connection,
                    &owner,
                    target.as_str(),
                    DEVICE,
                    method,
                    &(),
                    Duration::from_secs(90),
                ),
            };
            let _ = sender.send(result);
            state.busy.store(false, Ordering::Release);
            state.wake();
        }) {
        Ok(thread) => thread,
        Err(error) => {
            shared.busy.store(false, Ordering::Release);
            return Err(error.into());
        }
    };
    Ok(Operation {
        kind,
        path,
        owner: i.owner.clone(),
        receiver,
        thread: Some(thread),
        canceled: false,
    })
}
fn supervise(shared: Arc<Shared>, receiver: mpsc::Receiver<Event>, bus: Option<String>) {
    let mut failures = 0u32;
    while !shared.stop.load(Ordering::Acquire) {
        shared.lost.store(false, Ordering::Release);
        shared.wake_queued.store(false, Ordering::Release);
        let started = Instant::now();
        let result = run(&shared, &receiver, bus.as_deref());
        shared.agent.clear();
        shared.registry.lock().unwrap().reset();
        if result.is_err() {
            shared.publish(Snapshot {
                error: Some("Bluetooth service unavailable; reconnecting".into()),
                ..Default::default()
            });
        }
        if started.elapsed() >= Duration::from_secs(30) {
            failures = 0;
        }
        let deadline = Instant::now() + Duration::from_millis(250 << failures.min(4));
        failures = failures.saturating_add(1);
        while !shared.stop.load(Ordering::Acquire) && Instant::now() < deadline {
            match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Event::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Ok(Event::Action(_)) => {}
                Ok(Event::Wake)
                | Ok(Event::Context)
                | Ok(Event::CancelDevice(_))
                | Err(mpsc::RecvTimeoutError::Timeout) => {
                    shared.wake_queued.store(false, Ordering::Release);
                }
            }
        }
    }
}
fn run(shared: &Arc<Shared>, receiver: &mpsc::Receiver<Event>, bus: Option<&str>) -> Result<()> {
    let builder = if let Some(bus) = bus {
        Builder::address(bus)?
    } else {
        Builder::system()?
    };
    let connection = builder
        .method_timeout(Duration::from_secs(90))
        .serve_at(agent::PATH, agent::Agent(shared.agent.clone()))?
        .build()?;
    let owner = service_owner(&connection, bus.is_none())?;
    let watcher_active = Arc::new(Mutex::new(true));
    let watcher = signals(shared.clone(), watcher_active.clone(), &connection, &owner)?;
    let mut scan = None;
    let mut visible: Option<Visibility> = None;
    let mut operation: Option<Operation> = None;
    let result = (|| -> Result<()> {
        let mut selected = None;
        let mut registered = false;
        let mut default_agent = false;
        let mut registration_retry = Instant::now();
        let mut action_error = None;
        loop {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            ensure!(
                !shared.lost.load(Ordering::Acquire),
                "Bluetooth service replaced"
            );
            shared.wake_queued.store(false, Ordering::Release);
            let mut i = inventory(&connection, shared, selected, bus.is_some())?;
            ensure!(i.owner == owner, "Bluetooth service replaced");
            selected = i.snapshot.selected_adapter;
            if scan.as_ref().is_some_and(|path| {
                !i.snapshot.adapters.iter().any(|adapter| {
                    i.adapters.get(&adapter.id) == Some(path)
                        && adapter.powered
                        && adapter.discovering
                })
            }) {
                scan = None;
            }
            if shared.agent_released.swap(false, Ordering::AcqRel) {
                registered = false;
                default_agent = false;
            }
            if !registered && Instant::now() >= registration_retry {
                registered = rpc::<_, ()>(
                    &connection,
                    &owner,
                    "/org/bluez",
                    "org.bluez.AgentManager1",
                    "RegisterAgent",
                    &(OwnedObjectPath::try_from(agent::PATH)?, "KeyboardDisplay"),
                    SHORT,
                )
                .is_ok();
                registration_retry = Instant::now() + Duration::from_secs(4);
                if registered {
                    default_agent = rpc::<_, ()>(
                        &connection,
                        &owner,
                        "/org/bluez",
                        "org.bluez.AgentManager1",
                        "RequestDefaultAgent",
                        &(OwnedObjectPath::try_from(agent::PATH)?,),
                        SHORT,
                    )
                    .is_ok();
                }
            }
            i.snapshot.agent_available = registered;
            i.snapshot.default_agent = default_agent;
            let current_adapter = selected
                .and_then(|id| i.adapters.get(&id))
                .map(|p| p.as_str())
                .unwrap_or("");
            if visible.as_ref().is_some_and(|lease| {
                Instant::now() >= lease.deadline
                    || lease.path.as_str() != current_adapter
                    || !shared.open.load(Ordering::Acquire)
                    || !i
                        .snapshot
                        .adapter()
                        .is_some_and(|a| a.powered && a.discoverable)
            }) && let Some(lease) = visible.take()
            {
                lease.restore(&connection);
            }
            if !shared.open.load(Ordering::Acquire) || !shared.unlocked.load(Ordering::Acquire) {
                shared.agent.clear();
                cancel_pair(&connection, &mut operation);
                stop_scan(&connection, &owner, &mut scan)?;
            }
            shared
                .agent
                .bind(&owner, i.snapshot.epoch, current_adapter, visible.is_some());
            let expiring_device = shared.agent.prompt().map(|p| p.device);
            if shared.agent.expire() {
                let own_pair = operation.as_ref().is_some_and(
                    |op| matches!(op.kind, Pending::Pair(id) if Some(id) == expiring_device),
                );
                cancel_pair(&connection, &mut operation);
                if !own_pair && let Some(device) = expiring_device {
                    cancel_device(&connection, &i, device);
                }
            }
            if let Some(op) = &mut operation {
                let done = match op.receiver.try_recv() {
                    Ok(result) => Some(result),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Some(Err(anyhow::anyhow!("Bluetooth operation ended")))
                    }
                    Err(mpsc::TryRecvError::Empty) => None,
                };
                if let Some(done) = done {
                    op.finish_thread();
                    action_error = done.err().map(|e| error_label(&e));
                    shared.agent.disarm();
                    operation = None;
                    // Read back actual state rather than assuming the method reply
                    // alone means every profile is connected.
                    i = inventory(&connection, shared, selected, bus.is_some())?;
                    i.snapshot.agent_available = registered;
                    i.snapshot.default_agent = default_agent;
                }
            }
            i.snapshot.scanning = scan.is_some();
            i.snapshot.visible = visible.is_some();
            i.snapshot.pending = operation.as_ref().map(|op| op.kind);
            i.snapshot.busy = shared.busy.load(Ordering::Acquire);
            i.snapshot.prompt = shared.agent.prompt();
            i.snapshot.action_error = action_error.clone();
            shared.publish(i.snapshot.clone());
            let mut timeout = if operation.is_some() {
                Some(Duration::from_millis(250))
            } else if !registered {
                Some(registration_retry.saturating_duration_since(Instant::now()))
            } else {
                None
            };
            for deadline in [
                shared.agent.deadline(),
                visible.as_ref().map(|v| v.deadline),
            ]
            .into_iter()
            .flatten()
            {
                let delay = deadline.saturating_duration_since(Instant::now());
                timeout = Some(timeout.map_or(delay, |old| old.min(delay)));
            }
            let event = if let Some(timeout) = timeout {
                match receiver.recv_timeout(timeout) {
                    Ok(e) => e,
                    Err(mpsc::RecvTimeoutError::Timeout) => Event::Wake,
                    Err(_) => Event::Stop,
                }
            } else {
                receiver.recv().unwrap_or(Event::Stop)
            };
            match event {
                Event::Stop => return Ok(()),
                Event::Wake => {}
                Event::Context => {
                    if !default_agent && registered && shared.open.load(Ordering::Acquire) {
                        default_agent = rpc::<_, ()>(
                            &connection,
                            &owner,
                            "/org/bluez",
                            "org.bluez.AgentManager1",
                            "RequestDefaultAgent",
                            &(OwnedObjectPath::try_from(agent::PATH)?,),
                            SHORT,
                        )
                        .is_ok();
                    }
                }
                Event::CancelDevice(device) => {
                    if operation
                        .as_ref()
                        .is_some_and(|op| matches!(op.kind,Pending::Pair(id) if id==device))
                    {
                        cancel_pair(&connection, &mut operation);
                    } else {
                        cancel_device(&connection, &i, device);
                    }
                }
                Event::Action(UiAction::CancelPairing) => {
                    shared.agent.disarm();
                    cancel_pair(&connection, &mut operation);
                }
                Event::Action(action) => {
                    if !shared.open.load(Ordering::Acquire)
                        || !shared.unlocked.load(Ordering::Acquire)
                    {
                        continue;
                    }
                    let action_result = (|| -> Result<()> {
                        ensure!(
                            operation.is_none() && !shared.busy.load(Ordering::Acquire),
                            "Bluetooth operation still ending"
                        );
                        // Actions resolve against a fresh owner/token inventory.
                        let mut current = inventory(&connection, shared, selected, bus.is_some())?;
                        ensure!(current.owner == owner, "Bluetooth service replaced");
                        current.snapshot.agent_available = registered;
                        match action {
                            UiAction::Adapter { id } => {
                                ensure!(current.adapters.contains_key(&id), "Adapter changed");
                                stop_scan(&connection, &owner, &mut scan)?;
                                if let Some(lease) = visible.take() {
                                    lease.restore(&connection);
                                }
                                shared.agent.clear();
                                selected = Some(id);
                            }
                            UiAction::Radio { adapter, enabled } => {
                                ensure!(Some(adapter) == selected, "Adapter changed");
                                let path =
                                    current.adapters.get(&adapter).context("Adapter changed")?;
                                if !enabled {
                                    stop_scan(&connection, &owner, &mut scan)?;
                                    if let Some(lease) = visible.take() {
                                        lease.restore(&connection);
                                    }
                                }
                                set(
                                    &connection,
                                    &owner,
                                    path.as_str(),
                                    ADAPTER,
                                    "Powered",
                                    enabled,
                                )?;
                            }
                            UiAction::Scan { adapter, enabled } => {
                                ensure!(Some(adapter) == selected, "Adapter changed");
                                let path =
                                    current.adapters.get(&adapter).context("Adapter changed")?;
                                if enabled && scan.is_none() {
                                    scan = Some(path.clone());
                                    let result: Result<()> = rpc(
                                        &connection,
                                        &owner,
                                        path.as_str(),
                                        ADAPTER,
                                        "StartDiscovery",
                                        &(),
                                        SHORT,
                                    );
                                    if result.is_err() {
                                        stop_scan(&connection, &owner, &mut scan)?;
                                    }
                                    result?;
                                }
                                if !enabled {
                                    stop_scan(&connection, &owner, &mut scan)?;
                                }
                            }
                            UiAction::Visible { adapter, enabled } => {
                                ensure!(
                                    Some(adapter) == selected && default_agent,
                                    "Incoming pairing unavailable"
                                );
                                if enabled && visible.is_none() {
                                    visible =
                                        Some(Visibility::start(&connection, &current, adapter)?);
                                }
                                if !enabled && let Some(lease) = visible.take() {
                                    lease.restore(&connection);
                                }
                            }
                            UiAction::Pair { .. }
                            | UiAction::Connect { .. }
                            | UiAction::Disconnect { .. }
                            | UiAction::Forget { .. }
                            | UiAction::Trust { .. } => {
                                operation =
                                    Some(begin_operation(shared, &connection, &current, action)?);
                            }
                            UiAction::CancelPairing | UiAction::Reply { .. } => {}
                        }
                        Ok(())
                    })();
                    action_error = action_result.err().map(|e| error_label(&e));
                }
            }
        }
    })();
    *watcher_active.lock().unwrap() = false;
    shared.agent.clear();
    cancel_pair(&connection, &mut operation);
    let _ = stop_scan(&connection, &owner, &mut scan);
    if let Some(lease) = visible.take() {
        lease.restore(&connection);
    }
    let _ = connection.close();
    if watcher.is_finished() {
        let _ = watcher.join();
    }
    if let Some(mut op) = operation {
        op.finish_thread();
    }
    result
}
