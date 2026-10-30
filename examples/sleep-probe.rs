//! Private-bus login1 fixture. Never connects to the system bus or suspends.
use anyhow::{Result, ensure};
use clap::Parser;
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    os::unix::{fs::OpenOptionsExt, net::UnixStream},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use zbus::{blocking::connection::Builder, zvariant::OwnedFd};

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop", impl_display = true)]
enum InhibitError {
    #[zbus(name = "DBus.Error.AccessDenied")]
    AccessDenied(String),
    #[zbus(name = "DBus.Error.InvalidArgs")]
    InvalidArgs(String),
    #[zbus(name = "DBus.Error.Failed")]
    Failed(String),
    #[zbus(name = "login1.OperationInProgress")]
    OperationInProgress(String),
}

#[derive(Parser)]
struct Options {
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    commands: PathBuf,
}
struct Shared {
    report: Mutex<File>,
    preparing: AtomicBool,
    deny: AtomicBool,
    serial: AtomicU64,
    resume_busy_ms: AtomicU64,
    block_delay_ms: AtomicU64,
    deny_block: AtomicBool,
    busy_until: Mutex<Option<Instant>>,
    session: Mutex<SessionInfo>,
    idle_action: Mutex<String>,
    idle_delay_us: AtomicU64,
    hint_delay_ms: AtomicU64,
    deny_hint: AtomicBool,
}

struct SessionInfo {
    uid: u32,
    seat: String,
    class: String,
    kind: String,
    remote: bool,
    active: bool,
    vt: u32,
    idle: bool,
}
const SESSION_PATH: &str = "/org/freedesktop/login1/session/tarot_test";
const SESSION_IFACE: &str = "org.freedesktop.login1.Session";

