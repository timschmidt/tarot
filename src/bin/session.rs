//! Installed, unprivileged session entry. PAM/seat ownership belongs to the
//! display manager and elogind; this launcher never creates a login session.
use anyhow::{Context, Result, ensure};
use clap::{Parser, ValueEnum};
use serde::Serialize;
use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::{
        fs::{FileTypeExt, MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use zbus::{
    blocking::{Connection, Proxy, connection::Builder},
    zvariant::OwnedObjectPath,
};

#[derive(Parser)]
#[command(
    version,
    about = "Start Tarot in an authenticated local Wayland session"
)]
struct Options {
    /// Audio provider for this preview; external leaves startup to the platform.
    #[arg(long, value_enum, default_value = "pulse-audio")]
    audio_service: AudioService,
    /// Enable the private development control socket; never enables unlock.
    #[arg(long)]
    diagnostics: bool,
    /// Qualify authenticated-session idle reporting through diagnostics only.
    #[arg(long, requires = "diagnostics")]
    session_idle: bool,
    /// Permit the idle experiment on observed external power.
    #[arg(long, requires = "session_idle")]
    session_idle_on_external_power: bool,
    /// Inspect local session/device prerequisites without starting graphics.
    #[arg(long)]
    check_session: bool,
    #[arg(long, hide = true, conflicts_with = "check_session")]
    inside_session_bus: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum, Serialize)]
enum AudioService {
    PulseAudio,
    External,
}
impl AudioService {
    fn argument(self) -> &'static str {
        match self {
            Self::PulseAudio => "pulse-audio",
            Self::External => "external",
        }
    }
}

fn start_audio(provider: AudioService, log: &mut std::fs::File) -> Result<()> {
    if matches!(provider, AudioService::External) {
        writeln!(
            log,
            "{}",
            serde_json::json!({"event":"audio-service","provider":provider,"managed":false})
        )?;
        return Ok(());
    }
    trusted_program(Path::new("/usr/bin/pulseaudio"))?;
    let mut child = Command::new("/usr/bin/pulseaudio")
        .args(["--start", "--log-target=stderr"])
        .env_remove("DISPLAY")
        .env_remove("PULSE_SERVER")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log.try_clone()?)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.try_wait()? {
            writeln!(
                log,
                "{}",
                serde_json::json!({"event":"audio-service","provider":provider,"code":status.code(),"success":status.success()})
            )?;
            ensure!(status.success(), "User audio daemon startup failed");
            return Ok(());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("User audio daemon startup exceeded five seconds");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[derive(Debug, Serialize)]
struct Session {
    id: String,
    uid: u32,
    seat: String,
    vt: u32,
    active: bool,
    remote: bool,
    class: String,
    kind: String,
    #[serde(skip)]
    path: OwnedObjectPath,
    #[serde(skip)]
    owner: String,
}
impl Session {
    fn validate(&self, uid: u32) -> Result<()> {
        ensure!(
            uid != 0 && self.uid == uid,
            "An ordinary current-user PAM session is required"
        );
        ensure!(
            self.active && !self.remote && self.class == "user",
            "An active local user session is required"
        );
        ensure!(
            self.kind == "wayland" && self.seat == "seat0" && self.vt > 0,
            "A local seat0 Wayland VT session is required"
        );
        Ok(())
    }
}

fn current_session(connection: &Connection) -> Result<Session> {
    let bus = Proxy::new(
        connection,
        "org.freedesktop.DBus",
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
    )?;
    let owner: String = bus.call("GetNameOwner", &("org.freedesktop.login1",))?;
    let manager_uid: u32 = bus.call("GetConnectionUnixUser", &(owner.as_str(),))?;
    ensure!(manager_uid == 0, "Expected the system login manager");
    let manager = Proxy::new(
        connection,
        owner.as_str(),
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )?;
    let path: OwnedObjectPath = manager
        .call("GetSessionByPID", &(std::process::id(),))
        .context("No PAM login session for this process; launch Tarot from the login screen")?;
    let proxy = Proxy::new(
        connection,
        owner.as_str(),
        path.as_str(),
        "org.freedesktop.login1.Session",
    )?;
    let (uid, _): (u32, OwnedObjectPath) = proxy.get_property("User")?;
    let (seat, _): (String, OwnedObjectPath) = proxy.get_property("Seat")?;
    let session = Session {
        id: proxy.get_property("Id")?,
        uid,
        seat,
        vt: proxy.get_property("VTNr")?,
        active: proxy.get_property("Active")?,
        remote: proxy.get_property("Remote")?,
        class: proxy.get_property("Class")?,
        kind: proxy.get_property("Type")?,
        path: path.clone(),
        owner: owner.clone(),
    };
    session.validate(rustix::process::getuid().as_raw())?;
    Ok(session)
}

fn panel_card(root: &Path) -> Result<PathBuf> {
    let mut cards = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some((card, connector)) = name.to_str().and_then(|name| name.split_once("-DSI-"))
        else {
            continue;
        };
        if card
            .strip_prefix("card")
            .is_none_or(|number| number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()))
            || connector.is_empty()
            || !connector.bytes().all(|b| b.is_ascii_digit())
        {
            continue;
        }
        if std::fs::read_to_string(entry.path().join("status"))
            .is_ok_and(|s| s.trim() == "connected")
        {
            cards.push(Path::new("/dev/dri").join(card));
        }
    }
    ensure!(
        cards.len() == 1,
        "Expected exactly one connected DSI panel, found {}",
        cards.len()
    );
    Ok(cards.remove(0))
}

