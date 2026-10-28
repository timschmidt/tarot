//! Synthetic native D-Bus client. Use only fixture text in its optional trace.
use anyhow::{Context, Result};
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
    blocking::{Connection, Proxy},
    zvariant::OwnedValue,
};

#[derive(Parser)]
struct Options {
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    commands: PathBuf,
    #[arg(long, default_value_t = 120)]
    exit_after_seconds: u64,
    #[arg(long)]
    hold_name: bool,
}

fn record(file: &Arc<Mutex<File>>, value: Value) {
    let mut file = file.lock().unwrap();
    writeln!(file, "{value}").unwrap();
    file.flush().unwrap();
}

fn main() -> Result<()> {
    let options = Options::parse();
    let report = Arc::new(Mutex::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&options.report)?,
    ));
    let connection = Connection::session()?;
    if options.hold_name {
        connection.request_name_with_flags(
            "org.freedesktop.Notifications",
            zbus::fdo::RequestNameFlags::DoNotQueue.into(),
        )?;
        record(&report, json!({"event":"owning-name"}));
        let mut commands = BufReader::new(File::open(options.commands)?);
        let started = Instant::now();
        let mut line = String::new();
        while started.elapsed() < Duration::from_secs(options.exit_after_seconds) {
            commands.read_line(&mut line)?;
            if line.ends_with('\n') {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        connection.close()?;
        return Ok(());
    }
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )?;
    let capabilities: Vec<String> = proxy.call("GetCapabilities", &())?;
    let info: (String, String, String, String) = proxy.call("GetServerInformation", &())?;
    let signals = proxy.receive_all_signals()?;
    let signal_report = report.clone();
    let watcher = std::thread::spawn(move || {
        for signal in signals {
            match signal.header().member().map(|name| name.as_str()) {
                Some("ActionInvoked") => {
                    if let Ok((id, key)) = signal.body().deserialize::<(u32, String)>() {
                        record(&signal_report, json!({"event":"action","id":id,"key":key}));
                    }
                }
                Some("NotificationClosed") => {
                    if let Ok((id, reason)) = signal.body().deserialize::<(u32, u32)>() {
                        record(
                            &signal_report,
                            json!({"event":"closed","id":id,"reason":reason}),
                        );
                    }
                }
                _ => {}
            }
        }
    });
    record(
        &report,
        json!({"event":"ready","capabilities":capabilities,"info":info}),
    );
    let mut commands = BufReader::new(File::open(options.commands)?);
    let started = Instant::now();
    let mut pending = String::new();
    while started.elapsed() < Duration::from_secs(options.exit_after_seconds) {
        commands.read_line(&mut pending)?;
        if !pending.ends_with('\n') {
            std::thread::sleep(Duration::from_millis(20));
            continue;
        }
        let value: Value = serde_json::from_str(&pending)?;
        pending.clear();
        let command = value["command"].as_str().context("missing command")?;
        if command == "quit" {
            break;
        }
        let result: zbus::Result<Value> = match command {
            "notify" => {
                let text =
                    |key: &str, default: &str| value[key].as_str().unwrap_or(default).to_string();
                let actions: Vec<String> =
                    serde_json::from_value(value.get("actions").cloned().unwrap_or(json!([])))?;
                let hints = HashMap::from([
                    (
                        "urgency",
                        OwnedValue::from(value["urgency"].as_u64().unwrap_or(1) as u8),
                    ),
                    (
                        "resident",
                        OwnedValue::from(value["resident"].as_bool().unwrap_or(false)),
                    ),
                    (
                        "transient",
                        OwnedValue::from(value["transient"].as_bool().unwrap_or(false)),
                    ),
                ]);
                let result: zbus::Result<u32> = proxy.call(
                    "Notify",
                    &(
                        text("app_name", "Tarot fixture"),
                        value["replaces_id"].as_u64().unwrap_or(0) as u32,
                        "",
                        text("summary", "Synthetic notification"),
                        text("body", "Fixture body"),
                        actions,
                        hints,
                        value["timeout"].as_i64().unwrap_or(-1) as i32,
                    ),
                );
                result.map(|id| json!({"id":id}))
            }
            "close" => proxy
                .call::<_, _, ()>(
                    "CloseNotification",
                    &(value["id"].as_u64().unwrap() as u32,),
                )
                .map(|()| json!({})),
            _ => anyhow::bail!("unknown fixture command"),
        };
        let reply = match result {
            Ok(fields) => {
                json!({"event":"reply","request":value["request"],"ok":true,"result":fields})
            }
            Err(error) => {
                json!({"event":"reply","request":value["request"],"ok":false,"error":error.to_string()})
            }
        };
        record(&report, reply);
    }
    connection.close()?;
    watcher
        .join()
        .map_err(|_| anyhow::anyhow!("notification signal observer panicked"))?;
    Ok(())
}