struct Session(Arc<Shared>);
#[zbus::interface(name = "org.freedesktop.login1.Session")]
impl Session {
    async fn set_idle_hint(
        &self,
        idle: bool,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> Result<(), InhibitError> {
        self.0.record(json!({"event":"hint-request","idle":idle}));
        if idle {
            async_io::Timer::after(Duration::from_millis(
                self.0.hint_delay_ms.load(Ordering::Acquire),
            ))
            .await;
            if self.0.deny_hint.load(Ordering::Acquire) {
                return Err(InhibitError::AccessDenied("Fixture idle rejection".into()));
            }
        }
        let changed = {
            let mut state = self.0.session.lock().unwrap();
            let changed = state.idle != idle;
            state.idle = idle;
            changed
        };
        self.0.record(json!({"event":"hint-applied","idle":idle}));
        if changed {
            let values =
                std::collections::HashMap::from([("IdleHint", zbus::zvariant::Value::from(idle))]);
            connection
                .emit_signal(
                    None::<&str>,
                    SESSION_PATH,
                    "org.freedesktop.DBus.Properties",
                    "PropertiesChanged",
                    &(SESSION_IFACE, values, Vec::<&str>::new()),
                )
                .await
                .map_err(|e| InhibitError::Failed(e.to_string()))?;
        }
        Ok(())
    }
    #[zbus(property)]
    fn user(&self) -> (u32, zbus::zvariant::OwnedObjectPath) {
        (
            self.0.session.lock().unwrap().uid,
            "/org/freedesktop/login1/user/test".try_into().unwrap(),
        )
    }
    #[zbus(property)]
    fn seat(&self) -> (String, zbus::zvariant::OwnedObjectPath) {
        (
            self.0.session.lock().unwrap().seat.clone(),
            "/org/freedesktop/login1/seat/seat0".try_into().unwrap(),
        )
    }
    #[zbus(property)]
    fn class(&self) -> String {
        self.0.session.lock().unwrap().class.clone()
    }
    #[zbus(property)]
    fn r#type(&self) -> String {
        self.0.session.lock().unwrap().kind.clone()
    }
    #[zbus(property)]
    fn remote(&self) -> bool {
        self.0.session.lock().unwrap().remote
    }
    #[zbus(property)]
    fn active(&self) -> bool {
        self.0.session.lock().unwrap().active
    }
    #[zbus(property, name = "VTNr")]
    fn vt(&self) -> u32 {
        self.0.session.lock().unwrap().vt
    }
    #[zbus(property)]
    fn idle_hint(&self) -> bool {
        self.0.session.lock().unwrap().idle
    }
}
impl Shared {
    fn record(&self, value: Value) {
        let mut file = self.report.lock().unwrap();
        writeln!(file, "{value}").unwrap();
        file.flush().unwrap();
    }
}
struct Manager(Arc<Shared>);
#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl Manager {
    #[zbus(name = "GetSessionByPID")]
    fn get_session_by_pid(&self, pid: u32) -> zbus::zvariant::OwnedObjectPath {
        self.0.record(json!({"event":"session-request","pid":pid}));
        SESSION_PATH.try_into().unwrap()
    }
    #[zbus(property)]
    fn idle_action(&self) -> String {
        self.0.idle_action.lock().unwrap().clone()
    }
    #[zbus(property, name = "IdleActionUSec")]
    fn idle_action_usec(&self) -> u64 {
        self.0.idle_delay_us.load(Ordering::Acquire)
    }
    async fn inhibit(
        &self,
        what: &str,
        who: &str,
        why: &str,
        mode: &str,
    ) -> Result<OwnedFd, InhibitError> {
        if self.0.deny.load(Ordering::Acquire) {
            return Err(InhibitError::AccessDenied("Fixture rejection".into()));
        }
        if what != "sleep" || who != "Tarot" || why.is_empty() || !matches!(mode, "delay" | "block")
        {
            return Err(InhibitError::InvalidArgs(
                "Unexpected inhibit request".into(),
            ));
        }
        if mode == "block" {
            if why != "Audio playback or recording is active"
                || self.0.deny_block.load(Ordering::Acquire)
            {
                return Err(InhibitError::AccessDenied("Fixture block rejection".into()));
            }
            self.0.record(json!({"event":"block-request"}));
            async_io::Timer::after(Duration::from_millis(
                self.0.block_delay_ms.load(Ordering::Acquire),
            ))
            .await;
        }
        if self.0.preparing.load(Ordering::Acquire)
            || self
                .0
                .busy_until
                .lock()
                .unwrap()
                .is_some_and(|until| Instant::now() < until)
        {
            self.0.record(json!({"event":"inhibit-busy"}));
            return Err(InhibitError::OperationInProgress(
                "Sleep action still running".into(),
            ));
        }
        let (mut read, write) =
            UnixStream::pair().map_err(|e| InhibitError::Failed(e.to_string()))?;
        let serial = self.0.serial.fetch_add(1, Ordering::AcqRel) + 1;
        self.0
            .record(json!({"event":"inhibit","id":serial,"what":what,"mode":mode}));
        let shared = self.0.clone();
        std::thread::spawn(move || {
            let mut data = [0u8; 1];
            let result = read.read(&mut data);
            shared.record(json!({"event":"closed","id":serial,"eof":matches!(result,Ok(0))}));
        });
        Ok(std::os::fd::OwnedFd::from(write).into())
    }
    #[zbus(property)]
    fn preparing_for_sleep(&self) -> bool {
        self.0.preparing.load(Ordering::Acquire)
    }
}
fn main() -> Result<()> {
    let options = Options::parse();
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS")?;
    ensure!(
        address.starts_with("unix:path="),
        "Expected explicitly private Unix bus"
    );
    let shared = Arc::new(Shared {
        report: Mutex::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(options.report)?,
        ),
        preparing: AtomicBool::new(false),
        deny: AtomicBool::new(false),
        serial: AtomicU64::new(0),
        resume_busy_ms: AtomicU64::new(0),
        block_delay_ms: AtomicU64::new(0),
        deny_block: AtomicBool::new(false),
        busy_until: Mutex::new(None),
        session: Mutex::new(SessionInfo {
            uid: rustix::process::getuid().as_raw(),
            seat: "seat0".into(),
            class: "user".into(),
            kind: "wayland".into(),
            remote: false,
            active: true,
            vt: 7,
            idle: false,
        }),
        idle_action: Mutex::new("ignore".into()),
        idle_delay_us: AtomicU64::new(30_000_000),
        hint_delay_ms: AtomicU64::new(0),
        deny_hint: AtomicBool::new(false),
    });
    let connection = Builder::address(address.as_str())?
        .serve_at("/org/freedesktop/login1", Manager(shared.clone()))?
        .serve_at(SESSION_PATH, Session(shared.clone()))?
        .name("org.freedesktop.login1")?
        .build()?;
    shared.record(json!({"event":"ready"}));
    let mut reader = BufReader::new(File::open(options.commands)?);
    let mut line = String::new();
    loop {
        if reader.read_line(&mut line)? == 0 {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let value: Value = serde_json::from_str(&line)?;
        line.clear();
        match value["command"].as_str().unwrap_or("") {
            "quit" => break,
            "prepare" | "spoof" => {
                let start = value["start"].as_bool().unwrap();
                if value["command"] == "prepare" {
                    let was_preparing = shared.preparing.swap(start, Ordering::AcqRel);
                    if was_preparing && !start {
                        *shared.busy_until.lock().unwrap() = Some(
                            Instant::now()
                                + Duration::from_millis(
                                    shared.resume_busy_ms.load(Ordering::Acquire),
                                ),
                        );
                    }
                    connection.emit_signal(
                        None::<&str>,
                        "/org/freedesktop/login1",
                        "org.freedesktop.login1.Manager",
                        "PrepareForSleep",
                        &(start,),
                    )?;
                } else {
                    let foreign = Builder::address(address.as_str())?.build()?;
                    foreign.emit_signal(
                        None::<&str>,
                        "/org/freedesktop/login1",
                        "org.freedesktop.login1.Manager",
                        "PrepareForSleep",
                        &(start,),
                    )?;
                    foreign.close()?;
                }
            }
            "deny" => shared
                .deny
                .store(value["deny"].as_bool().unwrap(), Ordering::Release),
            "resume-busy" => {
                let millis = value["milliseconds"].as_u64().unwrap();
                ensure!(millis <= 10_000, "Bounded fixture busy period required");
                shared.resume_busy_ms.store(millis, Ordering::Release);
            }
            "block-delay" => {
                let millis = value["milliseconds"].as_u64().unwrap();
                ensure!(millis <= 2000, "Bounded block delay required");
                shared.block_delay_ms.store(millis, Ordering::Release);
            }
            "deny-block" => shared
                .deny_block
                .store(value["deny"].as_bool().unwrap(), Ordering::Release),
            "hint-delay" => {
                let millis = value["milliseconds"].as_u64().unwrap();
                ensure!(millis <= 2000, "Bounded hint delay required");
                shared.hint_delay_ms.store(millis, Ordering::Release);
            }
            "deny-hint" => shared
                .deny_hint
                .store(value["deny"].as_bool().unwrap(), Ordering::Release),
            "session" => {
                let mut session = shared.session.lock().unwrap();
                if let Some(v) = value["uid"].as_u64() {
                    session.uid = v.try_into()?;
                }
                if let Some(v) = value["active"].as_bool() {
                    session.active = v;
                }
                if let Some(v) = value["remote"].as_bool() {
                    session.remote = v;
                }
                if let Some(v) = value["idle"].as_bool() {
                    session.idle = v;
                }
                if let Some(v) = value["class"].as_str() {
                    session.class = v.to_owned();
                }
                if let Some(v) = value["kind"].as_str() {
                    session.kind = v.to_owned();
                }
                if let Some(v) = value["seat"].as_str() {
                    session.seat = v.to_owned();
                }
                if let Some(v) = value["vt"].as_u64() {
                    session.vt = v.try_into()?;
                }
                drop(session);
                // Invalidate all observed fields; the worker must fetch actual
                // values rather than trusting arbitrary signal payloads.
                let fields = vec![
                    "User", "Seat", "Class", "Type", "Active", "Remote", "VTNr", "IdleHint",
                ];
                connection.emit_signal(
                    None::<&str>,
                    SESSION_PATH,
                    "org.freedesktop.DBus.Properties",
                    "PropertiesChanged",
                    &(
                        SESSION_IFACE,
                        std::collections::HashMap::<&str, zbus::zvariant::Value<'_>>::new(),
                        fields,
                    ),
                )?;
            }
            "idle-action" => {
                let action = value["action"].as_str().unwrap();
                ensure!(action.len() < 64, "Bounded fixture action");
                *shared.idle_action.lock().unwrap() = action.into();
                if let Some(delay) = value["delay_us"].as_u64() {
                    shared.idle_delay_us.store(delay, Ordering::Release);
                }
                connection.emit_signal(
                    None::<&str>,
                    "/org/freedesktop/login1",
                    "org.freedesktop.DBus.Properties",
                    "PropertiesChanged",
                    &(
                        "org.freedesktop.login1.Manager",
                        std::collections::HashMap::<&str, zbus::zvariant::Value<'_>>::new(),
                        vec!["IdleAction", "IdleActionUSec"],
                    ),
                )?;
            }
            "release" => {
                connection.release_name("org.freedesktop.login1")?;
            }
            "acquire" => {
                connection.request_name("org.freedesktop.login1")?;
            }
            _ => anyhow::bail!("Unknown fixture command"),
        }
        shared.record(json!({"event":"reply","request":value["request"],"ok":true}));
    }
    connection.close()?;
    Ok(())
}
