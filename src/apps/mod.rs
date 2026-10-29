//! Installed applications. XDG precedence is applied before visibility filters.

mod exec;
pub(crate) mod uri;

use anyhow::{Context, Result, anyhow};
use freedesktop_desktop_entry as desktop;
use std::{
    collections::{BTreeMap, HashSet},
    ffi::OsStr,
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Child, Command},
};
use tarot_core::Application;

#[derive(Clone, Copy)]
pub enum ChildTag {
    Application(u64),
    InputMethod(u64),
    Locker(u64),
}

pub enum AppEvent {
    OpenUri {
        generation: u64,
        search_revision: u64,
        uri: String,
    },
    UriFinished {
        generation: u64,
        search_revision: u64,
        handler: Option<String>,
        success: bool,
    },
    Launch {
        id: String,
        generation: u64,
    },
    Exited {
        tag: ChildTag,
        result: std::result::Result<std::process::ExitStatus, String>,
    },
}

/// Called only for a session whose private bus was explicitly selected by its launcher.
pub fn publish_activation_environment(socket: &OsStr) -> Result<()> {
    let connection = zbus::blocking::connection::Builder::session()?
        .method_timeout(std::time::Duration::from_secs(3))
        .build()?;
    let proxy = zbus::blocking::fdo::DBusProxy::new(&connection)?;
    proxy.update_activation_environment(activation_environment(socket)?)?;
    Ok(())
}

pub fn activation_environment(
    socket: &OsStr,
) -> Result<std::collections::HashMap<&'static str, &str>> {
    let socket = socket
        .to_str()
        .context("Wayland socket name is not UTF-8")?;
    Ok(std::collections::HashMap::from([
        ("WAYLAND_DISPLAY", socket),
        ("DISPLAY", ""),
        ("XDG_CURRENT_DESKTOP", "Tarot"),
        ("XDG_SESSION_DESKTOP", "tarot"),
        ("XDG_SESSION_TYPE", "wayland"),
        ("GDK_BACKEND", "wayland"),
        ("QT_QPA_PLATFORM", "wayland"),
    ]))
}

pub fn init(
    handle: smithay::reexports::calloop::LoopHandle<'static, crate::CalloopData>,
    state: &mut crate::Compositor,
) -> Result<()> {
    use smithay::reexports::calloop::{
        channel,
        timer::{TimeoutAction, Timer},
    };
    let registry = Registry::from_environment();
    state.shell.applications = registry.applications.clone();
    state.shell.launcher.settings = state
        .preferences
        .as_ref()
        .and_then(|s| s.initial.launcher.clone())
        .unwrap_or_default();
    state.shell.favorites = state
        .preferences
        .as_ref()
        .and_then(|server| server.initial.quick_launch.clone())
        .unwrap_or_else(|| tarot_core::quick_launch::Favorites::initial(&state.shell.applications));
    let (sender, receiver) = channel::channel::<AppEvent>();
    state.app_events = Some(sender.clone());
    let uri_requests = uri::start(sender.clone());
    let loop_handle = handle.clone();
    handle.insert_source(receiver, move |event, _, data| {
        let channel::Event::Msg(event) = event else { return; };
        match event {
            AppEvent::OpenUri { generation, search_revision, uri } => {
                if data.state.protected() {
                    launch_failed(&mut data.state, generation, "Browser request canceled while the session was protected.".into());
                    return;
                }
                if uri_requests.try_send(uri::Request { generation, search_revision, uri, display: data.state.socket_name.clone() }).is_err() {
                    launch_failed(&mut data.state, generation, "Another browser request is in progress. Try again.".into());
                }
            }
            AppEvent::UriFinished { generation, search_revision, handler, success } => {
                let current = data.state.shell.launch_feedback.as_ref().is_some_and(|f| f.generation == generation && f.pending);
                if !current { return; }
                launch_failed(&mut data.state, generation, if success { "Request handed to the default browser." } else { "Unable to open the default browser. Check its installation and default-app setting, then try again." }.into());
                if success && !data.state.protected() && data.state.shell.just_type_active() && data.state.shell.search.revision == search_revision
                    && !data.state.shell.system_menu_open && !data.state.shell.notifications_open
                    && let Some(handler) = handler {
                    let app_id = handler.strip_suffix(".desktop").unwrap_or(&handler);
                    if let Some(id) = data.state.shell.cards.iter().rev().find(|c| !c.closing && (c.app_id == app_id || c.app_id == handler)).map(|c| c.id) {
                        data.state.apply_command(tarot_core::Command::Activate(id));
                        data.state.shell.launch_feedback = None;
                    }
                }
            }
            AppEvent::Launch { id, generation } => {
                match registry.launch(&id, &data.state.socket_name) {
                    Ok(child) => monitor_child(loop_handle.clone(), child, ChildTag::Application(generation), sender.clone()),
                    Err(error) => launch_failed(&mut data.state, generation, format!("{error:#}")),
                }
                if let Err(error) = loop_handle.insert_source(Timer::from_duration(std::time::Duration::from_secs(20)), move |_, _, data| {
                    launch_failed(&mut data.state, generation, "The application has not opened a window. It may still be starting.".into());
                    TimeoutAction::Drop
                }) { tracing::warn!(%error, "could not schedule application startup check"); }
            }
            AppEvent::Exited { tag: ChildTag::Locker(generation), .. } => {crate::lock::exited(&mut data.state,generation);}
            AppEvent::Exited { tag: ChildTag::InputMethod(generation), result } => {
                crate::input_method_process::exited(&mut data.state, generation, result);
            }
            AppEvent::Exited { tag: ChildTag::Application(generation), result } => {
                match result {
                    Ok(status) if status.success() => {},
                    Ok(status) => launch_failed(&mut data.state, generation, format!("The application exited: {status}")),
                    Err(error) => launch_failed(&mut data.state, generation, error),
                }
            }
        }
    }).map_err(|error| anyhow!(error.to_string()))?;
    Ok(())
}

