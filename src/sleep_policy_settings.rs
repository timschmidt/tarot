//! System sleep policy for the Power page: reads the elogind drop-ins and asks
//! the root helper to change them through pkexec, off the compositor thread.
//! Tarot's Polkit agent presents the administrator authentication.
use crate::{CalloopData, Compositor, boot_settings};
use smithay::reexports::calloop::{LoopHandle, channel};
use std::{path::Path, sync::mpsc};
use tarot::sleep_policy::{self, Paths, Status};

const HELPER: &str = "/usr/libexec/tarot-sleep-policy";

enum Update {
    Current(Result<Status, String>),
    Finished(Option<String>),
}

pub struct Server {
    requests: mpsc::SyncSender<Option<bool>>,
}

impl Server {
    pub fn request(&self, enabled: bool) -> bool {
        self.requests.try_send(Some(enabled)).is_ok()
    }
    /// Reread the drop-ins; dropped while a change is still waiting.
    pub fn refresh(&self) {
        let _ = self.requests.try_send(None);
    }
}

fn observed() -> Result<Status, String> {
    sleep_policy::status(&Paths::default())
        .map_err(|error| format!("Sleep policy unavailable: {error}"))
}

/// Starts only where the installed helper exists; nested sessions never show it.
pub fn start(
    handle: &LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
) -> anyhow::Result<Option<Server>> {
    if !Path::new(HELPER).is_file() || !Path::new(boot_settings::PKEXEC).is_file() {
        return Ok(None);
    }
    let (sender, receiver) = channel::channel::<Update>();
    handle
        .insert_source(receiver, |event, _, data| {
            let channel::Event::Msg(update) = event else {
                return;
            };
            let policy = &mut data.state.shell.display.sleep_policy;
            match update {
                Update::Current(Ok(status)) => {
                    if policy.enabled.is_none() {
                        policy.error = None;
                    }
                    policy.enabled = Some(status.enabled);
                    policy.package = status.package;
                    policy.restart_required = status.restart_required;
                }
                Update::Current(Err(error)) => {
                    policy.enabled = None;
                    policy.error = Some(error);
                }
                Update::Finished(error) => {
                    policy.pending = None;
                    if error.is_some() {
                        policy.error = error;
                    }
                }
            }
            data.state.ui_dirty = true;
            data.state.redraw = true;
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let (requests, incoming) = mpsc::sync_channel::<Option<bool>>(1);
    std::thread::spawn(move || {
        let _ = sender.send(Update::Current(observed()));
        while let Ok(request) = incoming.recv() {
            let Some(enabled) = request else {
                let _ = sender.send(Update::Current(observed()));
                continue;
            };
            let argument = if enabled { "on" } else { "off" };
            let error = boot_settings::run_helper(HELPER, argument, "the sleep policy");
            // Report what the files now say, not what was requested.
            let _ = sender.send(Update::Current(observed()));
            let _ = sender.send(Update::Finished(error));
        }
    });
    state.shell.display.sleep_policy.available = true;
    Ok(Some(Server { requests }))
}
