//! Session-bound Polkit agent. All bus, NSS and PAM/helper work stays off UI.
mod agent;
use crate::{CalloopData, Compositor};
use anyhow::{Context, Result, bail, ensure};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    collections::HashMap,
    future::Future,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    task::Poll,
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot::polkit::helper::{Conversation, Event};
use tarot_core::authorization::{Challenge, Identity, Phase, Request, Snapshot, UiAction};
use zbus::{
    blocking::{Connection, MessageIterator, connection::Builder},
    zvariant::{OwnedObjectPath, OwnedValue},
};
use zeroize::Zeroizing;
const AUTHORITY: &str = "org.freedesktop.PolicyKit1";
const AUTHORITY_PATH: &str = "/org/freedesktop/PolicyKit1/Authority";
const AUTHORITY_IFACE: &str = "org.freedesktop.PolicyKit1.Authority";
const LOGIN: &str = "org.freedesktop.login1";
const SESSION: &str = "org.freedesktop.login1.Session";
const SHORT: Duration = Duration::from_secs(3);
const REQUEST_TIME: Duration = Duration::from_secs(180);
/// Read-only admission hint for explicit platform actions. The mechanism and
/// authority must still make their own actual authorization decision.
#[derive(Clone, Default)]
pub struct Readiness(Arc<AtomicBool>);
impl Readiness {
    pub fn can_prompt(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
type Properties = HashMap<String, OwnedValue>;
type Subject = (String, HashMap<String, OwnedValue>);
#[derive(Clone)]
pub struct Options {
    pub bus: Option<String>,
    pub helper: Option<PathBuf>,
}
#[derive(Clone, PartialEq, Eq)]
struct Enrollment {
    authority: String,
    login: String,
    session: OwnedObjectPath,
    id: String,
    active: bool,
    epoch: u64,
}
struct Current {
    request: Request,
    cookie: Zeroizing<String>,
    epoch: u64,
    cancelled: Arc<AtomicBool>,
    actions: mpsc::SyncSender<UiAction>,
    answer: async_channel::Sender<Result<(), agent::Error>>,
}
#[derive(Default)]
struct Data {
    enrollment: Option<Enrollment>,
    epoch: u64,
    next: u64,
    current: Option<Current>,
    snapshot: Snapshot,
}
struct Shared {
    data: Mutex<Data>,
    update: Mutex<Option<Snapshot>>,
    ping: Ping,
    stop: AtomicBool,
    stop_tx: async_channel::Sender<()>,
    stop_rx: async_channel::Receiver<()>,
    protected: AtomicBool,
    busy: AtomicBool,
    readiness: Readiness,
    options: Options,
}
impl Shared {
    fn publish(&self, d: &mut Data, error: Option<String>) {
        let can_prompt = d.enrollment.as_ref().is_some_and(|e| e.active)
            && !self.protected.load(Ordering::Acquire)
            && !self.stop.load(Ordering::Acquire);
        self.readiness.0.store(can_prompt, Ordering::Release);
        let mut next = Snapshot {
            enabled: true,
            available: d.enrollment.is_some(),
            can_prompt,
            epoch: d.epoch,
            generation: d.snapshot.generation,
            request: d.current.as_ref().map(|c| c.request.clone()),
            error,
        };
        if next == d.snapshot {
            return;
        }
        next.generation = next.generation.saturating_add(1);
        d.snapshot = next.clone();
        *self.update.lock().unwrap() = Some(next);
        self.ping.ping();
    }
    fn cancel(&self, id: Option<u64>) {
        self.cancel_context(id, false);
    }
    fn deactivate(&self) {
        self.cancel_context(None, true);
    }
    fn cancel_context(&self, id: Option<u64>, deactivate: bool) {
        let mut d = self.data.lock().unwrap();
        if deactivate {
            self.readiness.0.store(false, Ordering::Release);
            if let Some(e) = &mut d.enrollment {
                e.active = false;
            }
        }
        if d.current
            .as_ref()
            .is_none_or(|c| id.is_some_and(|id| id != c.request.id))
        {
            if deactivate {
                let error = d.snapshot.error.clone();
                self.publish(&mut d, error);
            }
            return;
        }
        let current = d.current.take().unwrap();
        current.cancelled.store(true, Ordering::Release);
        self.publish(&mut d, None);
        drop(d);
        let _ = current.answer.try_send(Err(agent::Error::Cancelled(
            "Authentication canceled".into(),
        )));
    }
    fn reset(&self) {
        self.deactivate();
        let mut d = self.data.lock().unwrap();
        d.enrollment = None;
        self.publish(
            &mut d,
            Some("Authorization service unavailable; reconnecting".into()),
        );
    }
    fn finish(&self, id: u64, result: Result<bool>) {
        let mut d = self.data.lock().unwrap();
        if d.current.as_ref().is_none_or(|c| c.request.id != id) {
            return;
        }
        let current = d.current.take().unwrap();
        let cancelled = current.cancelled.load(Ordering::Acquire)
            || self.protected.load(Ordering::Acquire)
            || d.enrollment
                .as_ref()
                .is_none_or(|e| !e.active || e.epoch != current.epoch);
        let reply = if cancelled {
            Err(agent::Error::Cancelled(
                "Authentication context ended".into(),
            ))
        } else {
            match &result {
                Ok(_) => Ok(()),
                Err(_) => Err(agent::Error::Failed(
                    "Authentication did not complete".into(),
                )),
            }
        };
        let error = if cancelled {
            None
        } else {
            match result {
                Ok(true) => None,
                Ok(false) => Some("Authentication failed".into()),
                Err(_) => Some("Authentication did not complete".into()),
            }
        };
        self.publish(&mut d, error);
        drop(d);
        let _ = current.answer.try_send(reply);
    }
    fn change(&self, id: u64, change: impl FnOnce(&mut Request)) -> bool {
        let mut d = self.data.lock().unwrap();
        let Some(c) = d
            .current
            .as_mut()
            .filter(|c| c.request.id == id && !c.cancelled.load(Ordering::Acquire))
        else {
            return false;
        };
        change(&mut c.request);
        self.publish(&mut d, None);
        true
    }
}
pub struct Server {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn readiness(&self) -> Readiness {
        self.shared.readiness.clone()
    }
    pub fn protection(&self, protected: bool) {
        if self.shared.protected.swap(protected, Ordering::AcqRel) == protected {
            return;
        }
        if protected {
            self.shared.readiness.0.store(false, Ordering::Release);
            self.shared.cancel(None);
        }
        let mut d = self.shared.data.lock().unwrap();
        let error = d.snapshot.error.clone();
        self.shared.publish(&mut d, error);
    }
    pub fn action(&self, action: UiAction) -> Result<()> {
        if let UiAction::Cancel { request } = action {
            self.shared.cancel(Some(request));
            return Ok(());
        }
        ensure!(
            !self.shared.protected.load(Ordering::Acquire),
            "Session is locked"
        );
        let d = self.shared.data.lock().unwrap();
        let c = d.current.as_ref().context("Authentication request ended")?;
        let valid = match &action {
            UiAction::Select { request, uid } => {
                *request == c.request.id
                    && c.request.phase == Phase::Choose
                    && c.request.identities.iter().any(|i| i.uid == *uid)
            }
            UiAction::Respond { request, turn, .. } => {
                *request == c.request.id
                    && matches!(&c.request.phase,Phase::Question(q) if q.id==*turn)
            }
            UiAction::Cancel { .. } => false,
        };
        ensure!(valid, "Authentication question ended");
        c.actions
            .try_send(action)
            .map_err(|_| anyhow::anyhow!("Authentication input is busy"))
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.readiness.0.store(false, Ordering::Release);
        let _ = self.shared.stop_tx.try_send(());
        self.shared.cancel(None);
        if let Some(w) = self.worker.take().filter(|w| w.is_finished()) {
            let _ = w.join();
        }
    }
}
pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
    options: Options,
) -> Result<Server> {
    ensure!(
        options.helper.is_none() || options.bus.is_some(),
        "Authentication fixtures require an explicit private bus"
    );
    let (ping, source) = make_ping()?;
    let (stop_tx, stop_rx) = async_channel::bounded(1);
    let shared = Arc::new(Shared {
        data: Mutex::new(Data::default()),
        update: Mutex::new(None),
        ping,
        stop: AtomicBool::new(false),
        stop_tx,
        stop_rx,
        protected: AtomicBool::new(state.protected()),
        busy: AtomicBool::new(false),
        readiness: Readiness::default(),
        options,
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |_, _, data| {
            // Release this lock before shell effects can cancel a backend request.
            let snapshot = { updates.update.lock().unwrap().take() };
            if let Some(snapshot) = snapshot {
                data.state.sync_authorization(snapshot);
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state.shell.authorization.snapshot.enabled = true;
    let background = shared.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-authorization".into())
        .spawn(move || supervise(background))?;
    Ok(Server {
        shared,
        worker: Some(worker),
    })
}
async fn within<F, T, E>(future: F) -> Result<T>
where
    F: Future<Output = std::result::Result<T, E>>,
    E: Into<anyhow::Error>,
{
    let mut future = std::pin::pin!(future);
    let mut timer = std::pin::pin!(async_io::Timer::after(SHORT));
    std::future::poll_fn(|cx| {
        if let Poll::Ready(r) = future.as_mut().poll(cx) {
            return Poll::Ready(r.map_err(Into::into));
        }
        if timer.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Err(anyhow::anyhow!("Authorization request timed out")));
        }
        Poll::Pending
    })
    .await
}
fn rpc<B, R>(
    c: &Connection,
    owner: &str,
    path: &str,
    interface: &str,
    method: &str,
    body: &B,
) -> Result<R>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: for<'d> zbus::zvariant::DynamicDeserialize<'d>,
{
    let message = async_io::block_on(within(c.inner().call_method(
        Some(owner),
        path,
        Some(interface),
        method,
        body,
    )))?;
    Ok(message.body().deserialize()?)
}
fn prop<T: TryFrom<OwnedValue>>(p: &Properties, key: &str) -> Option<T> {
    p.get(key)?.try_clone().ok()?.try_into().ok()
}
fn owner(c: &Connection, name: &str, test: bool) -> Result<String> {
    let resolve = || {
        rpc::<_, String>(
            c,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetNameOwner",
            &(name,),
        )
    };
    let owner = match resolve() {
        Ok(owner) => owner,
        Err(error) if test => return Err(error),
        Err(_) => {
            let _: u32 = rpc(
                c,
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "StartServiceByName",
                &(name, 0u32),
            )?;
            resolve()?
        }
    };
    let uid: u32 = rpc(
        c,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "GetConnectionUnixUser",
        &(owner.as_str(),),
    )?;
    let trusted = if test {
        uid == unsafe { libc::geteuid() }
    } else {
        trusted_service(name, uid, unsafe { libc::geteuid() }, polkit_uid())
    };
    ensure!(trusted, "Untrusted authorization service");
    Ok(owner)
}
fn trusted_service(name: &str, uid: u32, gui_uid: u32, polkit_uid: Option<u32>) -> bool {
    uid == 0 || name == AUTHORITY && uid != gui_uid && polkit_uid == Some(uid)
}
fn polkit_uid() -> Option<u32> {
    let mut storage = vec![0u8; 16384];
    let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
    let mut found = std::ptr::null_mut();
    // SAFETY: fixed terminated account name and correctly sized live storage.
    let code = unsafe {
        libc::getpwnam_r(
            c"polkitd".as_ptr(),
            entry.as_mut_ptr(),
            storage.as_mut_ptr().cast(),
            storage.len(),
            &mut found,
        )
    };
    if code != 0 || found.is_null() {
        return None;
    }
    // SAFETY: successful lookup initialized the entry in owned storage.
    Some(unsafe { entry.assume_init() }.pw_uid)
}
fn enrollment(c: &Connection, test: bool) -> Result<Enrollment> {
    let authority = owner(c, AUTHORITY, test)?;
    let login = owner(c, LOGIN, test)?;
    let session: OwnedObjectPath = rpc(
        c,
        &login,
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
        "GetSessionByPID",
        &(std::process::id(),),
    )?;
    let p: Properties = rpc(
        c,
        &login,
        session.as_str(),
        "org.freedesktop.DBus.Properties",
        "GetAll",
        &(SESSION,),
    )?;
    let uid: (u32, OwnedObjectPath) = prop(&p, "User").context("Session user unavailable")?;
    let id: String = prop(&p, "Id").context("Session identity unavailable")?;
    ensure!(
        !id.is_empty()
            && id.len() <= 128
            && !id.chars().any(char::is_control)
            && uid.0 == unsafe { libc::geteuid() }
            && prop::<bool>(&p, "Remote") == Some(false)
            && prop::<String>(&p, "Type").as_deref() == Some("wayland"),
        "Not a local graphical session"
    );
    Ok(Enrollment {
        authority,
        login,
        session,
        id,
        active: prop::<bool>(&p, "Active") == Some(true)
            && prop::<bool>(&p, "LockedHint") != Some(true),
        epoch: 0,
    })
}
fn subject(e: &Enrollment) -> Subject {
    let mut p = HashMap::new();
    p.insert(
        "session-id".into(),
        OwnedValue::from(zbus::zvariant::Str::from(e.id.as_str())),
    );
    ("unix-session".into(), p)
}
fn supervise(s: Arc<Shared>) {
    let mut failures = 0u32;
    while !s.stop.load(Ordering::Acquire) {
        let started = Instant::now();
        let _ = run(&s);
        s.reset();
        if started.elapsed() > Duration::from_secs(30) {
            failures = 0;
        }
        let delay = Duration::from_millis(250 << failures.min(4));
        failures = failures.saturating_add(1);
        let end = Instant::now() + delay;
        while !s.stop.load(Ordering::Acquire) && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
fn run(s: &Arc<Shared>) -> Result<()> {
    let b = if let Some(bus) = &s.options.bus {
        Builder::address(bus.as_str())?
    } else {
        Builder::system()?
    };
    let c = b
        .method_timeout(SHORT)
        .serve_at(agent::PATH, agent::Agent(s.clone()))?
        .build()?;
    let result = (|| -> Result<()> {
        let mut e = enrollment(&c, s.options.bus.is_some())?;
        let mut messages = MessageIterator::from(&c).into_inner();
        for rule in [
            format!(
                "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='{AUTHORITY}'"
            ),
            format!(
                "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='{LOGIN}'"
            ),
            format!(
                "type='signal',sender='{}',path='{}',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged'",
                e.login, e.session
            ),
        ] {
            let _: () = rpc(
                &c,
                "org.freedesktop.DBus",
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "AddMatch",
                &(rule,),
            )?;
        }
        let _: () = rpc(
            &c,
            &e.authority,
            AUTHORITY_PATH,
            AUTHORITY_IFACE,
            "RegisterAuthenticationAgent",
            &(subject(&e), "C.UTF-8", agent::PATH),
        )?;
        // Enrollment cannot survive an owner/session change during registration.
        let checked = enrollment(&c, s.options.bus.is_some())?;
        ensure!(checked == e, "Authorization session changed");
        {
            let mut d = s.data.lock().unwrap();
            d.epoch = d
                .epoch
                .checked_add(1)
                .context("Authorization epoch limit")?;
            e.epoch = d.epoch;
            d.enrollment = Some(e.clone());
            s.publish(&mut d, None);
        }
        // The supervisor is the only owner of this reader; no retired watcher
        // can run after a new enrollment starts. Wait with a bounded stop check.
        loop {
            if s.stop.load(Ordering::Acquire) {
                break;
            }
            let next = async_io::block_on(async {
                let stream = &mut messages;
                // Stop wakes this event-driven reader without an idle polling timer.
                let mut next =
                    std::pin::pin!(std::future::poll_fn(|cx| futures_poll_next(stream, cx)));
                let mut stopping = std::pin::pin!(s.stop_rx.recv());
                std::future::poll_fn(|cx| {
                    if let Poll::Ready(v) = next.as_mut().poll(cx) {
                        return Poll::Ready(Some(v));
                    }
                    if stopping.as_mut().poll(cx).is_ready() {
                        return Poll::Ready(None);
                    }
                    Poll::Pending
                })
                .await
            });
            let Some(message) = next else {
                break;
            };
            let Some(message) = message else {
                bail!("Authorization bus closed");
            };
            let message = message?;
            let h = message.header();
            if h.message_type() != zbus::message::Type::Signal {
                continue;
            }
            if h.sender()
                .is_some_and(|n| n.as_str() == "org.freedesktop.DBus")
                && h.member().is_some_and(|m| m.as_str() == "NameOwnerChanged")
                && message
                    .body()
                    .deserialize::<(String, String, String)>()
                    .is_ok_and(|(name, old, new)| {
                        (name == AUTHORITY || name == LOGIN) && old != new
                    })
            {
                bail!("Authorization service changed");
            }
            if h.sender().is_some_and(|n| n.as_str() == e.login)
                && h.path().is_some_and(|p| p.as_str() == e.session.as_str())
            {
                let (interface, changed, invalidated): (String, Properties, Vec<String>) =
                    message.body().deserialize()?;
                if interface != SESSION {
                    continue;
                }
                if prop::<bool>(&changed, "Active") == Some(false)
                    || prop::<bool>(&changed, "LockedHint") == Some(true)
                    || ["Id", "User", "Type", "Remote"]
                        .iter()
                        .any(|key| changed.contains_key(*key))
                    || invalidated.iter().any(|key| {
                        ["Id", "User", "Type", "Remote", "Active", "LockedHint"]
                            .contains(&key.as_str())
                    })
                {
                    // Revoke the admission hint before the fresh property RPC;
                    // a rapid inactive/active transition must still retire the
                    // old conversation and block new actions during the check.
                    s.deactivate();
                }
                let mut checked = enrollment(&c, s.options.bus.is_some())?;
                checked.epoch = e.epoch;
                ensure!(
                    checked.authority == e.authority
                        && checked.login == e.login
                        && checked.session == e.session
                        && checked.id == e.id,
                    "Authorization session replaced"
                );
                e = checked;
                if !e.active {
                    s.cancel(None);
                }
                let mut d = s.data.lock().unwrap();
                d.enrollment = Some(e.clone());
                s.publish(&mut d, None);
            }
        }
        let _: Result<()> = rpc(
            &c,
            &e.authority,
            AUTHORITY_PATH,
            AUTHORITY_IFACE,
            "UnregisterAuthenticationAgent",
            &(subject(&e), agent::PATH),
        );
        Ok(())
    })();
    s.reset();
    let _ = c.close();
    result
}
// zbus reexports no StreamExt trait; poll its MessageStream directly.
fn futures_poll_next(
    stream: &mut zbus::MessageStream,
    cx: &mut std::task::Context<'_>,
) -> Poll<Option<zbus::Result<zbus::Message>>> {
    use futures_core::Stream;
    std::pin::Pin::new(stream).poll_next(cx)
}
fn authenticate(
    s: &Arc<Shared>,
    id: u64,
    cookie: Zeroizing<String>,
    uids: Vec<u32>,
    cancelled: &AtomicBool,
    actions: mpsc::Receiver<UiAction>,
) -> Result<bool> {
    let end = Instant::now() + REQUEST_TIME;
    let names = uids
        .iter()
        .map(|uid| {
            Ok(Identity {
                uid: *uid,
                name: tarot::polkit::helper::account_name(*uid)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    ensure!(!cancelled.load(Ordering::Acquire), "Authentication ended");
    s.change(id, |r| r.identities = names);
    let mut selected = if uids.len() == 1 { Some(uids[0]) } else { None };
    if selected.is_none() {
        s.change(id, |r| r.phase = Phase::Choose);
    }
    while selected.is_none() {
        ensure!(
            !cancelled.load(Ordering::Acquire) && Instant::now() < end,
            "Authentication ended"
        );
        if let Ok(UiAction::Select { request, uid }) =
            actions.recv_timeout(Duration::from_millis(50))
            && request == id
            && uids.contains(&uid)
        {
            selected = Some(uid);
        }
    }
    let uid = selected.unwrap();
    if !s.change(id, |r| {
        r.selected = Some(uid);
        r.phase = Phase::Starting;
    }) {
        bail!("Authentication ended");
    }
    ensure!(!cancelled.load(Ordering::Acquire), "Authentication ended");
    let mut helper = if let Some(path) = &s.options.helper {
        Conversation::private_fixture(
            path,
            uid,
            cookie,
            s.options
                .bus
                .as_deref()
                .context("Private bus unavailable")?,
        )?
    } else {
        Conversation::system(uid, cookie)?
    };
    loop {
        ensure!(
            !cancelled.load(Ordering::Acquire)
                && !s.protected.load(Ordering::Acquire)
                && Instant::now() < end,
            "Authentication ended"
        );
        for event in helper.advance()? {
            match event {
                Event::Request {
                    id: turn,
                    prompt,
                    echo,
                } => {
                    s.change(id, |r| {
                        r.phase = Phase::Question(Challenge {
                            id: turn,
                            prompt,
                            echo,
                        })
                    });
                }
                Event::Info(info) | Event::Error(info) => {
                    s.change(id, |r| {
                        r.info = Some(
                            info.chars()
                                .filter(|c| !c.is_control() || *c == '\n')
                                .take(1024)
                                .collect(),
                        )
                    });
                }
                Event::Completed { authorized } => return Ok(authorized),
            }
        }
        if let Ok(UiAction::Respond {
            request,
            turn,
            response,
        }) = actions.recv_timeout(Duration::from_millis(25))
            && request == id
            && helper.respond(turn, response.expose()).is_ok()
        {
            s.change(id, |r| r.phase = Phase::Checking);
        }
    }
}
impl Compositor {
    fn sync_authorization(&mut self, snapshot: Snapshot) {
        let route = (
            self.shell.system_page,
            self.shell.system_menu_open,
            self.shell.menus.revision,
        );
        let old = self.shell.authorization.snapshot.request.clone();
        let effects = self.shell.sync_authorization(snapshot, !self.protected());
        if old != self.shell.authorization.snapshot.request
            || route
                != (
                    self.shell.system_page,
                    self.shell.system_menu_open,
                    self.shell.menus.revision,
                )
        {
            self.shell.menus.invalidate();
            self.cancel_input_sequences();
        }
        self.apply_effects(effects);
        if let Some(server) = &self.system_services {
            server.watch(
                self.shell.system_menu_open
                    && self.shell.system_page != tarot_core::SystemPage::Activity,
            );
        }
        if let Some(server) = &self.network {
            server.watch(
                self.shell.system_menu_open
                    && self.shell.system_page == tarot_core::SystemPage::Wifi,
            );
        }
        self.refresh_keyboard();
        self.ui_dirty = true;
        self.redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prompt_readiness_tracks_active_enrollment_protection_and_retirement() {
        let (ping, _source) = make_ping().unwrap();
        let (stop_tx, stop_rx) = async_channel::bounded(1);
        let shared = Arc::new(Shared {
            data: Mutex::new(Data::default()),
            update: Mutex::new(None),
            ping,
            stop: AtomicBool::new(false),
            stop_tx,
            stop_rx,
            protected: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            readiness: Readiness::default(),
            options: Options {
                bus: None,
                helper: None,
            },
        });
        let server = Server {
            shared: shared.clone(),
            worker: None,
        };
        let readiness = server.readiness();
        {
            let mut data = shared.data.lock().unwrap();
            data.enrollment = Some(Enrollment {
                authority: ":1.2".into(),
                login: ":1.3".into(),
                session: OwnedObjectPath::try_from("/session/test").unwrap(),
                id: "test".into(),
                active: false,
                epoch: 1,
            });
            shared.publish(&mut data, None);
            assert!(data.snapshot.available);
            assert!(!data.snapshot.can_prompt);
            assert!(!readiness.can_prompt());
            data.enrollment.as_mut().unwrap().active = true;
            shared.publish(&mut data, None);
            assert!(data.snapshot.can_prompt);
        }
        assert!(readiness.can_prompt());
        server.protection(true);
        assert!(!readiness.can_prompt());
        server.protection(false);
        assert!(readiness.can_prompt());
        shared.deactivate();
        assert!(!readiness.can_prompt());
        // Unlocking cannot reactivate an enrollment retired by login1.
        server.protection(true);
        server.protection(false);
        assert!(!readiness.can_prompt());
        shared.reset();
        assert!(!shared.data.lock().unwrap().snapshot.available);
        drop(server);
        assert!(!readiness.can_prompt());
    }
    #[test]
    fn authority_accepts_the_dedicated_daemon_account_but_login_requires_root() {
        assert!(trusted_service(AUTHORITY, 102, 10000, Some(102)));
        assert!(trusted_service(AUTHORITY, 0, 10000, Some(102)));
        assert!(trusted_service(LOGIN, 0, 10000, Some(102)));
        assert!(!trusted_service(LOGIN, 102, 10000, Some(102)));
        assert!(!trusted_service(AUTHORITY, 10000, 10000, Some(10000)));
        assert!(!trusted_service(AUTHORITY, 103, 10000, Some(102)));
        assert!(!trusted_service(AUTHORITY, 102, 10000, None));
    }
}
