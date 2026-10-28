//! Private-bus SensorProxy fixture. Never connects to the system bus.
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
    time::Duration,
};
use zbus::{blocking::connection::Builder, fdo, zvariant::OwnedValue};

const NAME: &str = "net.hadess.SensorProxy";
const PATH: &str = "/net/hadess/SensorProxy";

#[derive(Parser)]
struct Options {
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    commands: PathBuf,
}

struct Data {
    available: bool,
    orientation: String,
    deny: bool,
    delay: u64,
    claims: u64,
    releases: u64,
}
struct Shared {
    data: Mutex<Data>,
    report: Mutex<File>,
}
impl Shared {
    fn record(&self, value: Value) {
        let mut file = self.report.lock().unwrap();
        writeln!(file, "{value}").unwrap();
        file.flush().unwrap();
    }
}
struct Sensor(Arc<Shared>);
#[zbus::interface(name = "net.hadess.SensorProxy")]
impl Sensor {
    #[zbus(property)]
    fn has_accelerometer(&self) -> bool {
        self.0.data.lock().unwrap().available
    }
    #[zbus(property)]
    fn accelerometer_orientation(&self) -> String {
        self.0.data.lock().unwrap().orientation.clone()
    }
    fn claim_accelerometer(&self) -> fdo::Result<()> {
        let (deny, delay) = {
            let data = self.0.data.lock().unwrap();
            (data.deny, data.delay)
        };
        std::thread::sleep(Duration::from_millis(delay));
        if deny {
            self.0.record(json!({"event":"claim-denied"}));
            return Err(fdo::Error::AccessDenied(
                "Synthetic sensor claim denial".into(),
            ));
        }
        self.0.data.lock().unwrap().claims += 1;
        self.0.record(json!({"event":"claimed"}));
        Ok(())
    }
    fn release_accelerometer(&self) {
        self.0.data.lock().unwrap().releases += 1;
        self.0.record(json!({"event":"released"}));
    }
}

fn main() -> Result<()> {
    let options = Options::parse();
    let shared = Arc::new(Shared {
        data: Mutex::new(Data {
            available: true,
            orientation: "normal".into(),
            deny: false,
            delay: 0,
            claims: 0,
            releases: 0,
        }),
        report: Mutex::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(options.report)?,
        ),
    });
    let connection = Builder::session()?
        .serve_at(PATH, Sensor(shared.clone()))?
        .name(NAME)?
        .build()?;
    shared.record(json!({"event":"ready"}));
    let mut input = BufReader::new(File::open(options.commands)?);
    let mut pending = String::new();
    loop {
        input.read_line(&mut pending)?;
        if !pending.ends_with('\n') {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        let value: Value = serde_json::from_str(pending.trim())?;
        pending.clear();
        match value["command"].as_str().unwrap_or("") {
            "exit" | "quit" => break,
            "release" => {
                connection.release_name(NAME)?;
            }
            "acquire" => {
                connection.request_name(NAME)?;
            }
            "set" => {
                let mut data = shared.data.lock().unwrap();
                if let Some(v) = value["available"].as_bool() {
                    data.available = v;
                }
                if let Some(v) = value["orientation"].as_str() {
                    data.orientation = v.into();
                }
                if let Some(v) = value["deny"].as_bool() {
                    data.deny = v;
                }
                if let Some(v) = value["delay"].as_u64() {
                    data.delay = v.min(5000);
                }
                drop(data);
                connection.emit_signal(
                    None::<&str>,
                    PATH,
                    "org.freedesktop.DBus.Properties",
                    "PropertiesChanged",
                    &(
                        NAME,
                        HashMap::<String, OwnedValue>::new(),
                        vec!["HasAccelerometer", "AccelerometerOrientation"],
                    ),
                )?;
            }
            _ => {}
        }
        let data = shared.data.lock().unwrap();
        shared.record(json!({"event":"reply","request":value["request"],"ok":true,"claims":data.claims,"releases":data.releases}));
    }
    Ok(())
}