fn private_directory(path: &Path, uid: u32) -> Result<()> {
    ensure!(path.is_absolute(), "Session directory must be absolute");
    std::fs::create_dir_all(path)?;
    let metadata = std::fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir() && metadata.uid() == uid && metadata.mode() & 0o077 == 0,
        "Session directory must be a private, current-user directory: {}",
        path.display()
    );
    Ok(())
}

fn trusted_program(path: &Path) -> Result<()> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("Missing installed program {}", path.display()))?;
    ensure!(
        metadata.is_file()
            && metadata.uid() == 0
            && metadata.mode() & 0o6022 == 0
            && metadata.mode() & 0o111 != 0,
        "Session programs must be root-owned executables without set-ID or writable group/other permissions"
    );
    Ok(())
}

// The caller holds this user's exclusive session lock. Remove only the socket
// entry in that private directory, never a substituted file or symlink target.
fn remove_diagnostic_socket(path: &Path, uid: u32) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.file_type().is_socket() && metadata.uid() == uid,
                "Unexpected diagnostic socket entry"
            );
            std::fs::remove_file(path)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn compositor_arguments(
    card: &Path,
    control: Option<&Path>,
    session_idle: bool,
    on_external_power: bool,
) -> Vec<std::ffi::OsString> {
    let mut args = vec![
        "--drm-device".into(),
        card.as_os_str().into(),
        "--scale".into(),
        "2".into(),
        "--publish-session-environment".into(),
        "--notifications".into(),
        "--system-services".into(),
        "--sleep-coordination".into(),
        "--lock-client".into(),
        "/usr/bin/tarot-lock".into(),
        "--input-method".into(),
        "/usr/bin/tarot-keyboard".into(),
        "--idle-preferences".into(),
        "--hardware-keys".into(),
    ];
    if let Some(path) = control {
        args.extend(["--control-socket".into(), path.as_os_str().into()]);
        args.extend(["--input-method-arg".into(), "--metadata".into()]);
        if session_idle {
            args.push("--session-idle".into());
            if on_external_power {
                args.push("--session-idle-on-external-power".into());
            }
        }
    }
    args
}

