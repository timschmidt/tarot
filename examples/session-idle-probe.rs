//! Drive the same idle publisher as the compositor against a private manager.
use anyhow::{Result, ensure};
use clap::Parser;
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Parser)]
struct Options {
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    commands: PathBuf,
}
fn record(file: &Mutex<File>, value: Value) {
    let mut file = file.lock().unwrap();
    writeln!(file, "{value}").unwrap();
    file.flush().unwrap();
}
fn main() -> Result<()> {
    let options = Options::parse();
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS")?;
    ensure!(
        address.starts_with("unix:path="),
        "Explicitly private bus required"
    );
    let file = Arc::new(Mutex::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(options.report)?,
    ));
    let events = file.clone();
    let worker = tarot::session_idle::start(address, false, move |status| {
        record(&events, json!({"event":"status","status":status}))
    })?;
    record(&file, json!({"event":"ready","pid":std::process::id()}));
    let mut commands = BufReader::new(File::open(options.commands)?);
    let mut line = String::new();
    loop {
        if commands.read_line(&mut line)? == 0 {
            std::thread::sleep(Duration::from_millis(10));
            continue;
        }
        let command: Value = serde_json::from_str(&line)?;
        line.clear();
        match command["command"].as_str().unwrap_or("") {
            "quit" => break,
            "ready" => worker.ready(command["ready"].as_bool().unwrap()),
            _ => anyhow::bail!("Unknown fixture command"),
        }
        record(
            &file,
            json!({"event":"reply","request":command["request"],"ok":true}),
        );
    }
    let started = Instant::now();
    drop(worker);
    record(
        &file,
        json!({"event":"stopped","elapsed_us":started.elapsed().as_micros()}),
    );
    Ok(())
}
