//! Boot destination for the Power page: reads the system configuration and
//! asks the root helper to change it through pkexec, off the compositor thread.
//! Tarot's Polkit agent presents the administrator authentication.
use crate::{CalloopData, Compositor};
use smithay::reexports::calloop::{LoopHandle, channel};
use std::{
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use tarot::boot_destination::{self, Destination, Paths};
use tarot_core::display::BootDestination;

const HELPER: &str = "/usr/libexec/tarot-boot-destination";
pub const PKEXEC: &str = "/usr/bin/pkexec";
/// Authentication waits for a person; a stuck helper must still end.
const CHANGE_TIMEOUT: Duration = Duration::from_secs(300);

enum Update {
    Current(Result<BootDestination, String>),
    Finished(Option<String>),
}

pub struct Server {
    requests: mpsc::SyncSender<Option<BootDestination>>,
}

impl Server {
    pub fn request(&self, choice: BootDestination) -> bool {
        self.requests.try_send(Some(choice)).is_ok()
    }
    /// Reread the files; dropped while a change is still waiting.
    pub fn refresh(&self) {
        let _ = self.requests.try_send(None);
    }
}

fn observed() -> Result<BootDestination, String> {
    match boot_destination::status(&Paths::default()) {
        Ok(status) => Ok(match status.destination {
            Destination::Greeter => BootDestination::Greeter,
            Destination::Tarot => BootDestination::Tarot,
        }),
        Err(error) => Err(format!("Startup setting unavailable: {error}")),
    }
}

fn change(choice: BootDestination) -> Option<String> {
    let argument = match choice {
        BootDestination::Greeter => "greeter",
        BootDestination::Tarot => "tarot",
    };
    run_helper(HELPER, argument, "the startup setting")
}

/// Runs `pkexec <helper> set <argument>` and describes any failure; pkexec
/// reports a dismissed or denied authentication as 126/127.
pub fn run_helper(helper: &str, argument: &str, setting: &str) -> Option<String> {
    let mut child = match Command::new(PKEXEC)
        .args([helper, "set", argument])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return Some("Could not ask for authorization".into()),
    };
    let deadline = Instant::now() + CHANGE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return match status.code() {
                    Some(0) => None,
                    Some(126 | 127) => Some("Not changed: authorization was not given".into()),
                    _ => Some(format!("Not changed: {setting} could not be written")),
                };
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Some("Not changed: authorization timed out".into());
            }
        }
    }
}

/// Starts only where the installed helper exists; nested sessions never show it.
pub fn start(
    handle: &LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
) -> anyhow::Result<Option<Server>> {
    if !Path::new(HELPER).is_file() || !Path::new(PKEXEC).is_file() {
        return Ok(None);
    }
    let (sender, receiver) = channel::channel::<Update>();
    handle
        .insert_source(receiver, |event, _, data| {
            let channel::Event::Msg(update) = event else {
                return;
            };
            let boot = &mut data.state.shell.display.boot;
            match update {
                Update::Current(Ok(current)) => boot.current = Some(current),
                Update::Current(Err(error)) => {
                    boot.current = None;
                    boot.error = Some(error);
                }
                Update::Finished(error) => {
                    boot.pending = None;
                    if error.is_some() {
                        boot.error = error;
                    }
                }
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let (requests, incoming) = mpsc::sync_channel::<Option<BootDestination>>(1);
    std::thread::spawn(move || {
        let _ = sender.send(Update::Current(observed()));
        while let Ok(request) = incoming.recv() {
            let Some(choice) = request else {
                let _ = sender.send(Update::Current(observed()));
                continue;
            };
            let error = change(choice);
            // Report what the files now say, not what was requested.
            let _ = sender.send(Update::Current(observed()));
            let _ = sender.send(Update::Finished(error));
        }
    });
    state.shell.display.boot.available = true;
    Ok(Some(Server { requests }))
}
