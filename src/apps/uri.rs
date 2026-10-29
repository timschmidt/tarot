//! Explicit URI handoff, on one bounded worker. No query text is logged.
use super::AppEvent;
use std::{
    ffi::{OsStr, OsString},
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

pub struct Request {
    pub generation: u64,
    pub search_revision: u64,
    pub uri: String,
    pub display: OsString,
}

pub fn start(
    events: smithay::reexports::calloop::channel::Sender<AppEvent>,
) -> mpsc::SyncSender<Request> {
    let (tx, rx) = mpsc::sync_channel::<Request>(1);
    std::thread::spawn(move || {
        let paths = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
            .unwrap_or_default();
        let gio = super::resolve_executable("gio", &paths).ok();
        let mime = super::resolve_executable("xdg-mime", &paths).ok();
        while let Ok(request) = rx.recv() {
            let handler = mime.as_ref().and_then(|p| {
                default_handler(
                    p,
                    if request.uri.starts_with("https://") {
                        "x-scheme-handler/https"
                    } else {
                        "x-scheme-handler/http"
                    },
                )
            });
            let success = gio
                .as_ref()
                .is_some_and(|p| open(p, &request.display, &request.uri));
            let _ = events.send(AppEvent::UriFinished {
                generation: request.generation,
                search_revision: request.search_revision,
                handler,
                success,
            });
        }
    });
    tx
}

fn wait(child: &mut Child, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Err(_) => return false,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

fn default_handler(program: &Path, scheme: &str) -> Option<String> {
    let mut child = Command::new(program)
        .args(["query", "default", scheme])
        .env("XDG_CURRENT_DESKTOP", "Tarot")
        .env("XDG_SESSION_DESKTOP", "tarot")
        .env("XDG_SESSION_TYPE", "wayland")
        .env_remove("DISPLAY")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    if rustix::fs::fcntl_setfl(&stdout, rustix::fs::OFlags::NONBLOCK).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut bytes = Vec::new();
    let success = loop {
        let mut buffer = [0; 512];
        match stdout.read(&mut buffer) {
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => break false,
        }
        if bytes.len() > 4096 || Instant::now() >= deadline {
            break false;
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let mut rest = [0; 4096];
                if let Ok(n) = stdout.read(&mut rest) {
                    bytes.extend_from_slice(&rest[..n]);
                }
                break status.success() && bytes.len() <= 4096;
            }
            Err(_) => break false,
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    if !success {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    }
    let value = String::from_utf8(bytes).ok()?.trim().to_owned();
    (value.ends_with(".desktop")
        && value.len() <= 256
        && !value
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '/'))
    .then_some(value)
}

fn open(program: &Path, display: &OsStr, uri: &str) -> bool {
    // These are the only external URI schemes exposed by Just Type.
    if !uri.starts_with("https://") && !uri.starts_with("http://") {
        return false;
    }
    let Ok(mut child) = Command::new(program)
        .args(["open", uri])
        .env("WAYLAND_DISPLAY", display)
        .env("GDK_BACKEND", "wayland")
        .env("QT_QPA_PLATFORM", "wayland")
        .env("XDG_SESSION_TYPE", "wayland")
        .env("XDG_CURRENT_DESKTOP", "Tarot")
        .env("XDG_SESSION_DESKTOP", "tarot")
        .env_remove("DISPLAY")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    wait(&mut child, Duration::from_secs(10))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn uri_handoff_keeps_metacharacters_in_one_argument_and_rejects_non_web_schemes() {
        let dir = tempfile::tempdir().unwrap();
        let helper = dir.path().join("gio");
        let output = dir.path().join("argument");
        let sentinel = dir.path().join("must-not-exist");
        std::fs::write(
            &helper,
            format!(
                "#!/bin/sh\n[ \"$1\" = open ] || exit 2\nprintf '%s' \"$2\" > '{}'\n",
                output.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
        let uri = format!(
            "https://example.invalid/?q=$(touch {})&x=one;two",
            sentinel.display()
        );
        assert!(open(&helper, OsStr::new("owned-display"), &uri));
        assert_eq!(std::fs::read_to_string(&output).unwrap(), uri);
        assert!(!sentinel.exists());
        std::fs::remove_file(output).unwrap();
        assert!(!open(
            &helper,
            OsStr::new("owned-display"),
            "file:///tmp/forbidden"
        ));
        assert!(!dir.path().join("argument").exists());
    }
    #[test]
    fn helper_deadline_reaps_its_owned_child() {
        let mut child = Command::new("/bin/sleep").arg("10").spawn().unwrap();
        let now = Instant::now();
        assert!(!wait(&mut child, Duration::from_millis(20)));
        assert!(now.elapsed() < Duration::from_secs(2));
        assert!(child.try_wait().unwrap().is_some());
    }
}