fn launch_failed(state: &mut crate::Compositor, generation: u64, message: String) {
    if let Some(feedback) = &mut state.shell.launch_feedback
        && feedback.generation == generation
        && feedback.pending
    {
        feedback.pending = false;
        feedback.message = message;
        state.ui_dirty = true;
        state.redraw = true;
    }
}

/// pidfd readiness reaps children without polling an idle compositor.
pub fn monitor_child(
    handle: smithay::reexports::calloop::LoopHandle<'static, crate::CalloopData>,
    child: Child,
    tag: ChildTag,
    sender: smithay::reexports::calloop::channel::Sender<AppEvent>,
) {
    use rustix::process::{Pid, PidfdFlags, pidfd_open};
    use smithay::reexports::calloop::{Interest, Mode, PostAction, generic::Generic};
    let pidfd =
        Pid::from_raw(child.id() as i32).and_then(|pid| pidfd_open(pid, PidfdFlags::empty()).ok());
    let child = std::sync::Arc::new(std::sync::Mutex::new(child));
    if let Some(pidfd) = pidfd {
        let monitored = child.clone();
        let events = sender.clone();
        let result = handle.insert_source(
            Generic::new(pidfd, Interest::READ, Mode::Level),
            move |_, _, _| {
                let result = monitored
                    .lock()
                    .map_err(|_| std::io::Error::other("child monitor poisoned"))?
                    .try_wait();
                match result {
                    Ok(None) => return Ok(PostAction::Continue),
                    Ok(Some(status)) => {
                        let _ = events.send(AppEvent::Exited {
                            tag,
                            result: Ok(status),
                        });
                    }
                    Err(error) => {
                        let _ = events.send(AppEvent::Exited {
                            tag,
                            result: Err(error.to_string()),
                        });
                    }
                }
                Ok(PostAction::Remove)
            },
        );
        if result.is_ok() {
            return;
        }
        tracing::warn!("pidfd registration failed; using a child wait worker");
    }
    std::thread::spawn(move || {
        let result = child
            .lock()
            .map_err(|_| "child monitor poisoned".to_string())
            .and_then(|mut child| child.wait().map_err(|error| error.to_string()));
        let _ = sender.send(AppEvent::Exited { tag, result });
    });
}

#[derive(Clone, Debug)]
pub struct LaunchSpec {
    pub argv: Vec<String>,
    pub directory: Option<PathBuf>,
}

