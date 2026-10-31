//! Read or change whether the system sleeps when every session is idle.
//! Changes run as root through pkexec.
//!
//! `status` prints JSON for anyone. `set on|off` requires root; turning it off
//! reloads elogind so `IdleAction=ignore` applies at once.
use anyhow::{Context, Result, bail, ensure};
use std::path::Path;
use tarot::sleep_policy::{self, Paths};

const ELOGIND: &str = "/usr/libexec/elogind/elogind";
const PID_FILE: &str = "/run/elogind.pid";

/// Signal only the elogind daemon named by its pid file, held through a pidfd
/// so a recycled pid can never receive it.
fn reload_elogind() -> Result<()> {
    let raw: i32 = std::fs::read_to_string(PID_FILE)
        .context("reading elogind's pid file")?
        .trim()
        .parse()
        .context("invalid elogind pid")?;
    let pid = rustix::process::Pid::from_raw(raw).context("invalid elogind pid")?;
    let pidfd = rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty())
        .context("elogind is not running")?;
    ensure!(
        std::fs::read_link(format!("/proc/{raw}/exe"))? == Path::new(ELOGIND),
        "the pid file does not name elogind"
    );
    rustix::process::pidfd_send_signal(&pidfd, rustix::process::Signal::HUP)?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let paths = Paths::default();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["status"] => {}
        ["set", choice] => {
            ensure!(
                rustix::process::geteuid().is_root(),
                "changing the sleep policy requires administrator authorization"
            );
            match *choice {
                "on" => sleep_policy::enable(&paths)?,
                "off" => {
                    if sleep_policy::disable(&paths)? {
                        reload_elogind()?;
                    }
                }
                _ => bail!("choose on or off"),
            }
        }
        _ => bail!("usage: tarot-sleep-policy status | set on|off"),
    }
    println!("{}", serde_json::to_string(&sleep_policy::status(&paths)?)?);
    Ok(())
}
