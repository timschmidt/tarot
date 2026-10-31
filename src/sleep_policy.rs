//! System sleep policy: whether elogind suspends once every session is idle.
//!
//! Tarot's session only reports protected idleness; elogind's `IdleAction`
//! decides whether sleep follows. The Power page's switch writes a
//! Tarot-managed elogind drop-in through the root helper behind Polkit.
//! The separate `tarot-automatic-sleep` package installs its own drop-in; while
//! it is present the setting reports on and cannot be changed here.
//!
//! elogind 255 arms its idle timer only at startup, so turning the policy on
//! takes effect after a restart; the drop-in records the boot it was written
//! in so the page can say so. Turning it off removes the drop-in, and a SIGHUP
//! reload restores `IdleAction=ignore` at once.
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 64 * 1024;
const BOOT_MARKER: &str = "# Written during boot ";

pub struct Paths {
    pub managed: PathBuf,
    pub package: PathBuf,
    pub boot_id: PathBuf,
}

impl Default for Paths {
    fn default() -> Self {
        Self {
            managed: "/etc/elogind/logind.conf.d/50-tarot-automatic-sleep-setting.conf".into(),
            package: "/etc/elogind/logind.conf.d/50-tarot-automatic-sleep.conf".into(),
            boot_id: "/proc/sys/kernel/random/boot_id".into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Status {
    /// A drop-in asks elogind to suspend when idle.
    pub enabled: bool,
    /// The package's drop-in is installed, so the setting cannot change it.
    pub package: bool,
    /// Turned on during this boot; elogind applies it after a restart.
    pub restart_required: bool,
}

/// Present (and a small regular file) or absent; anything else is an error.
fn read_small(path: &Path) -> Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
        Ok(meta) => {
            ensure!(
                meta.is_file() && meta.len() <= MAX_BYTES,
                "{} must be a small regular file",
                path.display()
            );
            Ok(Some(fs::read_to_string(path)?))
        }
    }
}

fn boot_id(paths: &Paths) -> Result<String> {
    let id = fs::read_to_string(&paths.boot_id).context("reading the boot id")?;
    let id = id.trim();
    ensure!(
        !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'),
        "unexpected boot id"
    );
    Ok(id.to_owned())
}

pub fn status(paths: &Paths) -> Result<Status> {
    let package = read_small(&paths.package)?.is_some();
    let managed = read_small(&paths.managed)?;
    let restart_required = match &managed {
        Some(text) => {
            let current = boot_id(paths)?;
            text.lines()
                .any(|line| line.strip_prefix(BOOT_MARKER) == Some(current.as_str()))
        }
        None => false,
    };
    Ok(Status {
        enabled: package || managed.is_some(),
        package,
        restart_required: restart_required && !package,
    })
}

pub fn managed_text(boot: &str) -> String {
    format!(
        "# Managed by Tarot's automatic sleep setting; turn it off in Power settings.\n\
         {BOOT_MARKER}{boot}\n\
         #\n\
         # Tarot reports its session idle only after a protected screen-off, once\n\
         # the user's Automatic sleep setting allows it. elogind then suspends\n\
         # when every session is idle and no sleep block exists.\n\
         [Login]\n\
         IdleAction=suspend\n\
         IdleActionSec=10s\n"
    )
}

pub fn enable(paths: &Paths) -> Result<()> {
    let current = status(paths)?;
    ensure!(
        !current.package,
        "the tarot-automatic-sleep package already turns this on"
    );
    if current.enabled {
        return Ok(());
    }
    let parent = paths.managed.parent().context("path has no parent")?;
    fs::create_dir_all(parent)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(managed_text(&boot_id(paths)?).as_bytes())?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(0o644))?;
    temp.as_file().sync_all()?;
    temp.persist(&paths.managed).map_err(|e| e.error)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

/// Returns whether a drop-in was removed, so the caller knows to reload.
pub fn disable(paths: &Paths) -> Result<bool> {
    let current = status(paths)?;
    ensure!(
        !current.package,
        "remove the tarot-automatic-sleep package to turn this off"
    );
    if read_small(&paths.managed)?.is_none() {
        return Ok(false);
    }
    fs::remove_file(&paths.managed)?;
    fs::File::open(paths.managed.parent().context("path has no parent")?)?.sync_all()?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(dir: &Path) -> Paths {
        fs::write(
            dir.join("boot_id"),
            "0f1e2d3c-aaaa-bbbb-cccc-0123456789ab\n",
        )
        .unwrap();
        Paths {
            managed: dir.join("conf.d/50-managed.conf"),
            package: dir.join("conf.d/50-package.conf"),
            boot_id: dir.join("boot_id"),
        }
    }

    #[test]
    fn enabling_needs_a_restart_in_the_same_boot_and_disabling_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        let off = status(&paths).unwrap();
        assert_eq!(
            (off.enabled, off.package, off.restart_required),
            (false, false, false)
        );
        assert!(!disable(&paths).unwrap(), "nothing to remove");
        enable(&paths).unwrap();
        let on = status(&paths).unwrap();
        assert_eq!((on.enabled, on.restart_required), (true, true));
        let text = fs::read_to_string(&paths.managed).unwrap();
        assert!(text.contains("IdleAction=suspend\n") && text.contains("IdleActionSec=10s\n"));
        assert_eq!(
            fs::metadata(&paths.managed).unwrap().permissions().mode() & 0o777,
            0o644
        );
        enable(&paths).unwrap();
        assert_eq!(
            fs::read_to_string(&paths.managed).unwrap(),
            text,
            "enabling is idempotent"
        );
        // After a reboot the recorded boot no longer matches.
        fs::write(&paths.boot_id, "11111111-2222-3333-4444-555555555555\n").unwrap();
        assert!(!status(&paths).unwrap().restart_required);
        assert!(disable(&paths).unwrap());
        assert!(!paths.managed.exists() && !status(&paths).unwrap().enabled);
    }

    #[test]
    fn the_package_drop_in_wins_and_unusual_files_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        fs::create_dir_all(paths.package.parent().unwrap()).unwrap();
        fs::write(&paths.package, "[Login]\nIdleAction=suspend\n").unwrap();
        let package = status(&paths).unwrap();
        assert_eq!(
            (package.enabled, package.package, package.restart_required),
            (true, true, false)
        );
        assert!(enable(&paths).is_err() && disable(&paths).is_err());
        assert!(!paths.managed.exists());
        fs::remove_file(&paths.package).unwrap();
        std::os::unix::fs::symlink(dir.path().join("elsewhere"), &paths.managed).unwrap();
        assert!(
            status(&paths).is_err(),
            "a symlinked drop-in is not trusted"
        );
        assert!(enable(&paths).is_err() && disable(&paths).is_err());
    }
}
