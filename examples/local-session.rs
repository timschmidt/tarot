//! Root-only, bounded-development launcher. Creates a real elogind session, then
//! starts an unprivileged command on the already selected VT8. No policy changes.
use anyhow::{Context, Result, bail, ensure};
use std::{
    io::Write,
    os::unix::fs::MetadataExt,
    process::{Command, Stdio},
    time::Duration,
};
use zbus::{
    blocking::{Proxy, connection::Builder},
    zvariant::{OwnedFd, OwnedObjectPath, OwnedValue},
};
type SessionReply = (
    String,
    OwnedObjectPath,
    String,
    OwnedFd,
    u32,
    String,
    u32,
    bool,
);
fn main() -> Result<()> {
    ensure!(
        rustix::process::getuid().as_raw() == 0,
        "Use an authenticated administrator terminal"
    );
    ensure!(
        std::fs::read_to_string("/proc/device-tree/model")?.contains("PinePhone Pro"),
        "Restricted to the verified spare PinePhone Pro"
    );
    ensure!(
        std::fs::read_to_string("/sys/class/tty/tty0/active")?.trim() == "tty8",
        "VT8 must already be selected by the bounded harness"
    );
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!(
        arguments.first().is_some_and(|arg| arg == "su"),
        "Expected the unprivileged su launcher"
    );
    ensure!(
        std::fs::metadata("/home/user")?.uid() == 10000,
        "Unexpected account ownership"
    );
    let original = std::fs::read_to_string("/sys/class/backlight/backlight/brightness")?
        .trim()
        .parse::<u32>()?;
    let connection = Builder::system()?
        .method_timeout(Duration::from_secs(3))
        .build()?;
    let manager = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )?;
    let mut child=Command::new("/bin/ash").args(["-c","IFS= read -r XDG_SESSION_ID || exit 1; export XDG_SESSION_ID XDG_SEAT=seat0 XDG_VTNR=8 XDG_SESSION_TYPE=wayland; exec \"$@\"","tarot-local-session"])
  .args(arguments).stdin(Stdio::piped()).spawn()?;
    // The SSH launcher belongs to an existing elogind cgroup. Move only this
    // newly forked, still-gated child out before registering its own session.
    // Neither the SSH session nor any pre-existing process is moved.
    if let Err(error) = std::fs::write("/sys/fs/cgroup/cgroup.procs", child.id().to_string()) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error)
            .context("detaching the gated development child from its inherited SSH cgroup");
    }
    let created: Result<SessionReply> = manager
        .call(
            "CreateSession",
            &(
                10000u32,
                child.id(),
                "tarot-development",
                "wayland",
                "user",
                "Tarot",
                "seat0",
                8u32,
                "tty8",
                "",
                false,
                "",
                "",
                Vec::<(String, OwnedValue)>::new(),
            ),
        )
        .map_err(Into::into);
    let (id, path, _runtime, lease, uid, seat, vt, existing) = match created {
        Ok(value) => value,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error).context("creating local development session");
        }
    };
    if existing || uid != 10000 || seat != "seat0" || vt != 8 {
        let _ = child.kill();
        let _ = child.wait();
        if !existing {
            let _: zbus::Result<()> = manager.call("ReleaseSession", &(&id,));
        }
        bail!(
            "Local session registration returned an unexpected session; existing sessions were retained"
        );
    }
    let result = (|| -> Result<_> {
        manager.call::<_, _, ()>("ActivateSession", &(&id,))?;
        let mut gate = child.stdin.take().context("missing session gate")?;
        writeln!(gate, "{id}")?;
        drop(gate);
        let status = child.wait()?;
        Ok(status)
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    // Restore this experiment's brightness while its foreground session still
    // exists; only the unprivileged compositor made the tested UI requests.
    let session = Proxy::new(
        &connection,
        "org.freedesktop.login1",
        path.as_str(),
        "org.freedesktop.login1.Session",
    )?;
    if std::fs::read_to_string("/sys/class/backlight/backlight/brightness")?
        .trim()
        .parse::<u32>()?
        != original
    {
        let restored: zbus::Result<()> =
            session.call("SetBrightness", &("backlight", "backlight", original));
        if restored.is_err() {
            // elogind may already have removed a session whose leader exited.
            // This root-only experiment restores only its pre-launch value;
            // the compositor's UI requests never use privileged sysfs writes.
            std::fs::write(
                "/sys/class/backlight/backlight/brightness",
                original.to_string(),
            )
            .context("restoring original development brightness after session exit")?;
        }
    }
    let released: zbus::Result<()> = manager.call("ReleaseSession", &(&id,));
    drop(lease);
    if let Err(error) = released {
        let already_gone = matches!(&error,zbus::Error::MethodError(name,_,_) if matches!(name.as_str(),"org.freedesktop.login1.NoSuchSession"|"org.freedesktop.DBus.Error.UnknownObject"));
        if !already_gone {
            return Err(error).context("releasing development session");
        }
    }
    ensure!(
        std::fs::read_to_string("/sys/class/backlight/backlight/actual_brightness")?
            .trim()
            .parse::<u32>()?
            == original,
        "Original brightness was not confirmed after cleanup"
    );
    let status = result?;
    ensure!(
        std::fs::metadata("/home/user")
            .context("checking development account")?
            .uid()
            == 10000,
        "Unexpected account ownership"
    );
    ensure!(
        status.success(),
        "Unprivileged development launcher exited {status}"
    );
    Ok(())
}
