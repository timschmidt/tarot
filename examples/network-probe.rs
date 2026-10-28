//! Synthetic NetworkManager service for private-bus regression. Never run on the system bus.
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
    zvariant::{OwnedObjectPath, OwnedValue, Value as Variant},
};
const NM: &str = "org.freedesktop.NetworkManager";
const ROOT: &str = "/org/freedesktop/NetworkManager";
const DEV: &str = "/org/freedesktop/NetworkManager/Devices/1";
const USB: &str = "/org/freedesktop/NetworkManager/Devices/2";
fn path(value: &str) -> OwnedObjectPath {
    value.try_into().unwrap()
}
fn ap(i: u8) -> OwnedObjectPath {
    path(&format!("{ROOT}/AccessPoint/{i}"))
}
fn profile(i: u8) -> OwnedObjectPath {
    path(&format!("{ROOT}/Settings/{i}"))
}
fn active(i: u8) -> OwnedObjectPath {
    path(&format!("{ROOT}/ActiveConnection/{i}"))
}
#[derive(Parser)]
struct Options {
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    commands: PathBuf,
    #[arg(long)]
    credentials: bool,
}
struct Data {
    radio: bool,
    hardware: bool,
    permission: String,
    deny: bool,
    state: u32,
    connected: u8,
    last_scan: i64,
    requested: u8,
    second: bool,
    credentials: bool,
    created: HashMap<u8, HashMap<String, HashMap<String, OwnedValue>>>,
    agent: Option<String>,
}
struct Shared {
    data: Mutex<Data>,
    report: Mutex<File>,
}
impl Shared {
    fn record(&self, value: Value) {
        let mut f = self.report.lock().unwrap();
        writeln!(f, "{value}").unwrap();
        f.flush().unwrap();
    }
}
struct Manager(Arc<Shared>);
#[zbus::interface(name = "org.freedesktop.NetworkManager")]
impl Manager {
    fn get_devices(&self) -> Vec<OwnedObjectPath> {
        vec![path(DEV), path(USB)]
    }
    fn get_permissions(&self) -> HashMap<String, String> {
        let permission = self.0.data.lock().unwrap().permission.clone();
        [
            "wifi.scan",
            "network-control",
            "enable-disable-wifi",
            "settings.modify.own",
        ]
        .iter()
        .map(|name| (format!("{NM}.{name}"), permission.clone()))
        .collect()
    }
    fn activate_connection(
        &self,
        connection: OwnedObjectPath,
        device: OwnedObjectPath,
        specific_object: OwnedObjectPath,
    ) -> fdo::Result<OwnedObjectPath> {
        let mut d = self.0.data.lock().unwrap();
        self.0.record(json!({"event":"activate","profile":connection.as_str(),"device":device.as_str(),"ap":specific_object.as_str()}));
        if d.deny {
            return Err(fdo::Error::AccessDenied("Fixture denied activation".into()));
        }
        let index = if connection == profile(1) {
            1
        } else if connection == profile(2) {
            2
        } else if let Some(index) = d
            .created
            .keys()
            .find(|i| profile(**i) == connection)
            .copied()
        {
            index
        } else {
            return Err(fdo::Error::InvalidArgs("Unknown profile".into()));
        };
        if device != path(DEV) || specific_object != ap(index) {
            return Err(fdo::Error::InvalidArgs("Wrong target".into()));
        }
        d.requested = index;
        Ok(active(index))
    }
    fn add_and_activate_connection2(
        &self,
        mut settings: HashMap<String, HashMap<String, OwnedValue>>,
        device: OwnedObjectPath,
        specific_object: OwnedObjectPath,
        options: HashMap<String, OwnedValue>,
    ) -> fdo::Result<(
        OwnedObjectPath,
        OwnedObjectPath,
        HashMap<String, OwnedValue>,
    )> {
        let mut d = self.0.data.lock().unwrap();
        if d.deny {
            return Err(fdo::Error::AccessDenied("Denied".into()));
        }
        if device != path(DEV) {
            return Err(fdo::Error::InvalidArgs("Wrong device".into()));
        }
        let index = if specific_object == ap(5) {
            5
        } else if specific_object == ap(3) {
            3
        } else {
            return Err(fdo::Error::InvalidArgs("Unexpected AP".into()));
        };
        let security = settings.get_mut("802-11-wireless-security");
        let flags = security
            .as_ref()
            .and_then(|s| s.get("psk-flags"))
            .and_then(|v| u32::try_from(v).ok());
        let matches = security
            .as_ref()
            .and_then(|s| s.get("psk"))
            .and_then(|v| <&str>::try_from(v).ok())
            .is_some_and(|s| s == "tarot-test-psk");
        if let Some(security) = security {
            security.remove("psk");
        }
        let persist = options
            .get("persist")
            .and_then(|v| <&str>::try_from(v).ok())
            .unwrap_or("");
        let permissions = settings
            .get("connection")
            .and_then(|s| s.get("permissions"))
            .and_then(|v| v.try_clone().ok())
            .and_then(|v| Vec::<String>::try_from(v).ok());
        self.0.record(json!({"event":"create","ap_index":index,"password_matches":matches,"secret_flags":flags,"persist":persist,"private_user":permissions.is_some_and(|p|p.len()==1&&p[0].starts_with("user:")),"device":device.as_str()}));
        settings.entry("connection".into()).or_default().insert(
            "uuid".into(),
            Variant::from(format!("fixture-new-{index}"))
                .try_into()
                .unwrap(),
        );
        d.created.insert(index, settings);
        d.requested = index;
        Ok((profile(index), active(index), HashMap::new()))
    }
    #[zbus(property)]
    fn wireless_enabled(&self) -> bool {
        self.0.data.lock().unwrap().radio
    }
    #[zbus(property)]
    fn set_wireless_enabled(&self, value: bool) -> fdo::Result<()> {
        let mut d = self.0.data.lock().unwrap();
        self.0.record(json!({"event":"radio","enabled":value}));
        if d.deny {
            return Err(fdo::Error::AccessDenied("Denied".into()));
        }
        d.radio = value;
        if !value {
            d.state = 30;
            d.connected = 0;
        }
        Ok(())
    }
    #[zbus(property)]
    fn wireless_hardware_enabled(&self) -> bool {
        self.0.data.lock().unwrap().hardware
    }
    #[zbus(property)]
    fn connectivity(&self) -> u32 {
        4
    }
}
struct Device {
    shared: Arc<Shared>,
    usb: bool,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Device")]
impl Device {
    #[zbus(property)]
    fn device_type(&self) -> u32 {
        if self.usb { 1 } else { 2 }
    }
    #[zbus(property)]
    fn state(&self) -> u32 {
        if self.usb {
            100
        } else {
            self.shared.data.lock().unwrap().state
        }
    }
    #[zbus(property)]
    fn state_reason(&self) -> (u32, u32) {
        (self.state(), 0)
    }
    #[zbus(property)]
    fn available_connections(&self) -> Vec<OwnedObjectPath> {
        if self.usb {
            vec![]
        } else {
            let d = self.shared.data.lock().unwrap();
            let mut paths = vec![profile(1), profile(2)];
            paths.extend(d.created.keys().map(|i| profile(*i)));
            paths
        }
    }
    #[zbus(property)]
    fn active_connection(&self) -> OwnedObjectPath {
        let d = self.shared.data.lock().unwrap();
        if self.usb || d.connected == 0 {
            path("/")
        } else {
            active(d.connected)
        }
    }
    fn disconnect(&self) -> fdo::Result<()> {
        self.shared
            .record(json!({"event":"disconnect","usb":self.usb}));
        if self.usb {
            return Err(fdo::Error::AccessDenied("USB must remain untouched".into()));
        }
        let mut d = self.shared.data.lock().unwrap();
        if d.deny {
            return Err(fdo::Error::AccessDenied("Denied".into()));
        }
        d.state = 30;
        d.connected = 0;
        Ok(())
    }
}
struct Wireless(Arc<Shared>);
#[zbus::interface(name = "org.freedesktop.NetworkManager.Device.Wireless")]
impl Wireless {
    fn get_all_access_points(&self) -> Vec<OwnedObjectPath> {
        let d = self.0.data.lock().unwrap();
        let mut points = vec![ap(1), ap(3), ap(4)];
        if d.second {
            points.push(ap(2));
        }
        if d.credentials {
            points.push(ap(5));
        }
        points
    }
    fn request_scan(&self, _options: HashMap<String, OwnedValue>) -> fdo::Result<()> {
        self.0.record(json!({"event":"scan"}));
        if self.0.data.lock().unwrap().deny {
            Err(fdo::Error::AccessDenied("Denied".into()))
        } else {
            Ok(())
        }
    }
    #[zbus(property)]
    fn active_access_point(&self) -> OwnedObjectPath {
        let d = self.0.data.lock().unwrap();
        if d.connected == 0 {
            path("/")
        } else {
            ap(d.connected)
        }
    }
    #[zbus(property)]
    fn last_scan(&self) -> i64 {
        self.0.data.lock().unwrap().last_scan
    }
}
struct AccessPoint {
    index: u8,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.AccessPoint")]
impl AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> Vec<u8> {
        if self.index == 5 {
            b"Tarot New".to_vec()
        } else if self.index == 2 {
            b"Tarot Guest".to_vec()
        } else {
            b"Tarot Lab".to_vec()
        }
    }
    #[zbus(property)]
    fn flags(&self) -> u32 {
        if self.index == 3 { 0 } else { 1 }
    }
    #[zbus(property)]
    fn wpa_flags(&self) -> u32 {
        0
    }
    #[zbus(property)]
    fn rsn_flags(&self) -> u32 {
        if self.index == 3 { 0 } else { 0x100 }
    }
    #[zbus(property)]
    fn strength(&self) -> u8 {
        match self.index {
            1 => 90,
            2 => 60,
            3 => 55,
            _ => 30,
        }
    }
}
struct Saved {
    shared: Arc<Shared>,
    index: u8,
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Settings.Connection")]
impl Saved {
    fn get_settings(&self) -> HashMap<String, HashMap<String, OwnedValue>> {
        if let Some(settings) = self.shared.data.lock().unwrap().created.get(&self.index) {
            return settings
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        v.iter()
                            .map(|(k, v)| (k.clone(), v.try_clone().unwrap()))
                            .collect(),
                    )
                })
                .collect();
        }
        let ssid = if self.index == 1 {
            b"Tarot Lab".to_vec()
        } else {
            b"Tarot Guest".to_vec()
        };
        HashMap::from([
            (
                "connection".into(),
                HashMap::from([
                    (
                        "id".into(),
                        Variant::from(format!("Saved {}", self.index))
                            .try_into()
                            .unwrap(),
                    ),
                    (
                        "uuid".into(),
                        Variant::from(format!("fixture-{}", self.index))
                            .try_into()
                            .unwrap(),
                    ),
                ]),
            ),
            (
                "802-11-wireless".into(),
                HashMap::from([("ssid".into(), Variant::from(ssid).try_into().unwrap())]),
            ),
            (
                "802-11-wireless-security".into(),
                HashMap::from([(
                    "key-mgmt".into(),
                    Variant::from("wpa-psk").try_into().unwrap(),
                )]),
            ),
        ])
    }
    fn get_secrets(
        &self,
        _setting_name: &str,
    ) -> fdo::Result<HashMap<String, HashMap<String, OwnedValue>>> {
        self.shared.record(json!({"event":"secrets-requested"}));
        Err(fdo::Error::AccessDenied("Secrets must not be read".into()))
    }
}
struct Active {
    index: u8,
}
struct AgentManager(Arc<Shared>);
#[zbus::interface(name = "org.freedesktop.NetworkManager.AgentManager")]
impl AgentManager {
    fn register_with_capabilities(
        &self,
        identifier: &str,
        capabilities: u32,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> fdo::Result<()> {
        if identifier != "org.tarot.Shell" || capabilities != 0 {
            return Err(fdo::Error::InvalidArgs("Unexpected registration".into()));
        }
        self.0.data.lock().unwrap().agent = header.sender().map(ToString::to_string);
        self.0.record(json!({"event":"agent-registered"}));
        Ok(())
    }
    fn unregister(&self) {
        self.0.data.lock().unwrap().agent = None;
    }
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.Connection.Active")]
impl Active {
    #[zbus(property)]
    fn connection(&self) -> OwnedObjectPath {
        profile(self.index)
    }
}
fn main() -> Result<()> {
    let options = Options::parse();
    let shared = Arc::new(Shared {
        data: Mutex::new(Data {
            radio: true,
            hardware: true,
            permission: "yes".into(),
            deny: false,
            state: 100,
            connected: 1,
            last_scan: 10,
            requested: 0,
            second: true,
            credentials: options.credentials,
            created: HashMap::new(),
            agent: None,
        }),
        report: Mutex::new(
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(options.report)?,
        ),
    });
    let mut builder = Builder::session()?
        .method_timeout(Duration::from_secs(150))
        .serve_at(ROOT, Manager(shared.clone()))?
        .serve_at(
            DEV,
            Device {
                shared: shared.clone(),
                usb: false,
            },
        )?
        .serve_at(DEV, Wireless(shared.clone()))?
        .serve_at(
            USB,
            Device {
                shared: shared.clone(),
                usb: true,
            },
        )?;
    if options.credentials {
        builder = builder.serve_at(
            "/org/freedesktop/NetworkManager/AgentManager",
            AgentManager(shared.clone()),
        )?;
    }
    for i in 1..=5 {
        builder = builder.serve_at(ap(i), AccessPoint { index: i })?;
    }
    for i in 1..=5 {
        builder = builder
            .serve_at(
                profile(i),
                Saved {
                    shared: shared.clone(),
                    index: i,
                },
            )?
            .serve_at(active(i), Active { index: i })?;
    }
    let connection = builder.build()?;
    connection.request_name_with_flags(NM, fdo::RequestNameFlags::DoNotQueue.into())?;
    shared.record(json!({"event":"ready"}));
    let mut commands = BufReader::new(File::open(options.commands)?);
    let mut pending = String::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(180) {
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
            connection.release_name(NM)?;
        }
        if value["command"] == "acquire" {
            connection.request_name_with_flags(NM, fdo::RequestNameFlags::DoNotQueue.into())?;
        }
        if value["command"] == "secret-request" {
            let target = shared.data.lock().unwrap().agent.clone().unwrap();
            let request = value["request"].clone();
            let shared = shared.clone();
            let caller = if value["spoof"].as_bool() == Some(true) {
                Builder::session()?.build()?
            } else {
                connection.clone()
            };
            let index = value["index"].as_u64().unwrap_or(1) as u8;
            let flags = value["flags"].as_u64().unwrap_or(5) as u32;
            std::thread::spawn(move || {
                let saved = Saved {
                    shared: shared.clone(),
                    index,
                };
                let result: zbus::Result<HashMap<String, HashMap<String, OwnedValue>>> =
                    zbus::blocking::Proxy::new(
                        &caller,
                        target.as_str(),
                        "/org/freedesktop/NetworkManager/SecretAgent",
                        "org.freedesktop.NetworkManager.SecretAgent",
                    )
                    .unwrap()
                    .call(
                        "GetSecrets",
                        &(
                            saved.get_settings(),
                            profile(index),
                            "802-11-wireless-security",
                            vec!["psk"],
                            flags,
                        ),
                    );
                let (ok, matches, error) = match result {
                    Ok(reply) => (
                        true,
                        reply
                            .get("802-11-wireless-security")
                            .and_then(|s| s.get("psk"))
                            .and_then(|v| <&str>::try_from(v).ok())
                            == Some("tarot-test-psk"),
                        None,
                    ),
                    Err(e) => (
                        false,
                        false,
                        Some(match e {
                            zbus::Error::MethodError(name, _, _) => name.to_string(),
                            _ => "TransportError".into(),
                        }),
                    ),
                };
                shared.record(json!({"event":"secret-result","request":request,"ok":ok,"password_matches":matches,"error":error}));
            });
        }
        if value["command"] == "secret-cancel" {
            let target = shared.data.lock().unwrap().agent.clone().unwrap();
            zbus::blocking::Proxy::new(
                &connection,
                target.as_str(),
                "/org/freedesktop/NetworkManager/SecretAgent",
                "org.freedesktop.NetworkManager.SecretAgent",
            )?
            .call::<_, _, ()>(
                "CancelGetSecrets",
                &(profile(1), "802-11-wireless-security"),
            )?;
        }
        {
            let mut d = shared.data.lock().unwrap();
            if value["command"] == "complete-scan" {
                d.last_scan += 1;
            }
            if value["command"] == "complete-connect" {
                d.connected = d.requested;
                d.state = 100;
            }
            if value["command"] == "auth-needed" {
                d.connected = d.requested;
                d.state = 60;
            }
            if let Some(v) = value["radio"].as_bool() {
                d.radio = v;
            }
            if let Some(v) = value["hardware"].as_bool() {
                d.hardware = v;
            }
            if let Some(v) = value["deny"].as_bool() {
                d.deny = v;
            }
            if let Some(v) = value["permission"].as_str() {
                d.permission = v.to_owned();
            }
            if let Some(v) = value["state"].as_u64() {
                d.state = v as u32;
            }
            if let Some(v) = value["connected"].as_u64() {
                d.connected = v as u8;
            }
            if let Some(v) = value["second"].as_bool() {
                d.second = v;
            }
        }
        for (p, interface) in [
            (ROOT, NM),
            (DEV, "org.freedesktop.NetworkManager.Device"),
            (DEV, "org.freedesktop.NetworkManager.Device.Wireless"),
        ] {
            connection.emit_signal(
                None::<&str>,
                p,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                &(
                    interface,
                    HashMap::<String, OwnedValue>::new(),
                    vec!["State", "LastScan"],
                ),
            )?;
        }
        shared.record(json!({"event":"reply","request":value["request"],"ok":true}));
    }
    connection.close()?;
    Ok(())
}
