//! Isolated fake system services for testing actual D-Bus clients. Never use on the system bus.
use anyhow::Result;
use clap::Parser;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use zbus::{
    blocking::connection::Builder,
    fdo,
    zvariant::{OwnedObjectPath, OwnedValue},
};

#[derive(Parser)]
struct Options {
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    commands: PathBuf,
    #[arg(long)]
    backlight_root: PathBuf,
}
#[derive(Clone)]
struct Data {
    percent: f64,
    state: u32,
    level: u32,
    present: bool,
    on_battery: bool,
    active: bool,
    remote: bool,
    deny: bool,
    delay: u64,
    confirm: bool,
}
struct Shared {
    data: Mutex<Data>,
    report: Mutex<File>,
    root: PathBuf,
}
impl Shared {
    fn record(&self, value: Value) {
        let mut file = self.report.lock().unwrap();
        writeln!(file, "{value}").unwrap();
        file.flush().unwrap();
    }
}
struct Device(Arc<Shared>);
#[zbus::interface(name = "org.freedesktop.UPower.Device")]
impl Device {
    #[zbus(property)]
    fn percentage(&self) -> f64 {
        self.0.data.lock().unwrap().percent
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        self.0.data.lock().unwrap().state
    }
    #[zbus(property)]
    fn battery_level(&self) -> u32 {
        self.0.data.lock().unwrap().level
    }
    #[zbus(property)]
    fn is_present(&self) -> bool {
        self.0.data.lock().unwrap().present
    }
}
struct Root(Arc<Shared>);
#[zbus::interface(name = "org.freedesktop.UPower")]
impl Root {
    #[zbus(property)]
    fn on_battery(&self) -> bool {
        self.0.data.lock().unwrap().on_battery
    }
}
struct Session(Arc<Shared>);
#[zbus::interface(name = "org.freedesktop.login1.Session")]
impl Session {
    #[zbus(property)]
    fn active(&self) -> bool {
        self.0.data.lock().unwrap().active
    }
    #[zbus(property)]
    fn remote(&self) -> bool {
        self.0.data.lock().unwrap().remote
    }
    #[zbus(property)]
    fn seat(&self) -> (String, OwnedObjectPath) {
        (
            "seat0".into(),
            OwnedObjectPath::try_from("/org/freedesktop/login1/seat/seat0").unwrap(),
        )
    }
    fn set_brightness(&self, subsystem: &str, name: &str, brightness: u32) -> fdo::Result<()> {
        let data = self.0.data.lock().unwrap().clone();
        self.0
            .record(json!({"event":"brightness-call","value":brightness,"denied":data.deny}));
        if data.deny {
            return Err(fdo::Error::AccessDenied("Fixture denied request".into()));
        }
        if subsystem != "backlight" || name != "panel" || brightness > 51 {
            return Err(fdo::Error::InvalidArgs(
                "Invalid fixture device/range".into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(data.delay));
        std::fs::write(self.0.root.join("panel/brightness"), brightness.to_string())
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        if data.confirm {
            std::fs::write(
                self.0.root.join("panel/actual_brightness"),
                brightness.to_string(),
            )
            .map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        Ok(())
    }
}
fn main() -> Result<()> {
    let options = Options::parse();
    let shared = Arc::new(Shared {
        data: Mutex::new(Data {
            percent: 44.0,
            state: 2,
            level: 1,
            present: true,
            on_battery: false,
            active: true,
            remote: false,
            deny: false,
            delay: 0,
            confirm: true,
        }),
        report: Mutex::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(options.report)?,
        ),
        root: options.backlight_root,
    });
    let connection = Builder::session()?
        .serve_at("/org/freedesktop/UPower", Root(shared.clone()))?
        .serve_at(
            "/org/freedesktop/UPower/devices/DisplayDevice",
            Device(shared.clone()),
        )?
        .serve_at(
            "/org/freedesktop/login1/session/self",
            Session(shared.clone()),
        )?
        .build()?;
    for name in ["org.freedesktop.UPower", "org.freedesktop.login1"] {
        connection.request_name_with_flags(name, fdo::RequestNameFlags::DoNotQueue.into())?;
    }
    shared.record(json!({"event":"ready"}));
    let mut commands = BufReader::new(File::open(options.commands)?);
    let mut pending = String::new();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(120) {
        commands.read_line(&mut pending)?;
        if !pending.ends_with('\n') {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let value: Value = serde_json::from_str(&pending)?;
        pending.clear();
        if value["command"] == "quit" {
            break;
        }
        if value["command"] == "release" {
            connection.release_name("org.freedesktop.UPower")?;
        }
        if value["command"] == "acquire" {
            connection.request_name_with_flags(
                "org.freedesktop.UPower",
                fdo::RequestNameFlags::DoNotQueue.into(),
            )?;
        }
        {
            let mut data = shared.data.lock().unwrap();
            if let Some(v) = value["percent"].as_f64() {
                data.percent = v;
            }
            if let Some(v) = value["state"].as_u64() {
                data.state = v as u32;
            }
            if let Some(v) = value["level"].as_u64() {
                data.level = v as u32;
            }
            if let Some(v) = value["present"].as_bool() {
                data.present = v;
            }
            if let Some(v) = value["on_battery"].as_bool() {
                data.on_battery = v;
            }
            if let Some(v) = value["active"].as_bool() {
                data.active = v;
            }
            if let Some(v) = value["remote"].as_bool() {
                data.remote = v;
            }
            if let Some(v) = value["deny"].as_bool() {
                data.deny = v;
            }
            if let Some(v) = value["delay"].as_u64() {
                data.delay = v.min(1500);
            }
            if let Some(v) = value["confirm"].as_bool() {
                data.confirm = v;
            }
        }
        for (path, interface) in [
            ("/org/freedesktop/UPower", "org.freedesktop.UPower"),
            (
                "/org/freedesktop/UPower/devices/DisplayDevice",
                "org.freedesktop.UPower.Device",
            ),
            (
                "/org/freedesktop/login1/session/self",
                "org.freedesktop.login1.Session",
            ),
        ] {
            connection.emit_signal(
                None::<&str>,
                path,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(
                    interface,
                    HashMap::<String, OwnedValue>::new(),
                    vec!["State", "Percentage"],
                ),
            )?;
        }
        shared.record(json!({"event":"reply","request":value["request"],"ok":true}));
    }
    connection.close()?;
    Ok(())
}
