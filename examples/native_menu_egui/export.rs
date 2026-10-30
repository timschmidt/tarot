//! Cooperating sample producer, using Rust/zbus rather than GIO. These GTK
//! interfaces are private; this is an example, not a stable application SDK.
use anyhow::{Context, Result};
use serde_json::json;
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use zbus::{
    blocking::{Connection, connection::Builder},
    fdo,
    zvariant::{OwnedValue, Signature, Value},
};

pub const MENU: &str = "/org/tarot/MenuEgui/menu";
pub const APP: &str = "/org/tarot/MenuEgui/app";
pub const WINDOW: &str = "/org/tarot/MenuEgui/window";
type Attributes = HashMap<String, OwnedValue>;
type Description = (bool, Signature, Vec<OwnedValue>);
type Descriptions = HashMap<String, Description>;
type Changes = (Vec<String>, HashMap<String, bool>, Attributes, Descriptions);

fn text(value: &str) -> OwnedValue {
    Value::from(value).try_into().unwrap()
}
fn signature(value: &'static str) -> Signature {
    Signature::try_from(value).unwrap()
}
fn invalid() -> fdo::Error {
    fdo::Error::InvalidArgs("Unsupported sample action or argument".into())
}

pub struct Data {
    pub chosen: String,
    pub flag: bool,
    pub window_value: i32,
    pub choose_enabled: bool,
    pub revision: u64,
    report: Option<File>,
}
impl Data {
    fn record(&mut self, event: serde_json::Value) {
        if let Some(report) = &mut self.report {
            // This is an explicit synthetic fixture report; never an app log.
            if let Err(error) = (|| -> Result<()> {
                let mut line = serde_json::to_vec(&event)?;
                line.push(b'\n');
                report.write_all(&line)?;
                report.flush()?;
                Ok(())
            })() {
                eprintln!("Synthetic menu report failed: {error}");
            }
        }
    }
}
pub struct Shared(pub Mutex<Data>);
impl Shared {
    fn descriptions(&self, window: bool) -> Descriptions {
        let data = self.0.lock().unwrap();
        if window {
            HashMap::from([("window".into(), (true, signature("i"), vec![]))])
        } else {
            HashMap::from([
                (
                    "choose".into(),
                    (
                        data.choose_enabled,
                        signature("s"),
                        vec![text(&data.chosen)],
                    ),
                ),
                (
                    "toggle".into(),
                    (true, signature(""), vec![data.flag.into()]),
                ),
                ("disabled".into(), (false, signature(""), vec![])),
            ])
        }
    }
    fn activate(&self, window: bool, name: &str, args: &[OwnedValue]) -> fdo::Result<Changes> {
        let mut data = self.0.lock().unwrap();
        let mut states = HashMap::new();
        let argument = match (window, name, args) {
            (false, "choose", [value]) => {
                let value = <&str>::try_from(value).map_err(|_| invalid())?;
                if !matches!(value, "day" | "night") {
                    return Err(invalid());
                }
                if !data.choose_enabled {
                    return Ok((vec![], HashMap::new(), states, HashMap::new()));
                }
                data.chosen = value.into();
                states.insert(name.into(), text(value));
                json!(value)
            }
            (false, "toggle", []) => {
                data.flag = !data.flag;
                states.insert(name.into(), data.flag.into());
                serde_json::Value::Null
            }
            (false, "disabled", []) => {
                return Ok((vec![], HashMap::new(), states, HashMap::new()));
            }
            (true, "window", [value]) => {
                data.window_value = i32::try_from(value).map_err(|_| invalid())?;
                json!(data.window_value)
            }
            _ => return Err(invalid()),
        };
        data.revision += 1;
        data.record(json!({"event":"action","name":name,"target":argument}));
        Ok((vec![], HashMap::new(), states, HashMap::new()))
    }
    fn set_state(&self, window: bool, name: &str, value: &OwnedValue) -> fdo::Result<Changes> {
        let mut data = self.0.lock().unwrap();
        let state = match (window, name) {
            (false, "choose") => {
                let value = <&str>::try_from(value).map_err(|_| invalid())?;
                if !matches!(value, "day" | "night") {
                    return Err(invalid());
                }
                data.chosen = value.into();
                text(value)
            }
            (false, "toggle") => {
                data.flag = bool::try_from(value).map_err(|_| invalid())?;
                data.flag.into()
            }
            _ => return Err(invalid()),
        };
        data.revision += 1;
        Ok((
            vec![],
            HashMap::new(),
            HashMap::from([(name.into(), state)]),
            HashMap::new(),
        ))
    }
    pub fn mapped(&self) {
        self.0.lock().unwrap().record(json!({"event":"mapped"}));
    }
    pub fn claim_mutation(&self, command: &str) {
        self.0
            .lock()
            .unwrap()
            .record(json!({"event":"claim-mutation","command":command}));
    }
}

