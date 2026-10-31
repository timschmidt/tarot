//! Boot destination: the distribution greeter, or a direct (locked) Tarot session.
//!
//! greetd selects its configuration through the OpenRC `cfgfile=` line. Direct
//! boot writes a Tarot-managed greetd configuration that keeps the current
//! greeter as `default_session` and adds an `initial_session` for one user;
//! greetd runs that once per boot and falls back to the greeter afterwards.
//! The original `cfgfile` is recorded before switching so it can be restored.
//! Reading is unprivileged; changes run in the root helper behind Polkit.
use anyhow::{Context, Result, bail, ensure};
use serde::Serialize;
use std::{
    fs,
    io::Write,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

/// greetd 0.10.3 starts an initial session with an empty environment; this PAM
/// service supplies the Wayland session type before pam_elogind runs.
pub const PAM_SERVICE: &str = "tarot-greetd";
pub const SESSION_COMMAND: &str = "/usr/bin/tarot-session";
const MAX_BYTES: u64 = 64 * 1024;

pub struct Paths {
    pub confd: PathBuf,
    pub managed: PathBuf,
    pub state: PathBuf,
}

impl Default for Paths {
    fn default() -> Self {
        Self {
            confd: "/etc/conf.d/greetd".into(),
            managed: "/etc/tarot/greetd-direct.toml".into(),
            state: "/etc/tarot/boot-destination".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Destination {
    Greeter,
    Tarot,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Status {
    pub destination: Destination,
    /// The account started directly, when the destination is Tarot.
    pub user: Option<String>,
}

fn read_small(path: &Path) -> Result<String> {
    let meta = fs::symlink_metadata(path).with_context(|| format!("reading {}", path.display()))?;
    ensure!(
        meta.is_file() && meta.len() <= MAX_BYTES,
        "{} must be a small regular file",
        path.display()
    );
    Ok(fs::read_to_string(path)?)
}

/// The value of the single active `cfgfile=` assignment.
fn cfgfile(text: &str) -> Result<PathBuf> {
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("cfgfile=") {
            ensure!(found.is_none(), "several cfgfile assignments");
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .unwrap_or(value);
            ensure!(
                value.starts_with('/') && !value.contains(['"', '$', '`', '\\', ' ']),
                "unsupported cfgfile value"
            );
            found = Some(PathBuf::from(value));
        }
    }
    found.context("no cfgfile assignment")
}

fn with_cfgfile(text: &str, path: &Path) -> String {
    let mut out = String::new();
    for line in text.lines() {
        if line.trim().starts_with("cfgfile=") {
            out.push_str(&format!("cfgfile=\"{}\"", path.display()));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

pub fn status(paths: &Paths) -> Result<Status> {
    let active = cfgfile(&read_small(&paths.confd)?)?;
    if active != paths.managed {
        return Ok(Status {
            destination: Destination::Greeter,
            user: None,
        });
    }
    let table: toml::Table = read_small(&paths.managed)?.parse()?;
    let user = table
        .get("initial_session")
        .and_then(|s| s.get("user"))
        .and_then(|u| u.as_str())
        .context("managed configuration has no initial session user")?;
    Ok(Status {
        destination: Destination::Tarot,
        user: Some(user.to_owned()),
    })
}

fn valid_user(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}

/// Root-owned, mode-0644 replacement in the same directory.
fn replace(path: &Path, text: &str, mode: u32, owner: Option<(u32, u32)>) -> Result<()> {
    let parent = path.parent().context("path has no parent")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    temp.write_all(text.as_bytes())?;
    temp.as_file()
        .set_permissions(fs::Permissions::from_mode(mode))?;
    if let Some((uid, gid)) = owner {
        std::os::unix::fs::fchown(temp.as_file(), Some(uid), Some(gid))?;
    }
    temp.as_file().sync_all()?;
    temp.persist(path).map_err(|e| e.error)?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

/// The direct-boot configuration derived from the current greeter config.
pub fn managed_config(greeter: &str, user: &str) -> Result<String> {
    ensure!(valid_user(user), "unsupported user name");
    let table: toml::Table = greeter.parse().context("parsing greeter configuration")?;
    ensure!(
        table.get("initial_session").is_none(),
        "the greeter configuration already has an initial session"
    );
    let terminal = table.get("terminal").context("no [terminal] section")?;
    let default = table
        .get("default_session")
        .context("no [default_session] section")?;
    let mut out = toml::Table::new();
    out.insert("terminal".into(), terminal.clone());
    let mut general = table
        .get("general")
        .and_then(|g| g.as_table())
        .cloned()
        .unwrap_or_default();
    general.insert("service".into(), PAM_SERVICE.into());
    out.insert("general".into(), general.into());
    out.insert("default_session".into(), default.clone());
    let mut initial = toml::Table::new();
    initial.insert("command".into(), SESSION_COMMAND.into());
    initial.insert("user".into(), user.into());
    out.insert("initial_session".into(), initial.into());
    Ok(format!(
        "# Managed by Tarot's boot destination setting; edits are replaced.\n\
         # The greeter stays default_session: logging out returns to it.\n{}",
        toml::to_string(&out)?
    ))
}

/// Select direct boot for `user`. Idempotent when already selected for them.
pub fn select_tarot(paths: &Paths, user: &str) -> Result<()> {
    let confd = read_small(&paths.confd)?;
    let active = cfgfile(&confd)?;
    let original = if active == paths.managed {
        PathBuf::from(read_small(&paths.state)?.trim())
    } else {
        active
    };
    ensure!(
        original != paths.managed && original.is_absolute(),
        "unusable original greeter configuration"
    );
    let text = managed_config(&read_small(&original)?, user)?;
    let dir = paths
        .managed
        .parent()
        .context("managed path has no parent")?;
    fs::create_dir_all(dir)?;
    // Record the original before any switch so it can always be restored.
    replace(
        &paths.state,
        &format!("{}\n", original.display()),
        0o644,
        None,
    )?;
    replace(&paths.managed, &text, 0o644, None)?;
    if cfgfile(&confd)? != paths.managed {
        let meta = fs::metadata(&paths.confd)?;
        replace(
            &paths.confd,
            &with_cfgfile(&confd, &paths.managed),
            meta.mode() & 0o7777,
            Some((meta.uid(), meta.gid())),
        )?;
    }
    Ok(())
}

/// Return to the recorded greeter configuration and remove managed files.
pub fn select_greeter(paths: &Paths) -> Result<()> {
    let confd = read_small(&paths.confd)?;
    if cfgfile(&confd)? != paths.managed {
        // Already the greeter; clear any leftovers from an interrupted change.
        let _ = fs::remove_file(&paths.managed);
        let _ = fs::remove_file(&paths.state);
        return Ok(());
    }
    let original = PathBuf::from(read_small(&paths.state)?.trim());
    ensure!(
        original.is_absolute() && original != paths.managed && original.is_file(),
        "recorded greeter configuration is missing"
    );
    let meta = fs::metadata(&paths.confd)?;
    replace(
        &paths.confd,
        &with_cfgfile(&confd, &original),
        meta.mode() & 0o7777,
        Some((meta.uid(), meta.gid())),
    )?;
    fs::remove_file(&paths.managed)?;
    fs::remove_file(&paths.state)?;
    Ok(())
}

/// Resolve a numeric UID to a local account through the passwd database file.
pub fn account_name(passwd: &str, uid: u32) -> Result<String> {
    for line in passwd.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        if fields.len() >= 7 && fields[2].parse::<u32>().ok() == Some(uid) {
            ensure!(valid_user(fields[0]), "unsupported user name");
            return Ok(fields[0].to_owned());
        }
    }
    bail!("no local account for UID {uid}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const GREETER: &str = "# phrog\n\n[terminal]\nvt = 7\n\n[default_session]\ncommand = \"/usr/libexec/phrog-greetd-session\"\nuser = \"greetd\"\n";
    const CONFD: &str = "# Configuration for greetd\n\n# Path to config file to use.\ncfgfile=\"/GREETER\"\n\n# supervisor=supervise-daemon\n";

    fn setup() -> (tempfile::TempDir, Paths, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let greeter = dir.path().join("phrog.toml");
        fs::write(&greeter, GREETER).unwrap();
        let confd = dir.path().join("greetd");
        fs::write(&confd, CONFD.replace("/GREETER", greeter.to_str().unwrap())).unwrap();
        fs::set_permissions(&confd, fs::Permissions::from_mode(0o640)).unwrap();
        let paths = Paths {
            confd,
            managed: dir.path().join("tarot/greetd-direct.toml"),
            state: dir.path().join("tarot/boot-destination"),
        };
        (dir, paths, greeter)
    }

    #[test]
    fn direct_boot_round_trip_restores_the_exact_original_file() {
        let (_dir, paths, greeter) = setup();
        let original = fs::read_to_string(&paths.confd).unwrap();
        assert_eq!(status(&paths).unwrap().destination, Destination::Greeter);
        select_tarot(&paths, "user").unwrap();
        assert_eq!(
            status(&paths).unwrap(),
            Status {
                destination: Destination::Tarot,
                user: Some("user".into())
            }
        );
        let managed: toml::Table = fs::read_to_string(&paths.managed).unwrap().parse().unwrap();
        assert_eq!(
            managed["initial_session"]["command"].as_str(),
            Some(SESSION_COMMAND)
        );
        assert_eq!(managed["general"]["service"].as_str(), Some(PAM_SERVICE));
        assert_eq!(managed["terminal"]["vt"].as_integer(), Some(7));
        assert_eq!(
            managed["default_session"]["command"].as_str(),
            Some("/usr/libexec/phrog-greetd-session")
        );
        // Only the cfgfile line changes, and the file keeps its mode.
        let switched = fs::read_to_string(&paths.confd).unwrap();
        assert_eq!(switched.lines().count(), original.lines().count());
        assert_eq!(
            fs::metadata(&paths.confd).unwrap().permissions().mode() & 0o7777,
            0o640
        );
        // Re-selecting is idempotent and keeps the recorded original.
        select_tarot(&paths, "user").unwrap();
        assert_eq!(
            fs::read_to_string(&paths.state).unwrap().trim(),
            greeter.to_str().unwrap()
        );
        select_greeter(&paths).unwrap();
        assert_eq!(fs::read_to_string(&paths.confd).unwrap(), original);
        assert!(!paths.managed.exists() && !paths.state.exists());
        assert_eq!(status(&paths).unwrap().destination, Destination::Greeter);
        select_greeter(&paths).unwrap();
        assert_eq!(fs::read_to_string(&paths.confd).unwrap(), original);
    }

    #[test]
    fn unsafe_inputs_are_rejected_without_switching() {
        let (_dir, paths, greeter) = setup();
        let original = fs::read_to_string(&paths.confd).unwrap();
        for user in ["", "-root", "a b", "x\"y", "../etc"] {
            assert!(select_tarot(&paths, user).is_err(), "{user:?}");
        }
        fs::write(
            &greeter,
            format!("{GREETER}\n[initial_session]\ncommand = \"x\"\nuser = \"y\"\n"),
        )
        .unwrap();
        assert!(select_tarot(&paths, "user").is_err());
        fs::write(&greeter, "[terminal]\nvt = 7\n").unwrap();
        assert!(select_tarot(&paths, "user").is_err());
        assert_eq!(fs::read_to_string(&paths.confd).unwrap(), original);
        for confd in [
            "cfgfile=$HOME/x\n",
            "cfgfile=\"rel.toml\"\n",
            "# none\n",
            "cfgfile=/a\ncfgfile=/b\n",
        ] {
            fs::write(&paths.confd, confd).unwrap();
            assert!(
                status(&paths).is_err() || cfgfile(confd).is_err(),
                "{confd:?}"
            );
            assert!(select_tarot(&paths, "user").is_err(), "{confd:?}");
        }
    }

    #[test]
    fn a_missing_recorded_greeter_blocks_restoration() {
        let (_dir, paths, greeter) = setup();
        select_tarot(&paths, "user").unwrap();
        fs::remove_file(&greeter).unwrap();
        assert!(select_greeter(&paths).is_err());
        assert_eq!(status(&paths).unwrap().destination, Destination::Tarot);
    }

    #[test]
    fn accounts_resolve_by_numeric_uid_only() {
        let passwd = "root:x:0:0:root:/root:/bin/sh\nuser:x:10000:10000::/home/user:/bin/bash\nbad name:x:5:5::/:/bin/sh\n";
        assert_eq!(account_name(passwd, 10000).unwrap(), "user");
        assert!(account_name(passwd, 5).is_err());
        assert!(account_name(passwd, 4242).is_err());
    }
}
