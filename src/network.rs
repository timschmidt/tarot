//! NetworkManager adapter. Calls use its unique owner, and rows use opaque tokens
//! that cannot survive daemon/bus replacement. Never requests stored secrets.
mod agent;
use crate::{CalloopData, Compositor};
use anyhow::{Context, Result, bail, ensure};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot_core::network::{
    Network, PasswordKind, Pending, Permission, Profile, Security, Snapshot, State, UiAction,
    ssid_label,
};
use zbus::{
    blocking::{Connection, MessageIterator, Proxy, connection::Builder, fdo::DBusProxy},
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
const NM: &str = "org.freedesktop.NetworkManager";
const ROOT: &str = "/org/freedesktop/NetworkManager";
const DEVICE: &str = "org.freedesktop.NetworkManager.Device";
const WIFI: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const AP: &str = "org.freedesktop.NetworkManager.AccessPoint";
const SETTINGS: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const ACTIVE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const SCAN: &str = "org.freedesktop.NetworkManager.wifi.scan";
const CONTROL: &str = "org.freedesktop.NetworkManager.network-control";
const RADIO: &str = "org.freedesktop.NetworkManager.enable-disable-wifi";
const CREATE: &str = "org.freedesktop.NetworkManager.settings.modify.own";

enum Event {
    Wake,
    Action(UiAction),
    Stop,
}
struct Shared {
    updates: Mutex<Option<Snapshot>>,
    latest: Mutex<Snapshot>,
    ping: Ping,
    sender: mpsc::SyncSender<Event>,
    stop: AtomicBool,
    lost: AtomicBool,
    busy: AtomicBool,
    watching: AtomicBool,
    wake_queued: AtomicBool,
    agent: Arc<agent::State>,
}
impl Shared {
    fn publish(&self, mut value: Snapshot, force: bool) {
        value.enabled = true;
        value.busy = self.busy.load(Ordering::Acquire);
        let mut latest = self.latest.lock().unwrap();
        value.generation = latest.generation;
        if *latest == value && !force {
            return;
        }
        value.generation += 1;
        *latest = value.clone();
        *self.updates.lock().unwrap() = Some(value);
        self.ping.ping();
    }
    fn wake(&self) {
        if !self.wake_queued.swap(true, Ordering::AcqRel) {
            let _ = self.sender.try_send(Event::Wake);
        }
    }
}
pub struct Server {
    shared: Arc<Shared>,
    connection: Arc<Mutex<Option<Connection>>>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn watch(&self, watching: bool) {
        self.shared.watching.store(watching, Ordering::Release);
        self.shared.wake();
    }
    pub fn action(&self, action: UiAction) -> Result<()> {
        match action {
            UiAction::Secrets { request, password } => {
                return self.shared.agent.respond(request, Some(password));
            }
            UiAction::CancelSecrets { request } => return self.shared.agent.respond(request, None),
            _ => {}
        }
        ensure!(
            self.shared
                .busy
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            "A Wi-Fi operation is still pending"
        );
        if self.shared.sender.try_send(Event::Action(action)).is_err() {
            self.shared.busy.store(false, Ordering::Release);
            bail!("Wi-Fi service is busy or unavailable");
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
    bus: Option<String>,
) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let (sender, receiver) = mpsc::sync_channel(64);
    let shared = Arc::new_cyclic(|weak| Shared {
        updates: Mutex::new(None),
        latest: Mutex::new(Snapshot::default()),
        ping,
        sender,
        stop: AtomicBool::new(false),
        lost: AtomicBool::new(false),
        busy: AtomicBool::new(false),
        watching: AtomicBool::new(false),
        wake_queued: AtomicBool::new(false),
        agent: Arc::new(agent::State::new(weak.clone())),
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |(), _, data| {
            if let Some(mut snapshot) = updates.updates.lock().unwrap().take() {
                snapshot.busy = updates.busy.load(Ordering::Acquire);
                let before = data.state.shell.network_input.as_ref().map(|i| i.target);
                let mut effects = data.state.shell.sync_network(snapshot);
                if data.state.protected() {
                    effects.extend(data.state.shell.cancel_network_input());
                    data.state.shell.system_menu_open = false;
                    data.state.shell.launcher.menu_open = false;
                    data.state.shell.menus.dismiss();
                }
                if before != data.state.shell.network_input.as_ref().map(|i| i.target) {
                    data.state.cancel_input_sequences();
                }
                data.state.apply_effects(effects);
                data.state.refresh_keyboard();
                data.state.ui_dirty = true;
                data.state.redraw = true;
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let connection = Arc::new(Mutex::new(None));
    let saved = connection.clone();
    let worker_shared = shared.clone();
    state.shell.network.enabled = true;
    let worker = std::thread::Builder::new()
        .name("tarot-wifi".into())
        .spawn(move || supervise(worker_shared, receiver, saved, bus))?;
    Ok(Server {
        shared,
        connection,
        worker: Some(worker),
    })
}

#[derive(Default)]
struct Registry {
    owner: String,
    epoch: u64,
    next: u64,
    ids: HashMap<String, u64>,
    seen: HashSet<String>,
}
impl Registry {
    fn bind(&mut self, owner: &str) {
        if self.owner != owner {
            self.owner = owner.to_owned();
            self.ids.clear();
            self.epoch += 1;
        }
        self.seen.clear();
    }
    fn disconnected(&mut self) {
        self.owner.clear();
        self.ids.clear();
    }
    fn id(&mut self, key: String) -> Result<u64> {
        self.seen.insert(key.clone());
        if let Some(id) = self.ids.get(&key) {
            return Ok(*id);
        }
        self.next = self
            .next
            .checked_add(1)
            .context("Wi-Fi token space exhausted")?;
        self.ids.insert(key, self.next);
        Ok(self.next)
    }
    fn finish(&mut self) {
        self.ids.retain(|key, _| self.seen.contains(key));
    }
}
#[derive(Clone)]
struct AccessPoint {
    path: OwnedObjectPath,
    ssid: Vec<u8>,
    security: Security,
    strength: u8,
    wpa: u32,
    rsn: u32,
}
struct SavedProfile {
    token: u64,
    path: OwnedObjectPath,
    ssid: Vec<u8>,
    key_mgmt: String,
    label: String,
}
struct Inventory {
    snapshot: Snapshot,
    owner: String,
    device: OwnedObjectPath,
    aps: HashMap<u64, Vec<AccessPoint>>,
    profiles: Vec<SavedProfile>,
    active: OwnedObjectPath,
}
struct Operation {
    epoch: u64,
    active: Option<OwnedObjectPath>,
    kind: Pending,
    deadline: Instant,
    last_scan: i64,
    profile: Option<u64>,
    profile_path: Option<OwnedObjectPath>,
    observed_active: bool,
}

fn supervise(
    shared: Arc<Shared>,
    receiver: mpsc::Receiver<Event>,
    saved: Arc<Mutex<Option<Connection>>>,
    bus: Option<String>,
) {
    let mut registry = Registry::default();
    let mut failures: u32 = 0;
    while !shared.stop.load(Ordering::Acquire) {
        shared.lost.store(false, Ordering::Release);
        let started = Instant::now();
        let result = run(&shared, &receiver, &saved, bus.as_deref(), &mut registry);
        shared.agent.clear();
        let connection = saved.lock().unwrap().take();
        if let Some(connection) = connection {
            let _ = connection.close();
        }
        registry.disconnected();
        shared.busy.store(false, Ordering::Release);
        if let Err(_error) = result {
            shared.publish(
                Snapshot {
                    epoch: registry.epoch,
                    error: Some("Wi-Fi service unavailable; reconnecting".into()),
                    ..Default::default()
                },
                true,
            );
        }
        if started.elapsed() >= Duration::from_secs(30) {
            failures = 0;
        }
        let deadline = Instant::now() + Duration::from_millis(250 << failures.min(4));
        failures = failures.saturating_add(1);
        while !shared.stop.load(Ordering::Acquire) && Instant::now() < deadline {
            match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Event::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Ok(Event::Action(_)) => {
                    shared.busy.store(false, Ordering::Release);
                    let mut value = shared.latest.lock().unwrap().clone();
                    value.action_error = Some("Wi-Fi service unavailable".into());
                    shared.publish(value, true);
                }
                Ok(Event::Wake) | Err(mpsc::RecvTimeoutError::Timeout) => {
                    shared.wake_queued.store(false, Ordering::Release);
                }
            }
        }
    }
}
fn watch(
    shared: Arc<Shared>,
    connection: &Connection,
    rule: &str,
    owner: bool,
) -> Result<JoinHandle<()>> {
    let messages = MessageIterator::for_match_rule(rule, connection, Some(64))?;
    Ok(std::thread::Builder::new()
        .name("tarot-wifi-events".into())
        .spawn(move || {
            for message in messages {
                let Ok(message) = message else {
                    break;
                };
                let relevant = if owner {
                    message
                        .body()
                        .deserialize::<(String, String, String)>()
                        .is_ok_and(|(name, _, _)| name == NM)
                } else {
                    true
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
    bus: Option<&str>,
    registry: &mut Registry,
) -> Result<()> {
    let builder = if let Some(bus) = bus {
        Builder::address(bus)?
    } else {
        Builder::system()?
    };
    let connection = builder
        .method_timeout(Duration::from_secs(5))
        .serve_at(agent::PATH, agent::Agent(shared.agent.clone()))?
        .build()?;
    *saved.lock().unwrap() = Some(connection.clone());
    if shared.stop.load(Ordering::Acquire) {
        return Ok(());
    }
    let properties = watch(
        shared.clone(),
        &connection,
        "type='signal',path_namespace='/org/freedesktop/NetworkManager'",
        false,
    )?;
    let owners = match watch(
        shared.clone(),
        &connection,
        "type='signal',interface='org.freedesktop.DBus',member='NameOwnerChanged'",
        true,
    ) {
        Ok(value) => value,
        Err(error) => {
            let _ = connection.close();
            let _ = properties.join();
            return Err(error);
        }
    };
    let result = (|| -> Result<()> {
        let mut operation: Option<Operation> = None;
        let mut action_error = None;
        let mut agent_owner = String::new();
        let mut agent_available = false;
        let mut agent_retry = Instant::now();
        loop {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            if shared.lost.load(Ordering::Acquire) {
                bail!("system bus disconnected");
            }
            let inventory = read_inventory(&connection, registry);
            if let Ok(i) = &inventory {
                if i.owner != agent_owner || !agent_available && Instant::now() >= agent_retry {
                    if i.owner != agent_owner {
                        shared.agent.clear();
                    }
                    agent_owner = i.owner.clone();
                    agent_available = Proxy::new(
                        &connection,
                        agent_owner.as_str(),
                        agent::MANAGER,
                        "org.freedesktop.NetworkManager.AgentManager",
                    )?
                    .call::<_, _, ()>("RegisterWithCapabilities", &("org.tarot.Shell", 0u32))
                    .is_ok();
                    agent_retry = Instant::now() + Duration::from_secs(4);
                }
            } else {
                agent_owner.clear();
                agent_available = false;
                shared.agent.clear();
            }
            shared.agent.expire();
            let mut snapshot = match &inventory {
                Ok(i) => i.snapshot.clone(),
                Err(_) => Snapshot {
                    epoch: registry.epoch,
                    error: Some("Wi-Fi service or device unavailable".into()),
                    ..Default::default()
                },
            };
            if inventory.is_err() {
                registry.disconnected();
            }
            snapshot.agent_available = agent_available;
            snapshot.secret_prompt = shared.agent.prompt();
            if let Some(op) = &mut operation {
                if op
                    .active
                    .as_ref()
                    .is_some_and(|active| inventory.as_ref().is_ok_and(|i| i.active == *active))
                {
                    op.observed_active = true;
                }
                if let Some(path) = &op.profile_path {
                    op.profile = inventory
                        .as_ref()
                        .ok()
                        .and_then(|i| i.profiles.iter().find(|p| p.path == *path).map(|p| p.token));
                }
                if let Some(deadline) = shared.agent.deadline() {
                    op.deadline = op.deadline.max(deadline + Duration::from_secs(10));
                }
            }
            if let Some(pending) = &operation {
                let active_matches = pending
                    .active
                    .as_ref()
                    .is_none_or(|active| inventory.as_ref().is_ok_and(|i| i.active == *active));
                if !snapshot.available || pending.epoch != snapshot.epoch {
                    action_error = Some("Wi-Fi service disappeared during the operation".into());
                    operation = None;
                } else if pending.observed_active && !active_matches {
                    action_error = Some("Connection request was superseded".into());
                    operation = None;
                } else if active_matches && pending_complete(&snapshot, pending) {
                    operation = None;
                    action_error = None;
                } else if active_matches
                    && matches!(pending.kind, Pending::Connect(_))
                    && (snapshot.state == State::Failed
                        || snapshot.state == State::CredentialsRequired && !agent_available)
                {
                    action_error = Some(if snapshot.state == State::CredentialsRequired {
                        "NetworkManager needs credentials; password entry is not available yet"
                            .into()
                    } else {
                        format!("Connection failed (reason {})", snapshot.state_reason)
                    });
                    operation = None;
                } else if Instant::now() >= pending.deadline {
                    action_error =
                        Some("Wi-Fi operation timed out; check connection state and retry".into());
                    operation = None;
                }
                if operation.is_none() {
                    shared.agent.clear();
                    snapshot.secret_prompt = None;
                    shared.busy.store(false, Ordering::Release);
                }
            }
            snapshot.pending = operation.as_ref().map(|o| o.kind.clone());
            snapshot.action_error = action_error.clone();
            shared.publish(snapshot.clone(), false);
            let mut timeout = if operation.is_some() {
                operation
                    .as_ref()
                    .map(|o| o.deadline.saturating_duration_since(Instant::now()))
            } else if !snapshot.available || !agent_available {
                Some(Duration::from_secs(4))
            } else {
                None
            };
            if let Some(deadline) = shared.agent.deadline() {
                let remaining = deadline.saturating_duration_since(Instant::now());
                timeout = Some(timeout.map_or(remaining, |d| d.min(remaining)));
            }
            let event = if let Some(timeout) = timeout {
                match receiver.recv_timeout(timeout) {
                    Ok(event) => Some(event),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => Some(Event::Stop),
                }
            } else {
                receiver.recv().ok()
            };
            match event {
                Some(Event::Stop) => return Ok(()),
                Some(Event::Wake) | None => {
                    shared.wake_queued.store(false, Ordering::Release);
                }
                Some(Event::Action(action)) => {
                    action_error = None;
                    // Refresh before resolving an intent; stale IDs never map to a new owner.
                    let fresh = read_inventory(&connection, registry);
                    match fresh.and_then(|i| request(&connection, &i, action, &shared.agent)) {
                        Ok(value) => operation = Some(value),
                        Err(_error) => {
                            shared.agent.clear();
                            shared.busy.store(false, Ordering::Release);
                            action_error = Some(
                                "Wi-Fi request rejected or no longer available; refresh and retry"
                                    .into(),
                            );
                        }
                    }
                    let mut value = shared.latest.lock().unwrap().clone();
                    value.action_error = action_error.clone();
                    value.pending = operation.as_ref().map(|o| o.kind.clone());
                    shared.publish(value, true);
                }
            }
        }
    })();
    let _ = connection.close();
    let _ = properties.join();
    let _ = owners.join();
    result
}
fn pending_complete(snapshot: &Snapshot, operation: &Operation) -> bool {
    match operation.kind {
        Pending::Scan => snapshot.last_scan > operation.last_scan,
        Pending::Radio(enabled) => snapshot.radio == enabled,
        Pending::Disconnect => !matches!(
            snapshot.state,
            State::Connected
                | State::Connecting
                | State::Disconnecting
                | State::CredentialsRequired
        ),
        Pending::Connect(id) => {
            snapshot.state == State::Connected
                && operation.profile.is_some()
                && snapshot.active_profile == operation.profile
                && snapshot.networks.iter().any(|n| n.id == id && n.connected)
        }
    }
}
fn request(
    connection: &Connection,
    inventory: &Inventory,
    action: UiAction,
    agent: &agent::State,
) -> Result<Operation> {
    let snapshot = &inventory.snapshot;
    let manager = Proxy::new(connection, inventory.owner.as_str(), ROOT, NM)?;
    let mut operation = Operation {
        epoch: snapshot.epoch,
        active: None,
        kind: Pending::Scan,
        deadline: Instant::now() + Duration::from_secs(20),
        last_scan: snapshot.last_scan,
        profile: None,
        profile_path: None,
        observed_active: false,
    };
    match action {
        UiAction::Scan => {
            ensure!(
                snapshot.scan_permission.allowed() && snapshot.radio && snapshot.hardware,
                "Wi-Fi scanning is not authorized/available"
            );
            Proxy::new(
                connection,
                inventory.owner.as_str(),
                inventory.device.as_str(),
                WIFI,
            )?
            .call::<_, _, ()>("RequestScan", &(HashMap::<String, OwnedValue>::new(),))?;
        }
        UiAction::Radio { enabled } => {
            ensure!(
                snapshot.radio_permission.allowed() && (!enabled || snapshot.hardware),
                "Wi-Fi radio control is not authorized/available"
            );
            Proxy::new(
                connection,
                inventory.owner.as_str(),
                ROOT,
                "org.freedesktop.DBus.Properties",
            )?
            .call::<_, _, ()>("Set", &(NM, "WirelessEnabled", Value::from(enabled)))?;
            operation.kind = Pending::Radio(enabled);
        }
        UiAction::Disconnect => {
            ensure!(
                snapshot.control_permission.allowed(),
                "Wi-Fi control is not authorized"
            );
            Proxy::new(
                connection,
                inventory.owner.as_str(),
                inventory.device.as_str(),
                DEVICE,
            )?
            .call::<_, _, ()>("Disconnect", &())?;
            operation.kind = Pending::Disconnect;
        }
        UiAction::Connect { network, profile } => {
            ensure!(
                snapshot.control_permission.allowed() && snapshot.radio && snapshot.hardware,
                "Wi-Fi connection is not authorized/available"
            );
            let profile = inventory
                .profiles
                .iter()
                .find(|p| p.token == profile)
                .context("Saved network is no longer available")?;
            let ap = inventory
                .aps
                .get(&network)
                .context("Network is no longer available")?
                .iter()
                .filter(|ap| compatible(ap, profile))
                .max_by_key(|ap| ap.strength)
                .context("Saved profile does not match network security")?;
            if let Some(kind) = match profile.key_mgmt.as_str() {
                "wpa-psk" => Some(PasswordKind::Psk),
                "sae" => Some(PasswordKind::Sae),
                _ => None,
            } {
                agent.arm(&inventory.owner, &profile.path, &profile.ssid, kind);
            }
            let active: OwnedObjectPath = manager.call(
                "ActivateConnection",
                &(&profile.path, &inventory.device, &ap.path),
            )?;
            operation.kind = Pending::Connect(network);
            operation.active = Some(active);
            operation.profile = Some(profile.token);
            operation.profile_path = Some(profile.path.clone());
            operation.deadline = Instant::now() + Duration::from_secs(45);
        }
        UiAction::Create {
            network,
            password,
            remember,
        } => {
            ensure!(
                snapshot.control_permission.allowed()
                    && snapshot.create_permission.allowed()
                    && snapshot.radio
                    && snapshot.hardware,
                "New Wi-Fi connection is not authorized/available"
            );
            let ap = inventory
                .aps
                .get(&network)
                .and_then(|aps| {
                    aps.iter()
                        .filter(|ap| password_kind(ap).is_some_and(|k| k.accepts(&password)))
                        .max_by_key(|ap| ap.strength)
                })
                .context("Network/password is no longer compatible")?;
            let kind = password_kind(ap).context("Unsupported network security")?;
            let username = local_username()?;
            let mut settings = HashMap::from([
                (
                    "connection".to_owned(),
                    HashMap::from([
                        (
                            "id".to_owned(),
                            owned(format!("Tarot {}", ssid_label(&ap.ssid)))?,
                        ),
                        ("type".to_owned(), owned("802-11-wireless")?),
                        (
                            "permissions".to_owned(),
                            owned(vec![format!("user:{username}:")])?,
                        ),
                        ("autoconnect".to_owned(), owned(remember)?),
                    ]),
                ),
                (
                    "802-11-wireless".to_owned(),
                    HashMap::from([
                        ("ssid".to_owned(), owned(ap.ssid.clone())?),
                        ("mode".to_owned(), owned("infrastructure")?),
                    ]),
                ),
            ]);
            if kind != PasswordKind::Open {
                settings.insert(
                    "802-11-wireless-security".into(),
                    HashMap::from([
                        (
                            "key-mgmt".into(),
                            owned(if kind == PasswordKind::Sae {
                                "sae"
                            } else {
                                "wpa-psk"
                            })?,
                        ),
                        ("psk".into(), owned(password.expose())?),
                        (
                            "psk-flags".into(),
                            // Profile persistence controls disk storage. A
                            // NOT_SAVED flag would discard this supplied secret
                            // before AddAndActivate starts the connection.
                            owned(0u32)?,
                        ),
                    ]),
                );
            }
            let options = HashMap::from([(
                "persist",
                owned(if remember { "disk" } else { "volatile" })?,
            )]);
            let (profile, active, _): (
                OwnedObjectPath,
                OwnedObjectPath,
                HashMap<String, OwnedValue>,
            ) = manager.call(
                "AddAndActivateConnection2",
                &(&settings, &inventory.device, &ap.path, &options),
            )?;
            if kind != PasswordKind::Open {
                agent.arm(&inventory.owner, &profile, &ap.ssid, kind);
            }
            operation.kind = Pending::Connect(network);
            operation.active = Some(active);
            operation.profile_path = Some(profile);
            operation.deadline = Instant::now() + Duration::from_secs(45);
        }
        UiAction::Secrets { .. } | UiAction::CancelSecrets { .. } => {
            bail!("Password request ended")
        }
    }
    Ok(operation)
}
fn owned<'a>(value: impl Into<Value<'a>>) -> Result<OwnedValue> {
    Ok(value.into().try_into()?)
}
fn local_username() -> Result<String> {
    let uid = rustix::process::getuid().as_raw();
    let passwd = std::fs::read_to_string("/etc/passwd")?;
    passwd
        .lines()
        .find_map(|line| {
            let fields: Vec<_> = line.split(':').collect();
            (fields.len() >= 4 && fields[2].parse::<u32>().ok() == Some(uid))
                .then(|| fields[0].to_owned())
        })
        .context("Local account name unavailable")
}
fn password_kind(ap: &AccessPoint) -> Option<PasswordKind> {
    match ap.security {
        Security::Open => Some(PasswordKind::Open),
        Security::Personal if (ap.wpa | ap.rsn) & 0x100 != 0 => Some(PasswordKind::Psk),
        Security::Personal if ap.rsn & 0x400 != 0 => Some(PasswordKind::Sae),
        _ => None,
    }
}
fn properties(
    connection: &Connection,
    owner: &str,
    path: &str,
    interface: &str,
) -> Result<HashMap<String, OwnedValue>> {
    Ok(
        Proxy::new(connection, owner, path, "org.freedesktop.DBus.Properties")?
            .call("GetAll", &(interface,))?,
    )
}
fn paths(value: &OwnedValue) -> Result<Vec<OwnedObjectPath>> {
    Ok(value.try_clone()?.try_into()?)
}
fn object(value: &OwnedValue) -> Result<OwnedObjectPath> {
    Ok(value.try_clone()?.try_into()?)
}
fn compatible(ap: &AccessPoint, profile: &SavedProfile) -> bool {
    ap.ssid == profile.ssid
        && match profile.key_mgmt.as_str() {
            "" | "none" => ap.security == Security::Open,
            "wpa-psk" => (ap.wpa | ap.rsn) & (0x100 | 0x400) != 0,
            "sae" => ap.rsn & 0x400 != 0,
            "wpa-eap" | "wpa-eap-suite-b-192" => (ap.wpa | ap.rsn) & 0x200 != 0,
            _ => false,
        }
}
fn read_inventory(connection: &Connection, registry: &mut Registry) -> Result<Inventory> {
    let owner = DBusProxy::new(connection)?
        .get_name_owner(NM.try_into()?)?
        .to_string();
    registry.bind(&owner);
    let manager = Proxy::new(connection, owner.as_str(), ROOT, NM)?;
    let root = properties(connection, &owner, ROOT, NM)?;
    let radio = bool::try_from(root.get("WirelessEnabled").context("missing radio state")?)?;
    let hardware = bool::try_from(
        root.get("WirelessHardwareEnabled")
            .context("missing hardware state")?,
    )?;
    let permissions: HashMap<String, String> = manager.call("GetPermissions", &())?;
    let devices: Vec<OwnedObjectPath> = manager.call("GetDevices", &())?;
    ensure!(devices.len() <= 32, "Too many network devices");
    let mut wifi_devices = Vec::new();
    for path in devices {
        let values = properties(connection, &owner, path.as_str(), DEVICE)?;
        if values.get("DeviceType").map(u32::try_from).transpose()? == Some(2) {
            wifi_devices.push((path, values));
        }
    }
    ensure!(wifi_devices.len() == 1, "A single Wi-Fi device is required");
    let (device, values) = wifi_devices.pop().unwrap();
    let state = State::from_nm(u32::try_from(
        values.get("State").context("missing device state")?,
    )?);
    let reason: (u32, u32) = values
        .get("StateReason")
        .context("missing device reason")?
        .try_clone()?
        .try_into()?;
    let wireless = properties(connection, &owner, device.as_str(), WIFI)?;
    let active_ap = object(
        wireless
            .get("ActiveAccessPoint")
            .context("missing active access point")?,
    )?;
    let last_scan = i64::try_from(wireless.get("LastScan").context("missing scan timestamp")?)?;
    let mut ap_paths: Vec<OwnedObjectPath> =
        Proxy::new(connection, owner.as_str(), device.as_str(), WIFI)?
            .call("GetAllAccessPoints", &())?;
    ensure!(ap_paths.len() <= 256, "Too many access points");
    if active_ap.as_str() != "/" && !ap_paths.contains(&active_ap) {
        ap_paths.push(active_ap.clone());
    }
    let available = paths(
        values
            .get("AvailableConnections")
            .context("missing saved profiles")?,
    )?;
    ensure!(available.len() <= 128, "Too many saved profiles");
    let mut profiles = Vec::new();
    for path in available {
        let settings: HashMap<String, HashMap<String, OwnedValue>> =
            match Proxy::new(connection, owner.as_str(), path.as_str(), SETTINGS)?
                .call("GetSettings", &())
            {
                Ok(value) => value,
                Err(_) => continue,
            };
        let Some(wireless) = settings.get("802-11-wireless") else {
            continue;
        };
        let Some(ssid) = wireless.get("ssid") else {
            continue;
        };
        let ssid: Vec<u8> = ssid.try_clone()?.try_into()?;
        if ssid.is_empty() || ssid.len() > 32 {
            continue;
        }
        let key_mgmt = settings
            .get("802-11-wireless-security")
            .and_then(|s| s.get("key-mgmt"))
            .map(<&str>::try_from)
            .transpose()?
            .unwrap_or("")
            .to_owned();
        let label = settings
            .get("connection")
            .and_then(|s| s.get("id"))
            .map(<&str>::try_from)
            .transpose()?
            .map(|s| ssid_label(s.as_bytes()))
            .unwrap_or_else(|| "Saved profile".into());
        let uuid = settings
            .get("connection")
            .and_then(|s| s.get("uuid"))
            .map(<&str>::try_from)
            .transpose()?
            .unwrap_or("");
        let key = format!("profile:{}:{uuid}:{ssid:?}:{key_mgmt}", path.as_str());
        let token = registry.id(key)?;
        profiles.push(SavedProfile {
            token,
            path,
            ssid,
            key_mgmt,
            label,
        });
    }
    let mut groups: HashMap<(Vec<u8>, Security), Vec<AccessPoint>> = HashMap::new();
    for path in ap_paths {
        let values = match properties(connection, &owner, path.as_str(), AP) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let ssid: Vec<u8> = values
            .get("Ssid")
            .context("missing SSID")?
            .try_clone()?
            .try_into()?;
        if ssid.is_empty() || ssid.len() > 32 {
            continue;
        }
        let flags = u32::try_from(values.get("Flags").context("missing AP flags")?)?;
        let wpa = u32::try_from(values.get("WpaFlags").context("missing WPA flags")?)?;
        let rsn = u32::try_from(values.get("RsnFlags").context("missing RSN flags")?)?;
        let strength =
            u8::try_from(values.get("Strength").context("missing AP strength")?)?.min(100);
        let security = Security::from_flags(flags, wpa, rsn);
        groups
            .entry((ssid.clone(), security))
            .or_default()
            .push(AccessPoint {
                path,
                ssid,
                security,
                strength,
                wpa,
                rsn,
            });
    }
    let mut networks = Vec::new();
    let mut aps = HashMap::new();
    for ((ssid, security), candidates) in groups {
        let id = registry.id(format!("network:{device}:{ssid:?}:{security:?}"))?;
        let compatible_profiles = profiles
            .iter()
            .filter(|profile| candidates.iter().any(|ap| compatible(ap, profile)))
            .map(|profile| Profile {
                id: profile.token,
                label: profile.label.clone(),
            })
            .collect();
        networks.push(Network {
            id,
            label: ssid_label(&ssid),
            strength: candidates.iter().map(|ap| ap.strength).max().unwrap_or(0),
            security,
            connected: state == State::Connected
                && candidates.iter().any(|ap| ap.path == active_ap),
            profiles: compatible_profiles,
            password_kind: candidates
                .iter()
                .max_by_key(|ap| ap.strength)
                .and_then(password_kind),
        });
        aps.insert(id, candidates);
    }
    networks.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| (!b.profiles.is_empty()).cmp(&(!a.profiles.is_empty())))
            .then_with(|| b.strength.cmp(&a.strength))
            .then_with(|| a.label.cmp(&b.label))
    });
    let active = object(
        values
            .get("ActiveConnection")
            .context("missing active connection")?,
    )?;
    let active_profile = if active.as_str() != "/" {
        properties(connection, &owner, active.as_str(), ACTIVE)
            .ok()
            .and_then(|v| v.get("Connection").and_then(|p| object(p).ok()))
            .and_then(|path| profiles.iter().find(|p| p.path == path).map(|p| p.token))
    } else {
        None
    };
    registry.finish();
    let connectivity = root
        .get("Connectivity")
        .map(u32::try_from)
        .transpose()?
        .unwrap_or(0);
    drop(manager);
    Ok(Inventory {
        snapshot: Snapshot {
            epoch: registry.epoch,
            available: true,
            radio,
            hardware,
            state,
            state_reason: reason.1,
            connectivity,
            scan_permission: Permission::from_nm(permissions.get(SCAN)),
            control_permission: Permission::from_nm(permissions.get(CONTROL)),
            radio_permission: Permission::from_nm(permissions.get(RADIO)),
            create_permission: Permission::from_nm(permissions.get(CREATE)),
            last_scan,
            networks,
            active_profile,
            ..Default::default()
        },
        owner,
        device,
        aps,
        profiles,
        active,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owner_replacement_or_disappearing_rows_never_reuse_tokens() {
        let mut registry = Registry::default();
        registry.bind(":1.1");
        let first = registry.id("row".into()).unwrap();
        registry.finish();
        registry.bind(":1.1");
        assert_eq!(registry.id("row".into()).unwrap(), first);
        registry.finish();
        registry.bind(":1.2");
        let second = registry.id("row".into()).unwrap();
        assert!(second > first);
        registry.disconnected();
        registry.bind(":1.2");
        let third = registry.id("row".into()).unwrap();
        assert!(third > second);
        registry.bind(":1.2");
        registry.finish();
        registry.bind(":1.2");
        assert!(registry.id("row".into()).unwrap() > third);
    }
    #[test]
    fn saved_security_cannot_be_retargeted_to_open_or_different_ssid() {
        let mut profile = SavedProfile {
            token: 1,
            path: "/profile".try_into().unwrap(),
            ssid: b"Lab".to_vec(),
            key_mgmt: "wpa-psk".into(),
            label: "Saved".into(),
        };
        let mut ap = AccessPoint {
            path: "/ap".try_into().unwrap(),
            ssid: b"Lab".to_vec(),
            security: Security::Open,
            strength: 90,
            wpa: 0,
            rsn: 0,
        };
        assert!(!compatible(&ap, &profile));
        ap.security = Security::Personal;
        ap.rsn = 0x400;
        assert!(compatible(&ap, &profile));
        ap.ssid = b"Other".to_vec();
        assert!(!compatible(&ap, &profile));
        ap.ssid = b"Lab".to_vec();
        profile.key_mgmt = "sae".into();
        assert!(compatible(&ap, &profile));
        ap.rsn = 0x100;
        assert!(!compatible(&ap, &profile));
    }
}