fn run(options: Options) -> Result<()> {
    let uid = rustix::process::getuid();
    ensure!(
        !uid.is_root()
            && uid == rustix::process::geteuid()
            && rustix::process::getgid() == rustix::process::getegid(),
        "Tarot sessions refuse root and set-ID execution"
    );
    let connection = Builder::system()?
        .method_timeout(Duration::from_secs(3))
        .build()?;
    let session = current_session(&connection)?;
    let runtime = PathBuf::from(
        std::env::var_os("XDG_RUNTIME_DIR").context("PAM must provide XDG_RUNTIME_DIR")?,
    );
    let expected_runtime = format!("/run/user/{}", uid.as_raw());
    ensure!(
        runtime == Path::new(&expected_runtime),
        "Unexpected PAM runtime directory"
    );
    let metadata = std::fs::symlink_metadata(&runtime)?;
    ensure!(
        metadata.is_dir() && metadata.uid() == uid.as_raw() && metadata.mode() & 0o077 == 0,
        "Invalid PAM runtime ownership/permissions"
    );
    let card = panel_card(Path::new("/sys/class/drm"))?;
    for program in [
        "/usr/bin/tarot",
        "/usr/bin/tarot-lock",
        "/usr/bin/tarot-keyboard",
        "/usr/bin/tarot-session",
        "/usr/bin/dbus-run-session",
        "/usr/libexec/tarot-dbus-daemon",
    ] {
        trusted_program(Path::new(program))?;
    }
    if options.check_session {
        println!(
            "{}",
            serde_json::json!({"session":session,"card":card,"runtime":runtime,"graphics_started":false})
        );
        return Ok(());
    }
    if !options.inside_session_bus {
        // A new bus belongs to this display session, never a previous greeter or
        // same-UID desktop. Its supervisor follows this session's lifetime.
        let mut command = Command::new("/usr/bin/dbus-run-session");
        command.arg("--dbus-daemon=/usr/libexec/tarot-dbus-daemon");
        command.args(["--", "/usr/bin/tarot-session", "--inside-session-bus"]);
        command.args(["--audio-service", options.audio_service.argument()]);
        if options.diagnostics {
            command.arg("--diagnostics");
        }
        if options.session_idle {
            command.arg("--session-idle");
        }
        if options.session_idle_on_external_power {
            command.arg("--session-idle-on-external-power");
        }
        command
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("DBUS_SESSION_BUS_PID");
        return Err(command.exec()).context("starting the private session bus");
    }
    ensure!(
        std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some(),
        "Private session bus unavailable"
    );
    rustix::process::umask(rustix::fs::Mode::from_raw_mode(0o077));
    let directory = runtime.join("tarot-session");
    private_directory(&directory, uid.as_raw())?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
        .open(directory.join("session.lock"))?;
    ensure!(
        lock.metadata()?.uid() == uid.as_raw(),
        "Unexpected session lock owner"
    );
    rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .context("Another Tarot session is already running for this user")?;
    let control_path = directory.join("control.sock");
    remove_diagnostic_socket(&control_path, uid.as_raw())?;
    let control = options.diagnostics.then_some(control_path.clone());
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state")
        });
    let logs = state.join("tarot/sessions");
    private_directory(&logs, uid.as_raw())?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let mut log = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(logs.join(format!("session-{stamp}-{}.log", std::process::id())))?;
    writeln!(
        log,
        "{}",
        serde_json::json!({"event":"session-start","session":session,"card":card,"diagnostics":options.diagnostics,"session_idle":options.session_idle,"session_idle_on_external_power":options.session_idle_on_external_power})
    )?;
    let mut command = Command::new("/usr/bin/tarot");
    command
        .args(compositor_arguments(&card, control.as_deref(), options.session_idle, options.session_idle_on_external_power))
        .env("LIBSEAT_BACKEND", "logind")
        .env("XDG_CURRENT_DESKTOP", "Tarot")
        .env("XDG_SESSION_DESKTOP", "tarot")
        .env("XDG_SESSION_TYPE", "wayland")
        .env("GDK_BACKEND", "wayland")
        .env("QT_QPA_PLATFORM", "wayland")
        .env("RUST_LOG", if options.diagnostics {
            "tarot=info,smithay=warn,tarot::input::metadata=debug,smithay::wayland::text_input::metadata=debug"
        } else {
            "tarot=info,smithay=warn"
        })
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("SEATD_SOCK")
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log.try_clone()?)
        .process_group(0);
    if matches!(options.audio_service, AudioService::PulseAudio) {
        command.env_remove("PULSE_SERVER");
    }
    let parent = rustix::process::getpid();
    // SAFETY: only async-signal-safe prctl/getppid syscalls execute after fork.
    unsafe {
        command.pre_exec(move || {
            rustix::process::set_parent_process_death_signal(Some(rustix::process::Signal::TERM))?;
            if rustix::process::getppid() != Some(parent) {
                return Err(std::io::Error::from_raw_os_error(10));
            }
            Ok(())
        });
    }
    let mut child = command
        .spawn()
        .context("starting the installed compositor")?;
    let child_pid =
        rustix::process::Pid::from_raw(child.id() as i32).context("invalid compositor PID")?;
    writeln!(
        log,
        "{}",
        serde_json::json!({"event":"compositor-start","pid":child.id()})
    )?;
    // Start once per login, alongside protected graphics startup. Adapter
    // reconnects never spawn daemons. A failed provider remains visible as an
    // unavailable audio service; it must not prevent session protection.
    if let Err(error) = start_audio(options.audio_service, &mut log) {
        writeln!(
            log,
            "{}",
            serde_json::json!({"event":"audio-service-error","error":error.to_string()})
        )?;
    }
    // Leave the dead leader unreaped until its process group has been cleaned:
    // the PID cannot be reused for an unrelated group during these signals.
    let ended = loop {
        match rustix::process::waitid(
            rustix::process::WaitId::Pid(child_pid),
            rustix::process::WaitIdOptions::EXITED | rustix::process::WaitIdOptions::NOWAIT,
        ) {
            Ok(Some(status)) => break status,
            Ok(None) => continue,
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
    };
    let _ = writeln!(
        log,
        "{}",
        serde_json::json!({"event":"compositor-exit","code":ended.exit_status(),"signal":ended.terminating_signal()})
    );
    let _ = rustix::process::kill_process_group(child_pid, rustix::process::Signal::TERM);
    std::thread::sleep(Duration::from_millis(100));
    let _ = rustix::process::kill_process_group(child_pid, rustix::process::Signal::KILL);
    let status = child.wait()?;
    // SIGKILL/SIGTERM bypass the compositor's socket destructor. Clean its
    // diagnostic entry before Terminate can kill this supervisor as well.
    if let Err(error) = remove_diagnostic_socket(&control_path, uid.as_raw()) {
        let _ = writeln!(
            log,
            "{}",
            serde_json::json!({"event":"diagnostic-cleanup-error","error":error.to_string()})
        );
    }
    // Kill only this verified PAM session, including its private bus and apps.
    // Successful Terminate may kill this supervisor before the reply arrives;
    // greetd observes session exit and returns to its authenticated greeter.
    let _ = writeln!(
        log,
        "{}",
        serde_json::json!({"event":"session-stop-request","compositor_success":status.success()})
    );
    let _ = log.flush();
    let proxy = Proxy::new(
        &connection,
        session.owner.as_str(),
        session.path.as_str(),
        "org.freedesktop.login1.Session",
    )?;
    proxy
        .call::<_, _, ()>("Terminate", &())
        .context("terminating this login session after compositor exit")?;
    ensure!(
        status.success(),
        "Compositor exited unsuccessfully; returned to login screen"
    );
    Ok(())
}

