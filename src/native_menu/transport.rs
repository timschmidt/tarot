//! Direct Rust org.gtk.Menus/org.gtk.Actions subset, based on GLib 2.88.1's
//! gio/{gmenuexporter,gactiongroupexporter}.c. These are private interfaces.
use crate::process_identity::Process;
use anyhow::{Result, bail, ensure};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use zbus::{
    blocking::Connection,
    names::OwnedUniqueName,
    zvariant::{Fd, OwnedObjectPath, OwnedValue, Signature, Structure, Value},
};

const MAX_BODY: usize = 64 * 1024;
const MAX_TOTAL_BODY: usize = 256 * 1024;
const MAX_GROUPS: usize = 16;
const MAX_MENUS: usize = 64;
const MAX_ITEMS: usize = 256;
const MAX_DEPTH: usize = 8;
const MAX_TEXT: usize = 512;
const MAX_ACTIONS: usize = 256;
pub const METHOD_TIMEOUT: Duration = Duration::from_millis(500);
pub(super) use super::socket::Cancellation;

/// Owning surface/claim authority. Dropping or retiring it invalidates all
/// worker leases; a lease cannot extend its authority.
pub struct Authority(Arc<AtomicBool>);
#[derive(Clone)]
pub struct Lease(Vec<Arc<AtomicBool>>);
impl Lease {
    pub fn is_active(&self) -> bool {
        self.0
            .iter()
            .all(|authority| authority.load(Ordering::Acquire))
    }
    /// A client, surface and claim can each retire the same work independently.
    pub fn intersect(&self, other: &Self) -> Result<Self> {
        ensure!(
            self.0.len() + other.0.len() <= 8,
            "too many export authorities"
        );
        Ok(Self(self.0.iter().chain(&other.0).cloned().collect()))
    }
}
impl Default for Authority {
    fn default() -> Self {
        Self(Arc::new(AtomicBool::new(true)))
    }
}
impl Authority {
    pub fn lease(&self) -> Lease {
        Lease(vec![self.0.clone()])
    }
    pub fn retire(&self) {
        self.0.store(false, Ordering::Release);
    }
}
impl Drop for Authority {
    fn drop(&mut self) {
        self.retire();
    }
}