#[derive(Default)]
pub struct Registry {
    pub applications: Vec<Application>,
    specs: BTreeMap<String, std::result::Result<LaunchSpec, String>>,
}

impl Registry {
    pub fn from_environment() -> Self {
        let locales = desktop::get_languages_from_env();
        let search = std::env::var_os("PATH")
            .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
            .unwrap_or_default();
        Self::scan(desktop::default_paths(), &locales, &search)
    }

    pub fn scan(
        paths: impl IntoIterator<Item = PathBuf>,
        locales: &[String],
        executable_paths: &[PathBuf],
    ) -> Self {
        let mut registry = Self::default();
        let mut seen = HashSet::new();
        for root in paths {
            let mut files = Vec::new();
            walk(&root, 0, &mut files);
            files.sort();
            for path in files {
                let Ok(relative) = path.strip_prefix(&root) else {
                    continue;
                };
                let id = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("-");
                if !seen.insert(id.clone()) {
                    continue;
                }
                // Hidden/invalid higher-priority files must not reveal a lower-priority entry.
                if fs::metadata(&path).is_ok_and(|metadata| metadata.len() > 1024 * 1024) {
                    continue;
                }
                let Ok(entry) = desktop::DesktopEntry::from_path(&path, Some(locales)) else {
                    continue;
                };
                if entry.type_() != Some("Application") || entry.hidden() || entry.no_display() {
                    continue;
                }
                if entry
                    .only_show_in()
                    .is_some_and(|desktops| !desktops.contains(&"Tarot"))
                    || entry
                        .not_show_in()
                        .is_some_and(|desktops| desktops.contains(&"Tarot"))
                {
                    continue;
                }
                let Some(name) = entry
                    .name(locales)
                    .filter(|name| !name.trim().is_empty())
                    .map(|name| name.into_owned())
                else {
                    continue;
                };
                let icon = entry.icon().map(str::to_owned);
                let result =
                    launch_spec(&entry, &name, executable_paths).map_err(|error| error.to_string());
                registry.applications.push(Application {
                    id: id.clone(),
                    name,
                    description: entry
                        .comment(locales)
                        .map(|text| text.into_owned())
                        .unwrap_or_default(),
                    icon,
                    keywords: entry
                        .keywords(locales)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|value| value.into_owned())
                        .collect(),
                    unavailable: result.as_ref().err().cloned(),
                });
                registry.specs.insert(id, result);
            }
        }
        registry
            .applications
            .sort_by_cached_key(|app| (app.name.to_lowercase(), app.id.clone()));
        registry
    }

    pub fn launch(&self, id: &str, wayland_display: &OsStr) -> Result<Child> {
        let spec = self
            .specs
            .get(id)
            .ok_or_else(|| anyhow!("application is no longer installed"))?
            .as_ref()
            .map_err(|reason| anyhow!(reason.clone()))?;
        let (program, args) = spec.argv.split_first().context("empty launch command")?;
        let mut command = Command::new(program);
        command
            .args(args)
            .env("WAYLAND_DISPLAY", wayland_display)
            .env("GDK_BACKEND", "wayland")
            .env("QT_QPA_PLATFORM", "wayland")
            .env("XDG_SESSION_TYPE", "wayland")
            .env("XDG_CURRENT_DESKTOP", "Tarot")
            .env("XDG_SESSION_DESKTOP", "tarot")
            .env_remove("DISPLAY");
        if let Some(directory) = &spec.directory {
            command.current_dir(directory);
        } else if let Some(home) = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
        {
            command.current_dir(home);
        }
        command.stdin(std::process::Stdio::null());
        command.spawn().context("starting application")
    }
}

fn walk(root: &Path, depth: u8, files: &mut Vec<PathBuf>) {
    if depth > 16 || files.len() >= 10_000 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= 10_000 {
            break;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            walk(&entry.path(), depth + 1, files);
        } else if entry.path().extension() == Some(OsStr::new("desktop"))
            && (kind.is_file() || kind.is_symlink())
        {
            files.push(entry.path());
        }
    }
}

