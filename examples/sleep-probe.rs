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
    });
    let connection = Builder::address(address.as_str())?
        .serve_at("/org/freedesktop/login1", Manager(shared.clone()))?
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