#[derive(Clone, Debug)]
pub struct Export {
    owner: OwnedUniqueName,
    menu: OwnedObjectPath,
    application: Option<OwnedObjectPath>,
    window: Option<OwnedObjectPath>,
}
impl Export {
    pub fn owner(&self) -> &str {
        self.owner.as_str()
    }
    pub fn relevant_signal(&self, message: &zbus::Message) -> bool {
        let h = message.header();
        if !h
            .sender()
            .is_some_and(|s| s.as_str() == self.owner.as_str())
            || h.member().map(|s| s.as_str()) != Some("Changed")
        {
            return false;
        }
        match (
            h.interface().map(|s| s.as_str()),
            h.path().map(|s| s.as_str()),
        ) {
            (Some("org.gtk.Menus"), Some(p)) => p == self.menu.as_str(),
            (Some("org.gtk.Actions"), Some(p)) => {
                self.application.as_ref().is_some_and(|v| v.as_str() == p)
                    || self.window.as_ref().is_some_and(|v| v.as_str() == p)
            }
            _ => false,
        }
    }
    pub fn new(
        owner: &str,
        menu: &str,
        application: Option<&str>,
        window: Option<&str>,
    ) -> Result<Self> {
        ensure!(
            owner.len() <= 255 && menu.len() <= 1024,
            "oversized export endpoint"
        );
        let path = |p: &str| -> Result<OwnedObjectPath> {
            ensure!(p.len() <= 1024, "oversized action endpoint");
            Ok(OwnedObjectPath::try_from(p.to_owned())?)
        };
        Ok(Self {
            owner: OwnedUniqueName::try_from(owner.to_owned())?,
            menu: path(menu)?,
            application: application.map(path).transpose()?,
            window: window.map(path).transpose()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Scalar {
    Bool(bool),
    String(String),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F64(f64),
}
impl Scalar {
    fn read(value: &OwnedValue) -> Result<Self> {
        Ok(match &**value {
            Value::Bool(v) => Self::Bool(*v),
            Value::Str(v) if v.len() <= MAX_TEXT => Self::String(v.to_string()),
            Value::I32(v) => Self::I32(*v),
            Value::U32(v) => Self::U32(*v),
            Value::I64(v) => Self::I64(*v),
            Value::U64(v) => Self::U64(*v),
            Value::F64(v) if v.is_finite() => Self::F64(*v),
            _ => bail!("unsupported action value"),
        })
    }
    fn signature(&self) -> &'static str {
        match self {
            Self::Bool(_) => "b",
            Self::String(_) => "s",
            Self::I32(_) => "i",
            Self::U32(_) => "u",
            Self::I64(_) => "x",
            Self::U64(_) => "t",
            Self::F64(_) => "d",
        }
    }
    fn value(&self) -> Value<'_> {
        match self {
            Self::Bool(v) => (*v).into(),
            Self::String(v) => v.as_str().into(),
            Self::I32(v) => (*v).into(),
            Self::U32(v) => (*v).into(),
            Self::I64(v) => (*v).into(),
            Self::U64(v) => (*v).into(),
            Self::F64(v) => (*v).into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Action {
    pub name: String,
    pub enabled: bool,
    pub parameter: String,
    pub state: Option<Scalar>,
    pub target: Option<Scalar>,
    pub supported: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Item {
    Action {
        label: String,
        action: Action,
    },
    Submenu {
        label: String,
        items: Vec<Item>,
    },
    Section {
        label: Option<String>,
        items: Vec<Item>,
    },
    Unavailable {
        label: String,
    },
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    #[serde(skip)]
    binding: u64,
    pub items: Vec<Item>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Dispatch {
    /// A method reply arrived. This does not confirm document-operation completion.
    Replied,
    /// Dispatch was attempted but its result is unknown. Never retry automatically.
    Uncertain,
}

type Attributes = HashMap<String, OwnedValue>;
type MenuWire = Vec<(u32, u32, Vec<Attributes>)>;
type ActionWire = HashMap<String, (bool, Signature, Vec<OwnedValue>)>;
type Address = (u32, u32);

pub fn session_connection() -> Result<Connection> {
    session_connection_cancellable(Arc::new(Cancellation::new(Arc::new(AtomicBool::new(
        false,
    )))))
}

pub(super) fn session_connection_cancellable(cancel: Arc<Cancellation>) -> Result<Connection> {
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS")?;
    super::socket::connect(&address, cancel)
}

/// Owned worker-side binding to one original bus connection, unique exporter,
/// retained Wayland peer and claim lease. No reconnection or PID fallback.
pub struct Binding {
    connection: Connection,
    export: Export,
    peer: Arc<Process>,
    exporter: Process,
    lease: Lease,
    id: u64,
    barrier: Option<Arc<dyn Fn(zbus::message::Sequence) -> Result<()> + Send + Sync>>,
    subscriptions: Mutex<BTreeSet<u32>>,
}
impl Binding {
    pub fn new(
        connection: Connection,
        export: Export,
        peer: Arc<Process>,
        lease: Lease,
    ) -> Result<Self> {
        ensure!(
            connection
                .method_timeout()
                .is_some_and(|t| t <= METHOD_TIMEOUT),
            "bounded method timeout required"
        );
        ensure!(lease.is_active(), "retired export claim");
        let exporter = credentials(&connection, &export.owner)?;
        ensure!(
            peer.same_live_process(&exporter)?,
            "export belongs to another process"
        );
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let binding = Self {
            connection,
            export,
            peer,
            exporter,
            lease,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            subscriptions: Mutex::new(BTreeSet::new()),
            barrier: None,
        };
        binding.valid()?;
        Ok(binding)
    }

    pub fn with_barrier(
        mut self,
        barrier: Arc<dyn Fn(zbus::message::Sequence) -> Result<()> + Send + Sync>,
    ) -> Self {
        self.barrier = Some(barrier);
        self
    }
    fn ordered(&self, message: &zbus::Message) -> Result<()> {
        if let Some(barrier) = &self.barrier {
            barrier(message.recv_position())?;
        }
        Ok(())
    }
    fn valid(&self) -> Result<()> {
        ensure!(self.lease.is_active(), "retired export claim");
        ensure!(
            self.peer.same_live_process(&self.exporter)?,
            "export process exited"
        );
        Ok(())
    }

    fn call<B>(&self, path: &str, interface: &str, method: &str, args: &B) -> Result<zbus::Message>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        self.valid()?;
        let reply = self.connection.call_method(
            Some(self.export.owner.as_str()),
            path,
            Some(interface),
            method,
            args,
        )?;
        self.ordered(&reply)?;
        self.valid()?;
        ensure!(
            reply
                .header()
                .sender()
                .is_some_and(|sender| sender.as_str() == self.export.owner.as_str()),
            "foreign menu reply"
        );
        ensure!(
            reply.body().len() <= MAX_BODY && reply.data().fds().is_empty(),
            "oversized or FD-bearing menu reply"
        );
        Ok(reply)
    }

    /// Read a bounded candidate. Signal/claim generation retirement
    /// remains the compositor worker's responsibility; dispatch also reloads and
    /// compares the entire displayed tree immediately before invoking a choice.
    pub fn snapshot(&self) -> Result<Snapshot> {
        self.snapshot_mode(false)
    }

    /// Keep one baseline subscription per current group until the binding closes.
    /// Install signal matches before calling this method.
    pub fn watched_snapshot(&self) -> Result<Snapshot> {
        self.snapshot_mode(true)
    }

    fn snapshot_mode(&self, retain: bool) -> Result<Snapshot> {
        let mut baseline = self.subscriptions.lock().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        self.valid()?;
        let current = credentials(&self.connection, &self.export.owner)?;
        ensure!(
            self.peer.same_live_process(&current)?,
            "export connection changed"
        );
        let mut actions = HashMap::new();
        let mut bytes = 0usize;
        for (prefix, path) in [
            ("app", &self.export.application),
            ("win", &self.export.window),
        ] {
            if let Some(path) = path {
                ensure!(Instant::now() < deadline, "menu loading deadline");
                let message = self.call(path.as_str(), "org.gtk.Actions", "DescribeAll", &())?;
                bytes += message.body().len();
                let wire: ActionWire = message.body().deserialize()?;
                ensure!(
                    wire.len() <= MAX_ACTIONS && actions.len() + wire.len() <= MAX_ACTIONS,
                    "too many actions"
                );
                for (name, (enabled, parameter, state)) in wire {
                    ensure!(
                        name.len() <= MAX_TEXT && state.len() <= 1,
                        "invalid action description"
                    );
                    let state_value = state.first().map(Scalar::read);
                    let supported = state_value.as_ref().is_none_or(|s| s.is_ok())
                        && matches!(
                            parameter.to_string().as_str(),
                            "" | "b" | "s" | "i" | "u" | "x" | "t" | "d"
                        );
                    actions.insert(
                        format!("{prefix}.{name}"),
                        Action {
                            name: format!("{prefix}.{name}"),
                            enabled,
                            parameter: parameter.to_string(),
                            state: state_value.and_then(Result::ok),
                            target: None,
                            supported,
                        },
                    );
                }
            }
        }
        let mut subscribed = BTreeSet::new();
        let result = (|| {
            let mut menus = BTreeMap::new();
            let mut pending = BTreeSet::from([0u32]);
            let mut count = 0usize;
            while let Some(group) = pending.pop_first() {
                ensure!(Instant::now() < deadline, "menu loading deadline");
                ensure!(subscribed.len() < MAX_GROUPS, "too many menu groups");
                // Record even a timed-out Start so End is still attempted.
                subscribed.insert(group);
                let message = self.call(
                    self.export.menu.as_str(),
                    "org.gtk.Menus",
                    "Start",
                    &(vec![group],),
                )?;
                bytes += message.body().len();
                ensure!(
                    bytes <= MAX_TOTAL_BODY,
                    "menu snapshot byte budget exceeded"
                );
                let wire: MenuWire = message.body().deserialize()?;
                for (g, menu, rows) in wire {
                    ensure!(
                        g == group && !menus.contains_key(&(g, menu)),
                        "invalid menu group reply"
                    );
                    count += rows.len();
                    ensure!(
                        count <= MAX_ITEMS && menus.len() < MAX_MENUS,
                        "menu snapshot item budget exceeded"
                    );
                    for attrs in &rows {
                        ensure!(
                            attrs.len() <= 16 && attrs.keys().all(|k| k.len() <= 128),
                            "too many menu attributes"
                        );
                        for key in [":submenu", ":section"] {
                            if let Some(value) = attrs.get(key) {
                                let address = address(value)?;
                                if !subscribed.contains(&address.0) {
                                    pending.insert(address.0);
                                }
                            }
                        }
                    }
                    menus.insert((g, menu), rows);
                }
            }
            let mut path = BTreeSet::new();
            let mut rendered = 0usize;
            let items = tree((0, 0), &menus, &actions, &mut path, 0, &mut rendered)?;
            self.valid()?;
            ensure!(Instant::now() < deadline, "menu loading deadline");
            Ok(Snapshot {
                binding: self.id,
                items,
            })
        })();
        // Each Start has a matching End on this same connection. End failure
        // invalidates the candidate; closing the binding releases lost remotes.
        let end_groups = if retain && result.is_ok() {
            // Existing baseline references are replaced by this load's Start;
            // each old reference (including unused groups) is ended once.
            baseline.iter().copied().collect::<Vec<_>>()
        } else {
            let mut groups = subscribed.iter().copied().collect::<Vec<_>>();
            if retain {
                groups.extend(baseline.iter().copied());
            }
            groups
        };
        let cleanup = self.connection.call_method(
            Some(self.export.owner.as_str()),
            self.export.menu.as_str(),
            Some("org.gtk.Menus"),
            "End",
            &(end_groups,),
        );
        let cleanup = cleanup.and_then(|reply| {
            if self.ordered(&reply).is_err() {
                return Err(zbus::Error::Failure("menu signal barrier failed".into()));
            }
            if reply
                .header()
                .sender()
                .is_some_and(|sender| sender.as_str() == self.export.owner.as_str())
                && reply.body().is_empty()
                && reply.data().fds().is_empty()
            {
                Ok(())
            } else {
                Err(zbus::Error::Failure("invalid menu End reply".into()))
            }
        });
        if cleanup.is_err() {
            let _ = self.connection.clone().close();
        }
        if retain {
            *baseline = if result.is_ok() && cleanup.is_ok() {
                subscribed
            } else {
                BTreeSet::new()
            };
        }
        result.and_then(|snapshot| {
            cleanup?;
            self.valid()?;
            Ok(snapshot)
        })
    }

    pub fn activate(&self, snapshot: &Snapshot, path: &[usize]) -> Result<Dispatch> {
        self.activate_inner(snapshot, path, None)
    }
    pub fn activate_guarded(
        &self,
        snapshot: &Snapshot,
        path: &[usize],
        guard: &Lease,
    ) -> Result<Dispatch> {
        self.activate_inner(snapshot, path, Some(guard))
    }
    fn activate_inner(
        &self,
        snapshot: &Snapshot,
        path: &[usize],
        guard: Option<&Lease>,
    ) -> Result<Dispatch> {
        ensure!(guard.is_none_or(Lease::is_active), "retired presented menu");
        self.valid()?;
        ensure!(
            snapshot.binding == self.id,
            "snapshot belongs to another binding"
        );
        let fresh = self.snapshot()?;
        ensure!(
            fresh.items == snapshot.items,
            "menu changed after selection"
        );
        let action = choice(&snapshot.items, path)?;
        ensure!(action.enabled && action.supported, "action unavailable");
        let (prefix, name) = action
            .name
            .split_once('.')
            .ok_or_else(|| anyhow::anyhow!("unscoped action"))?;
        let endpoint = match prefix {
            "app" => &self.export.application,
            "win" => &self.export.window,
            _ => bail!("unsupported action scope"),
        };
        let endpoint = endpoint
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing action endpoint"))?;
        let parameters: Vec<Value<'_>> = action
            .target
            .as_ref()
            .map(Scalar::value)
            .into_iter()
            .collect();
        self.valid()?;
        ensure!(guard.is_none_or(Lease::is_active), "retired presented menu");
        // Once attempted, neither retirement nor a timeout proves cancellation.
        let result = self.connection.call_method(
            Some(self.export.owner.as_str()),
            endpoint.as_str(),
            Some("org.gtk.Actions"),
            "Activate",
            &(name, parameters, HashMap::<String, Value<'_>>::new()),
        );
        Ok(
            if result.is_ok_and(|reply| {
                reply
                    .header()
                    .sender()
                    .is_some_and(|sender| sender.as_str() == self.export.owner.as_str())
                    && reply.body().is_empty()
                    && reply.data().fds().is_empty()
            }) {
                Dispatch::Replied
            } else {
                Dispatch::Uncertain
            },
        )
    }
}

fn credentials(connection: &Connection, owner: &OwnedUniqueName) -> Result<Process> {
    let message = connection.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "GetConnectionCredentials",
        &(owner.as_str(),),
    )?;
    ensure!(
        message
            .header()
            .sender()
            .is_some_and(|sender| sender.as_str() == "org.freedesktop.DBus"),
        "foreign credential reply"
    );
    ensure!(
        message.body().len() <= 4096 && message.data().fds().len() <= 1,
        "oversized process credentials"
    );
    let mut values: HashMap<String, OwnedValue> = message.body().deserialize()?;
    let value = values
        .remove("ProcessFD")
        .ok_or_else(|| anyhow::anyhow!("ProcessFD unavailable"))?;
    let fd = Fd::try_from(value)?;
    Ok(Process::from_fd(fd.try_into()?)?)
}
fn address(value: &OwnedValue) -> Result<Address> {
    let structure = <&Structure<'_>>::try_from(value)?;
    let fields = structure.fields();
    ensure!(fields.len() == 2, "invalid menu link shape");
    Ok((u32::try_from(&fields[0])?, u32::try_from(&fields[1])?))
}
fn text(attrs: &Attributes, key: &str) -> Result<Option<String>> {
    attrs
        .get(key)
        .map(|value| {
            let text = <&str>::try_from(value)?;
            ensure!(
                text.len() <= MAX_TEXT && !text.chars().any(char::is_control),
                "invalid menu text"
            );
            Ok(text.to_owned())
        })
        .transpose()
}
fn tree(
    at: Address,
    menus: &BTreeMap<Address, Vec<Attributes>>,
    actions: &HashMap<String, Action>,
    path: &mut BTreeSet<Address>,
    depth: usize,
    rendered: &mut usize,
) -> Result<Vec<Item>> {
    ensure!(depth <= MAX_DEPTH && path.insert(at), "cyclic or deep menu");
    // GLib omits empty menus from Start replies, including the root and linked
    // submenus. Every referenced group was subscribed above; absence is an
    // authoritative empty model, not a loading error or a fabricated action.
    let Some(rows) = menus.get(&at) else {
        path.remove(&at);
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for attrs in rows {
        *rendered += 1;
        ensure!(*rendered <= MAX_ITEMS, "expanded menu too large");
        let label = text(attrs, "label")?;
        let sub = attrs.get(":submenu");
        let section = attrs.get(":section");
        let name = text(attrs, "action")?;
        ensure!(
            usize::from(sub.is_some())
                + usize::from(section.is_some())
                + usize::from(name.is_some())
                <= 1,
            "ambiguous menu item"
        );
        let item = if let Some(link) = sub {
            Item::Submenu {
                label: label.ok_or_else(|| anyhow::anyhow!("submenu needs a label"))?,
                items: tree(address(link)?, menus, actions, path, depth + 1, rendered)?,
            }
        } else if let Some(link) = section {
            Item::Section {
                label,
                items: tree(address(link)?, menus, actions, path, depth + 1, rendered)?,
            }
        } else {
            let label = label.ok_or_else(|| anyhow::anyhow!("menu item needs a label"))?;
            if let Some(mut action) = name.and_then(|n| actions.get(&n).cloned()) {
                action.target = attrs
                    .get("target")
                    .map(Scalar::read)
                    .transpose()
                    .ok()
                    .flatten();
                let target_valid = match (&action.target, action.parameter.as_str()) {
                    (None, "") => !attrs.contains_key("target"),
                    (Some(value), parameter) => value.signature() == parameter,
                    _ => false,
                };
                action.supported &= target_valid;
                Item::Action { label, action }
            } else {
                Item::Unavailable { label }
            }
        };
        result.push(item);
    }
    path.remove(&at);
    Ok(result)
}
fn choice<'a>(items: &'a [Item], path: &[usize]) -> Result<&'a Action> {
    ensure!(
        !path.is_empty() && path.len() <= MAX_DEPTH + 1,
        "invalid choice path"
    );
    match items.get(path[0]) {
        Some(Item::Action { action, .. }) if path.len() == 1 => Ok(action),
        Some(Item::Submenu { items, .. } | Item::Section { items, .. }) if path.len() > 1 => {
            choice(items, &path[1..])
        }
        _ => bail!("choice is not an action"),
    }
}