fn main() -> ExitCode {
    match run(Options::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Tarot session: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    #[test]
    fn diagnostic_cleanup_preserves_wrong_owner_files_and_symlinks() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("control.sock");
        let uid = rustix::process::getuid().as_raw();
        let listener = std::os::unix::net::UnixListener::bind(&path).unwrap();
        assert!(remove_diagnostic_socket(&path, uid.wrapping_add(1)).is_err());
        assert!(path.exists());
        remove_diagnostic_socket(&path, uid).unwrap();
        assert!(!path.exists());
        drop(listener);
        remove_diagnostic_socket(&path, uid).unwrap();
        std::fs::write(&path, "preserve").unwrap();
        assert!(remove_diagnostic_socket(&path, uid).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "preserve");
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(&path, &link).unwrap();
        assert!(remove_diagnostic_socket(&link, uid).is_err());
        assert!(link.is_symlink() && path.exists());
    }
    fn session() -> Session {
        Session {
            id: "c1".into(),
            uid: 10000,
            seat: "seat0".into(),
            vt: 7,
            active: true,
            remote: false,
            class: "user".into(),
            kind: "wayland".into(),
            path: OwnedObjectPath::try_from("/session/c1").unwrap(),
            owner: ":1.2".into(),
        }
    }
    #[test]
    fn rejects_remote_inactive_greeter_wrong_user_and_non_wayland() {
        assert!(session().validate(10000).is_ok());
        for change in 0..7 {
            let mut s = session();
            match change {
                0 => s.remote = true,
                1 => s.active = false,
                2 => s.class = "greeter".into(),
                3 => s.uid = 0,
                4 => s.kind = "tty".into(),
                5 => s.seat.clear(),
                _ => s.vt = 0,
            }
            assert!(s.validate(10000).is_err());
        }
        assert!(session().validate(10001).is_err());
        assert!(session().validate(0).is_err());
    }
    #[test]
    fn selects_connected_panel_and_rejects_ambiguous_panels() {
        let root = tempfile::tempdir().unwrap();
        for (name, state) in [
            ("card0-HDMI-A-1", "connected"),
            ("card1-DSI-1", "connected"),
            ("card2-DSI-1", "disconnected"),
        ] {
            let p = root.path().join(name);
            std::fs::create_dir(&p).unwrap();
            std::fs::write(p.join("status"), state).unwrap();
        }
        assert_eq!(
            panel_card(root.path()).unwrap(),
            PathBuf::from("/dev/dri/card1")
        );
        std::fs::write(root.path().join("card2-DSI-1/status"), "connected").unwrap();
        assert!(panel_card(root.path()).is_err());
        std::fs::write(root.path().join("card1-DSI-1/status"), "disconnected").unwrap();
        std::fs::write(root.path().join("card2-DSI-1/status"), "disconnected").unwrap();
        assert!(panel_card(root.path()).is_err());
    }
    #[test]
    fn installed_session_always_has_managed_lock_and_no_test_escape() {
        let args = compositor_arguments(Path::new("/dev/dri/card1"), None, false, false);
        assert!(args.contains(&OsStr::new("--sleep-coordination").to_owned()));
        assert!(args.contains(&OsStr::new("--hardware-keys").to_owned()));
        assert!(
            args.windows(2)
                .any(|p| p == ["--lock-client", "/usr/bin/tarot-lock"])
        );
        for escape in [
            "--lock-probe-unlocked-start",
            "--exit-after-ms",
            "--control-socket",
            "--screenshot",
            "--platform-bus",
            "--input-method-arg",
            "--session-idle",
            "--session-idle-on-external-power",
        ] {
            assert!(!args.contains(&OsStr::new(escape).to_owned()));
        }
        let diagnostics = compositor_arguments(
            Path::new("/dev/dri/card1"),
            Some(Path::new("/tmp/control.sock")),
            false,
            false,
        );
        assert!(
            diagnostics
                .windows(2)
                .any(|p| p == ["--input-method-arg", "--metadata"])
        );
    }
    #[test]
    fn idle_experiment_requires_explicit_diagnostics_and_power_policy() {
        assert!(Options::try_parse_from(["tarot-session", "--session-idle"]).is_err());
        assert!(
            Options::try_parse_from([
                "tarot-session",
                "--diagnostics",
                "--session-idle-on-external-power"
            ])
            .is_err()
        );
        let options = Options::try_parse_from([
            "tarot-session",
            "--diagnostics",
            "--session-idle",
            "--session-idle-on-external-power",
        ])
        .unwrap();
        assert!(
            options.diagnostics && options.session_idle && options.session_idle_on_external_power
        );
        let diagnostic = compositor_arguments(
            Path::new("/dev/dri/card1"),
            Some(Path::new("/tmp/control.sock")),
            true,
            true,
        );
        assert!(diagnostic.iter().any(|v| v == "--session-idle"));
        assert!(
            diagnostic
                .iter()
                .any(|v| v == "--session-idle-on-external-power")
        );
        let normal = compositor_arguments(Path::new("/dev/dri/card1"), None, true, true);
        assert!(
            !normal
                .iter()
                .any(|v| v == "--session-idle" || v == "--session-idle-on-external-power")
        );
    }
}