fn launch_spec(entry: &desktop::DesktopEntry, name: &str, paths: &[PathBuf]) -> Result<LaunchSpec> {
    if let Some(program) = entry.try_exec() {
        resolve_executable(program, paths).context("TryExec program is unavailable")?;
    }
    let mut argv = exec::parse(
        entry
            .exec()
            .context("This application requires D-Bus activation without an Exec fallback.")?,
        name,
        entry.icon(),
        &entry.path.to_string_lossy(),
    )?;
    argv[0] = resolve_executable(&argv[0], paths)?
        .to_string_lossy()
        .into_owned();
    if entry.terminal() {
        let terminal = resolve_executable("foot", paths)
            .context("Terminal applications require foot to be installed.")?;
        argv.splice(0..0, [terminal.to_string_lossy().into_owned(), "--".into()]);
    }
    let directory = entry
        .path()
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    anyhow::ensure!(
        directory.as_ref().is_none_or(|path| path.is_absolute()),
        "Application working directory must be absolute."
    );
    Ok(LaunchSpec { argv, directory })
}

fn resolve_executable(program: &str, paths: &[PathBuf]) -> Result<PathBuf> {
    let executable = |path: &Path| {
        fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
    };
    let path = Path::new(program);
    if path.is_absolute() && executable(path) {
        return Ok(path.into());
    }
    if !program.contains('/')
        && let Some(path) = paths
            .iter()
            .map(|directory| directory.join(program))
            .find(|path| executable(path))
    {
        return Ok(path);
    }
    Err(anyhow!("Executable is unavailable: {program}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_user_entry_masks_system_app_and_nested_ids_remain_stable() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("user");
        let system = dir.path().join("system");
        fs::create_dir_all(&user).unwrap();
        fs::create_dir_all(system.join("tools")).unwrap();
        fs::write(
            system.join("masked.desktop"),
            "[Desktop Entry]\nType=Application\nName=System App\nExec=/bin/echo\n",
        )
        .unwrap();
        fs::write(
            user.join("masked.desktop"),
            "[Desktop Entry]\nHidden=true\n",
        )
        .unwrap();
        fs::write(
            system.join("tools/visible.desktop"),
            "[Desktop Entry]\nType=Application\nName=Visible\nExec=/bin/echo %c\n",
        )
        .unwrap();
        let registry = Registry::scan([user, system], &[], &[]);
        assert_eq!(registry.applications.len(), 1);
        assert_eq!(registry.applications[0].id, "tools-visible.desktop");
        assert_eq!(
            registry.specs["tools-visible.desktop"]
                .as_ref()
                .unwrap()
                .argv,
            ["/bin/echo", "Visible"]
        );
    }

    #[test]
    fn desktop_visibility_and_launch_failures_are_explicit() {
        let dir = tempfile::tempdir().unwrap();
        for (id, fields) in [
            ("hidden", "NoDisplay=true"),
            ("gnome", "OnlyShowIn=GNOME;"),
            ("excluded", "NotShowIn=Tarot;"),
            ("missing", "TryExec=/tarot-missing-executable"),
            ("badexec", "Exec=/bin/echo %z"),
        ] {
            fs::write(
                dir.path().join(format!("{id}.desktop")),
                format!("[Desktop Entry]\nType=Application\nName={id}\nExec=/bin/echo\n{fields}\n"),
            )
            .unwrap();
        }
        let registry = Registry::scan([dir.path().into()], &[], &[]);
        assert_eq!(registry.applications.len(), 2);
        assert!(
            registry
                .applications
                .iter()
                .all(|app| app.unavailable.is_some())
        );
    }

    #[test]
    fn terminal_entries_wrap_structured_arguments_without_shell_reparsing() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = dir.path().join("foot");
        fs::write(&terminal, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&terminal, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(dir.path().join("cli.desktop"), "[Desktop Entry]\nType=Application\nName=CLI\nExec=/bin/echo \"two words\"\nTerminal=true\n").unwrap();
        let registry = Registry::scan([dir.path().into()], &[], &[dir.path().into()]);
        assert_eq!(
            registry.specs["cli.desktop"].as_ref().unwrap().argv,
            [
                terminal.to_string_lossy().as_ref(),
                "--",
                "/bin/echo",
                "two words"
            ]
        );
    }
}
