//! Development client for Tarot's explicitly enabled private control socket.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    time::Duration,
};

#[derive(Parser)]
#[command(about = "Inspect and control a Tarot development session")]
struct Options {
    #[arg(long)]
    socket: PathBuf,
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    Lock,
    RestartLocker,
    Status,
    Home,
    Overview,
    Launcher,
    Notifications {
        #[arg(long)]
        open: bool,
    },
    Applications,
    Launch {
        id: String,
    },
    Activate {
        id: u64,
    },
    Close {
        id: u64,
    },
    Capture {
        path: PathBuf,
    },
    Exit,
}

fn main() -> Result<()> {
    let options = Options::parse();
    let request = match options.command {
        Action::Lock => json!({"command":"lock"}),
        Action::RestartLocker => json!({"command":"restart_locker"}),
        Action::Status => json!({"command": "status"}),
        Action::Home => json!({"command": "home"}),
        Action::Overview => json!({"command": "overview"}),
        Action::Launcher => json!({"command": "launcher"}),
        Action::Notifications { open } => json!({"command":"notifications","open":open}),
        Action::Applications => json!({"command": "applications"}),
        Action::Launch { id } => json!({"command": "launch", "id": id}),
        Action::Activate { id } => json!({"command": "activate", "id": id}),
        Action::Close { id } => json!({"command": "close", "id": id}),
        Action::Capture { path } => json!({"command": "capture", "path": path}),
        Action::Exit => json!({"command": "exit"}),
    };
    let mut stream =
        UnixStream::connect(options.socket).context("connecting to Tarot control socket")?;
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    serde_json::to_writer(&mut stream, &request)?;
    stream.write_all(b"\n")?;
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    let value: serde_json::Value =
        serde_json::from_str(&response).context("invalid control response")?;
    anyhow::ensure!(value["ok"] == true, "{}", value["error"]);
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
