//! Explicitly enabled development control. Blocking socket I/O stays off the compositor thread.

use crate::CalloopData;
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use smithay::reexports::calloop::{LoopHandle, channel};
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tarot_core::{Command, WindowId};

#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    IdleTimeouts {
        settings: tarot_core::idle::Settings,
    },
    RotationLock {
        locked: bool,
    },
    Rotate {
        orientation: tarot_core::orientation::Orientation,
    },
    Lock,
    Status,
    RestartLocker,
    Home,
    Overview,
    Launcher,
    Notifications {
        open: bool,
    },
    NotificationAction {
        action: tarot_core::notifications::UiAction,
    },
    SystemMenu {
        open: bool,
    },
    PowerAction {
        action: tarot_core::power::UiAction,
    },
    AudioAction {
        action: tarot_core::audio::UiAction,
    },
    NetworkAction {
        action: tarot_core::network::UiAction,
    },
    NetworkSetup {
        network: u64,
    },
    CredentialText {
        value: tarot_core::network::Secret,
    },
    CredentialBackspace,
    CredentialRemember {
        remember: bool,
    },
    CredentialSubmit,
    CredentialCancel,
    SystemPage {
        page: tarot_core::SystemPage,
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

struct Message {
    request: Request,
    response: mpsc::SyncSender<Value>,
}

pub struct ControlServer {
    path: PathBuf,
    inode: u64,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

pub fn start(path: &Path, handle: &LoopHandle<'_, CalloopData>) -> Result<ControlServer> {
    let parent = path
        .parent()
        .context("control socket needs a parent directory")?;
    let metadata = fs::metadata(parent).context("control socket directory must exist")?;
    anyhow::ensure!(
        metadata.uid() == rustix::process::getuid().as_raw() && metadata.mode() & 0o077 == 0,
        "control socket requires a private directory owned by the current user"
    );
    let listener = UnixListener::bind(path).context("binding development control socket")?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let inode = fs::symlink_metadata(path)?.ino();
    let (sender, receiver) = channel::channel::<Message>();
    handle
        .insert_source(receiver, |event, _, data| {
            let channel::Event::Msg(message) = event else {
                return;
            };
            let state = &mut data.state;
            if state.protected() {
                let response=match message.request {
                    Request::Lock=>json!({"ok":true}),
                    Request::Rotate { orientation } => { state.request_orientation(orientation); json!({"ok":true}) },
                    Request::Status=>json!({"ok":true,"pid":std::process::id(),"lock":state.lock_status()}),
                    Request::Exit=>{state.loop_signal.stop();json!({"ok":true})},
                    Request::RestartLocker=>match state.restart_locker() {Ok(())=>json!({"ok":true}),Err(e)=>json!({"ok":false,"error":e.to_string()})},
                    _=>json!({"ok":false,"error":"Session is locked"}),
                };
                let _=message.response.send(response);return;
            }
            let include_apps = matches!(&message.request, Request::Applications);
            match message.request {
                Request::IdleTimeouts { settings } => {
                    if !settings.valid() || !state.shell.display.idle_configurable {
                        let _ = message.response.send(json!({"ok":false,"error":"Invalid or managed idle settings"}));
                        return;
                    }
                    state.apply_command(Command::IdleTimeouts(settings));
                },
                Request::Lock => {let response=match state.request_lock() {Ok(())=>json!({"ok":true}),Err(error)=>json!({"ok":false,"error":error.to_string()})};let _=message.response.send(response);return;},
                Request::Status | Request::Applications => {}
                Request::Rotate { orientation } => state.request_orientation(orientation),
                Request::RotationLock { locked } => state.apply_command(Command::RotationLock(locked)),
                Request::RestartLocker => {},
                Request::Home => state.apply_command(Command::Home),
                Request::Overview => state.apply_command(Command::Overview),
                Request::Launcher => state.apply_command(Command::Launcher),
                Request::Notifications {open} => state.apply_command(Command::NotificationsOpen(open)),
                Request::NotificationAction {action} => state.apply_command(Command::Notification(action)),
                Request::SystemMenu {open} => state.apply_command(Command::SystemMenuOpen(open)),
                Request::PowerAction {action} => state.apply_command(Command::Power(action)),
                Request::NetworkAction {action} => state.apply_command(Command::Network(action)),
                Request::AudioAction {action} =>state.apply_command(Command::Audio(action)),
                Request::NetworkSetup {network} =>state.apply_command(Command::NetworkSetup(network)),
                Request::CredentialText {value}=>state.apply_command(Command::CredentialText(value)),
                Request::CredentialBackspace=>state.apply_command(Command::CredentialBackspace),
                Request::CredentialRemember {remember}=>state.apply_command(Command::CredentialRemember(remember)),
                Request::CredentialSubmit=>state.apply_command(Command::CredentialSubmit),
                Request::CredentialCancel=>state.apply_command(Command::CredentialCancel),
                Request::SystemPage {page} => state.apply_command(Command::SystemPage(page)),
                Request::Launch { id } => state.apply_command(Command::Launch(id)),
                Request::Activate { id } => state.apply_command(Command::Activate(WindowId(id))),
                Request::Close { id } => state.apply_command(Command::Close(WindowId(id))),
                Request::Capture { path } => {
                    state.snapshot_error = None;
                    state.snapshot = Some(path);
                    state.snapshot_after = Duration::ZERO;
                    state.redraw = true;
                }
                Request::Exit => state.loop_signal.stop(),
            }
            let credential_input=state.shell.network_input.as_ref().map(|i|json!({
                "target":match i.target{tarot_core::network::InputTarget::New(_)=>"New",tarot_core::network::InputTarget::Agent(_)=>"Agent"},
                "id":match i.target{tarot_core::network::InputTarget::New(id)|tarot_core::network::InputTarget::Agent(id)=>id},
                "valid":i.kind.accepts(&i.password),"has_password":!i.password.expose().is_empty(),"keyboard":i.keyboard,"remember":i.remember
            }));
            let network=json!({
                    "enabled":state.shell.network.enabled,"available":state.shell.network.available,
                    "generation":state.shell.network.generation,"epoch":state.shell.network.epoch,
                    "radio":state.shell.network.radio,"hardware":state.shell.network.hardware,
                    "state":state.shell.network.state,"state_reason":state.shell.network.state_reason,
                    "connectivity":state.shell.network.connectivity,"last_scan":state.shell.network.last_scan,
                    "scan_permission":state.shell.network.scan_permission,"control_permission":state.shell.network.control_permission,
                    "create_permission":state.shell.network.create_permission,"agent_available":state.shell.network.agent_available,
                    "secret_prompt":state.shell.network.secret_prompt.as_ref().map(|p|json!({"id":p.id,"kind":p.kind})),
                    "radio_permission":state.shell.network.radio_permission,"busy":state.shell.network.busy,
                    "pending":state.shell.network.pending,"active_profile":state.shell.network.active_profile,
                    "error":state.shell.network.error,"action_error":state.shell.network.action_error,
                    "networks":state.shell.network.networks.iter().map(|n|json!({"id":n.id,"security":n.security,"strength":n.strength,"connected":n.connected,"profiles":n.profiles.iter().map(|p|p.id).collect::<Vec<_>>()})).collect::<Vec<_>>(),
                });
            let mut response = json!({
                "ok": true,
                "pid": std::process::id(),
                "output": state.output_geometry,
                "sensors": state.shell.sensors,
                "rotation_locked": state.shell.rotation_locked,
                "display_settings": state.shell.display,
                "wayland_display": state.socket_name.to_string_lossy(),
                "view": state.shell.view,
                "popup_grab_active": state.popup_grab.is_some(),
                "popup_owner": state.popup_owner,
                "keyboard_visible": state.keyboard_visible(),
                "keyboard_owner": state.keyboard.as_ref().and_then(|keyboard| keyboard.owner),
                "input_method": state.input_method.as_ref().map(|managed| managed.status()),
                "shell_text_focus": state.shell.shell_text_focus,
                "card_menu_open": state.shell.card_menu_open,
                "system_menu_open": state.shell.system_menu_open,
                "lock":state.lock_status(),
                "system_page": state.shell.system_page,
                "credential_input":credential_input,
                "network":network,
                "power": state.shell.power,
                "audio": {"enabled":state.shell.audio.enabled,"available":state.shell.audio.available,"generation":state.shell.audio.generation,"epoch":state.shell.audio.epoch,"pending":state.shell.audio.pending,"error":state.shell.audio.error,"action_error":state.shell.audio.action_error,"outputs":state.shell.audio.outputs.iter().map(|o|json!({"id":o.id,"percent":o.percent,"muted":o.muted,"default":o.default})).collect::<Vec<_>>()},
                "notifications": {
                    "available":state.shell.notifications.available,
                    "generation":state.shell.notifications.generation,
                    "dashboard_open":state.shell.notifications_open,
                    "dnd":state.shell.notifications.dnd,
                    "hide_previews":state.shell.notifications.hide_previews,
                    "banner":state.shell.notifications.banner,
                    "error":state.shell.notifications.error,
                    "settings_error":state.shell.notifications.settings_error,
                    "settings_saved":state.shell.notifications.settings_saved,
                    "settings_available":state.shell.notifications.settings_available,
                    "items":state.shell.notifications.items.iter().map(|n|json!({"id":n.id,"revision":n.revision,"live":n.live,"orphaned":n.orphaned,"action_count":n.actions.len(),"urgent":n.urgent})).collect::<Vec<_>>(),
                },
                "launcher_query": state.shell.launcher_query,
                "keyboard_rect": state.layout.keyboard(),
                "app_area": state.layout.content(),
                "selected": state.shell.selected,
                "presentation": state.presentation,
                "cards": state.shell.cards,
                "native_windows": state.families.nodes(),
                "capture_pending": state.snapshot.is_some(),
                "capture_error": state.snapshot_error,
                "render_metrics": state.metrics,
                "application_count": state.shell.applications.len(),
                "dmabuf_enabled": state.dmabuf_global.is_some(),
                "launch_feedback": state.shell.launch_feedback,
            });
            response["native_geometry"] = json!(state.space.elements().filter_map(|window| {
                let id = window.user_data().get::<tarot_core::WindowId>()?;
                let geometry = window.geometry();
                Some(json!({"id":id,"fullscreen":state.fullscreen_window(*id),
                    "geometry":{"x":geometry.loc.x,"y":geometry.loc.y,"width":geometry.size.w,"height":geometry.size.h}}))
            }).collect::<Vec<_>>());
            response["hardware_keys"] = json!(state.hardware.enabled);
            response["volume_feedback"] = json!(state.shell.volume_feedback);
            response["favorites"] = json!(state.shell.favorites);
            response["clock"] = json!(state.shell.clock_label);
            response["calendar_day"] = json!(state.shell.calendar_day);
            response["editing_favorites"] = json!(state.shell.editing_favorites);
            if include_apps {
                response["applications"] = json!(state.shell.applications);
            }
            let _ = message.response.send(response);
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;

    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker = thread::Builder::new()
        .name("tarot-control".into())
        .spawn(move || {
            for incoming in listener.incoming() {
                if worker_stop.load(Ordering::Acquire) {
                    break;
                }
                let Ok(mut stream) = incoming else {
                    break;
                };
                let result = exchange(&mut stream, &sender);
                if let Err(error) = result {
                    let _ = writeln!(
                        stream,
                        "{}",
                        json!({"ok": false, "error": error.to_string()})
                    );
                }
            }
        })?;
    Ok(ControlServer {
        path: path.into(),
        inode,
        stop,
        worker: Some(worker),
    })
}

fn exchange(stream: &mut UnixStream, sender: &channel::Sender<Message>) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    stream.set_write_timeout(Some(Duration::from_secs(1)))?;
    let mut line = zeroize::Zeroizing::new(String::new());
    BufReader::new(stream.try_clone()?)
        .take(4097)
        .read_line(&mut line)?;
    anyhow::ensure!(
        line.len() <= 4096 && line.ends_with('\n'),
        "request must be one JSON line of at most 4096 bytes"
    );
    let request = serde_json::from_str(&line)?;
    let (response, receiver) = mpsc::sync_channel(1);
    sender
        .send(Message { request, response })
        .context("compositor control channel closed")?;
    let response = receiver
        .recv_timeout(Duration::from_secs(2))
        .context("compositor response timed out")?;
    serde_json::to_writer(&mut *stream, &response)?;
    stream.write_all(b"\n")?;
    Ok(())
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = UnixStream::connect(&self.path);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        if fs::symlink_metadata(&self.path).is_ok_and(|metadata| metadata.ino() == self.inode) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