struct Menus;
#[zbus::interface(name = "org.gtk.Menus")]
impl Menus {
    fn start(&self, groups: Vec<u32>) -> fdo::Result<Vec<(u32, u32, Vec<Attributes>)>> {
        if groups.len() > 16 {
            return Err(invalid());
        }
        // A static group needs no Changed signals or retained subscriber table.
        if !groups.contains(&0) {
            return Ok(vec![]);
        }
        let action = |label: &str, name: &str| {
            HashMap::from([("label".into(), text(label)), ("action".into(), text(name))])
        };
        let mut choose = action("Choose night", "app.choose");
        choose.insert("target".into(), text("night"));
        let mut window = action("Window seven", "win.window");
        window.insert("target".into(), 7i32.into());
        Ok(vec![
            (
                0,
                0,
                vec![HashMap::from([
                    ("label".into(), text("_Preferences")),
                    (
                        ":submenu".into(),
                        Value::from((0u32, 1u32)).try_into().unwrap(),
                    ),
                ])],
            ),
            (
                0,
                1,
                vec![
                    choose,
                    action("Toggle flag", "app.toggle"),
                    action("Disabled", "app.disabled"),
                    window,
                ],
            ),
        ])
    }
    fn end(&self, groups: Vec<u32>) -> fdo::Result<()> {
        if groups.len() > 16 {
            Err(invalid())
        } else {
            Ok(())
        }
    }
}
struct Actions {
    shared: Arc<Shared>,
    window: bool,
}
#[zbus::interface(name = "org.gtk.Actions")]
impl Actions {
    fn list(&self) -> Vec<String> {
        self.shared.descriptions(self.window).into_keys().collect()
    }
    fn describe(&self, action_name: &str) -> fdo::Result<(Description,)> {
        self.shared
            .descriptions(self.window)
            .remove(action_name)
            .map(|v| (v,))
            .ok_or_else(invalid)
    }
    fn describe_all(&self) -> Descriptions {
        self.shared.descriptions(self.window)
    }
    async fn activate(
        &self,
        action_name: &str,
        parameter: Vec<OwnedValue>,
        platform_data: Attributes,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        if platform_data.len() > 16 || parameter.len() > 1 {
            return Err(invalid());
        }
        let changes = self.shared.activate(self.window, action_name, &parameter)?;
        self.emit(connection, changes).await
    }
    async fn set_state(
        &self,
        action_name: &str,
        value: OwnedValue,
        platform_data: Attributes,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> fdo::Result<()> {
        if platform_data.len() > 16 {
            return Err(invalid());
        }
        let changes = self.shared.set_state(self.window, action_name, &value)?;
        self.emit(connection, changes).await
    }
}
impl Actions {
    async fn emit(&self, connection: &zbus::Connection, changes: Changes) -> fdo::Result<()> {
        connection
            .emit_signal(
                None::<&str>,
                if self.window { WINDOW } else { APP },
                "org.gtk.Actions",
                "Changed",
                &changes,
            )
            .await?;
        Ok(())
    }
}

pub struct Export {
    pub shared: Arc<Shared>,
    pub connection: Connection,
}
impl Export {
    pub fn start(report: Option<&Path>) -> Result<Self> {
        let report = report
            .map(|path| {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(path)
            })
            .transpose()?;
        let shared = Arc::new(Shared(Mutex::new(Data {
            chosen: "day".into(),
            flag: false,
            window_value: 0,
            choose_enabled: true,
            revision: 0,
            report,
        })));
        Self::connect(shared)
    }
    /// Fixture reconnection preserves callbacks/state, but obtains a new bus name.
    pub fn reconnect(&self) -> Result<Self> {
        Self::connect(self.shared.clone())
    }
    fn connect(shared: Arc<Shared>) -> Result<Self> {
        let address =
            std::env::var("DBUS_SESSION_BUS_ADDRESS").context("explicit session bus required")?;
        let connection = Builder::address(address.as_str())?
            .method_timeout(Duration::from_millis(500))
            .max_queued(16)
            .serve_at(MENU, Menus)?
            .serve_at(
                APP,
                Actions {
                    shared: shared.clone(),
                    window: false,
                },
            )?
            .serve_at(
                WINDOW,
                Actions {
                    shared: shared.clone(),
                    window: true,
                },
            )?
            .build()?;
        shared
            .0
            .lock()
            .unwrap()
            .record(json!({"event":"export","owner":connection.unique_name().unwrap().as_str()}));
        Ok(Self { shared, connection })
    }
    pub fn owner(&self) -> &str {
        self.connection.unique_name().unwrap().as_str()
    }
    pub fn local(&self, name: &str) -> Result<()> {
        let parameters = if name == "choose" {
            vec![text("night")]
        } else {
            vec![]
        };
        let changes = self.shared.activate(false, name, &parameters)?;
        self.connection
            .emit_signal(None::<&str>, APP, "org.gtk.Actions", "Changed", &changes)?;
        Ok(())
    }
    /// Synthetic QA mutations never activate an action or invoke a callback.
    pub fn mutate(&self, command: &str) -> Result<()> {
        if !matches!(command, "flip" | "disable") {
            anyhow::bail!("unknown QA mutation");
        }
        self.shared
            .0
            .lock()
            .unwrap()
            .record(json!({"event":"mutation","command":command}));
        for enabled in if command == "flip" {
            &[false, true][..]
        } else {
            &[false][..]
        } {
            {
                let mut data = self.shared.0.lock().unwrap();
                data.choose_enabled = *enabled;
                data.revision += 1;
            }
            let changes: Changes = (
                vec![],
                HashMap::from([("choose".into(), *enabled)]),
                HashMap::new(),
                HashMap::new(),
            );
            self.connection.emit_signal(
                None::<&str>,
                APP,
                "org.gtk.Actions",
                "Changed",
                &changes,
            )?;
        }
        Ok(())
    }
}
impl Drop for Export {
    fn drop(&mut self) {
        let _ = self.connection.clone().close();
    }
}
