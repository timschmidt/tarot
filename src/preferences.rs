//! Versioned shell preferences. All file access happens on a service worker.
mod service;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
pub use service::{Handle, Server, start};
use std::{
    ffi::OsString,
    fs::{self, DirBuilder, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Notifications {
    pub dnd: bool,
    pub hide_previews: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    version: u32,
    #[serde(default)]
    pub notifications: Notifications,
    #[serde(default)]
    pub display: tarot_core::display::Preferences,
    #[serde(default)]
    pub quick_launch: Option<tarot_core::quick_launch::Favorites>,
    #[serde(default)]
    pub launcher: Option<tarot_core::launcher::Settings>,
    #[serde(default)]
    pub feedback: tarot_core::feedback::Settings,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 6,
            notifications: Notifications::default(),
            display: tarot_core::display::Preferences::default(),
            quick_launch: None,
            launcher: None,
            feedback: tarot_core::feedback::Settings::default(),
        }
    }
}

pub fn config_path(config_home: Option<OsString>, home: Option<OsString>) -> Result<PathBuf> {
    let base = config_home
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            home.map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(".config"))
        })
        .context("No absolute XDG_CONFIG_HOME or HOME for shell settings")?;
    Ok(base.join("tarot/settings.toml"))
}

pub struct Store {
    path: PathBuf,
    pub current: Preferences,
    pub error: Option<String>,
    pub saved: bool,
    writable: bool,
}

impl Store {
    pub fn load(path: PathBuf) -> Self {
        match read(&path) {
            Ok(preferences) => Self {
                path,
                current: preferences.unwrap_or_default(),
                error: None,
                saved: true,
                writable: true,
            },
            Err(error) => Self {
                path,
                current: Preferences::default(),
                error: Some(format!(
                    "Cannot load shell settings: {error}. Original file preserved."
                )),
                saved: false,
                writable: false,
            },
        }
    }

    pub fn replace(&mut self, preferences: Preferences) {
        self.current = preferences;
        self.saved = false;
        if !self.writable {
            return;
        }
        match write(&self.path, &self.current) {
            Ok(()) => {
                self.saved = true;
                self.error = None;
            }
            Err(error) => {
                self.error = Some(format!(
                    "Settings apply this session but could not be saved: {error}"
                ));
            }
        }
    }
}

fn read(path: &Path) -> Result<Option<Preferences>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(
            rustix::fs::OFlags::NOFOLLOW.bits() as i32 | rustix::fs::OFlags::NONBLOCK.bits() as i32,
        )
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).context("opening settings"),
    };
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file() && metadata.uid() == rustix::process::getuid().as_raw(),
        "settings must be an owned regular file"
    );
    ensure!(metadata.len() <= MAX_BYTES, "settings exceed size limit");
    let mut text = String::new();
    file.take(MAX_BYTES + 1)
        .read_to_string(&mut text)
        .context("reading settings")?;
    ensure!(text.len() as u64 <= MAX_BYTES, "settings exceed size limit");
    let mut preferences: Preferences = toml::from_str(&text)
        .map_err(|_| anyhow::anyhow!("invalid or unsupported settings schema"))?;
    ensure!(
        (1..=6).contains(&preferences.version),
        "unsupported settings version"
    );
    if preferences.version < 6 {
        // Old schemas must not smuggle a later section into a migration.
        let old: toml::Value = toml::from_str(&text)?;
        ensure!(
            old.get("display")
                .and_then(|display| display.get("sleep"))
                .is_none()
                && (preferences.version >= 5 || old.get("feedback").is_none())
                && (preferences.version >= 4 || old.get("launcher").is_none())
                && (preferences.version >= 3 || old.get("quick_launch").is_none())
                && (preferences.version != 1 || old.get("display").is_none()),
            "invalid mixed settings version"
        );
        preferences.version = 6;
    }
    ensure!(preferences.display.idle.valid(), "invalid display timeouts");
    ensure!(preferences.display.sleep.valid(), "invalid sleep settings");
    ensure!(
        preferences.quick_launch.as_ref().is_none_or(|f| f.valid()),
        "invalid Quick Launch favorites"
    );
    ensure!(
        preferences.launcher.as_ref().is_none_or(|p| p.valid()),
        "invalid Launcher pages"
    );
    Ok(Some(preferences))
}

fn write(path: &Path, preferences: &Preferences) -> Result<()> {
    ensure!(
        preferences.version == 6
            && preferences.display.idle.valid()
            && preferences.display.sleep.valid()
            && preferences.quick_launch.as_ref().is_none_or(|f| f.valid())
            && preferences.launcher.as_ref().is_none_or(|p| p.valid()),
        "invalid settings"
    );
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .context("settings need a parent directory")?;
    DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .context("creating settings directory")?;
    match fs::symlink_metadata(path) {
        Ok(meta) if !meta.is_file() || meta.uid() != rustix::process::getuid().as_raw() => {
            bail!("settings target must be an owned regular file")
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            return Err(error).context("checking settings target");
        }
        _ => {}
    }
    let text = toml::to_string(preferences).context("encoding settings")?;
    ensure!(text.len() as u64 <= MAX_BYTES, "settings exceed size limit");
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).context("creating settings transaction")?;
    temporary
        .write_all(text.as_bytes())
        .context("writing settings")?;
    temporary.as_file().sync_all().context("syncing settings")?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .context("replacing settings")?;
    File::open(parent)?
        .sync_all()
        .context("syncing settings directory")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    #[test]
    fn version_two_favorites_migrate_lazily_and_explicit_empty_survives() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let original =
            "version = 2\n[notifications]\ndnd = true\n[display]\nrotation_locked = true\n";
        fs::write(&path, original).unwrap();
        let mut store = Store::load(path.clone());
        assert!(store.current.quick_launch.is_none() && store.saved);
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        // Files written before the fifth position existed still load, and a
        // missing fifth application is not written back.
        let older = dir.path().join("older.toml");
        fs::write(
            &older,
            "version = 6\n[quick_launch]\napplications = [\"a.desktop\"]\n",
        )
        .unwrap();
        let loaded = Store::load(older).current.quick_launch.unwrap();
        assert!(loaded.fifth.is_none() && loaded.applications == ["a.desktop"]);
        assert!(!toml::to_string(&loaded).unwrap().contains("fifth"));
        let mut next = store.current.clone();
        next.quick_launch = Some(tarot_core::quick_launch::Favorites {
            applications: vec!["one.desktop".into(), "two.desktop".into()],
            fifth: Some("three.desktop".into()),
        });
        store.replace(next.clone());
        assert!(store.saved);
        assert_eq!(Store::load(path.clone()).current, next);
        next.quick_launch = Some(Default::default());
        store.replace(next.clone());
        let loaded = Store::load(path);
        assert_eq!(loaded.current, next);
        assert!(loaded.current.notifications.dnd && loaded.current.display.rotation_locked);
    }

    #[test]
    fn version_three_migrates_lazily_and_pages_round_trip_with_other_sections() {
        use tarot_core::launcher::{Edit, PageId, Settings};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let original = "version = 3\n[notifications]\ndnd = true\n[quick_launch]\napplications = ['org.example.First.desktop']\n";
        fs::write(&path, original).unwrap();
        let mut store = Store::load(path.clone());
        assert!(store.saved && store.current.launcher.is_none());
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let mut pages = Settings::default();
        pages
            .edit(Edit::Create("Work \"and\" play".into()), &[])
            .unwrap();
        pages
            .edit(
                Edit::Reorder {
                    page: PageId(2),
                    at: 0,
                },
                &[],
            )
            .unwrap();
        let mut next = store.current.clone();
        next.launcher = Some(pages.clone());
        store.replace(next.clone());
        assert!(store.saved, "{:?}", store.error);
        let loaded = Store::load(path.clone());
        assert_eq!(loaded.current, next);
        assert_eq!(loaded.current.launcher, Some(pages));
        assert!(loaded.current.notifications.dnd);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let saved = fs::read_to_string(&path).unwrap();
        for invalid in [
            saved.replacen("version = 6", "version = 3", 1),
            saved.replacen("inbox = 1", "inbox = 999", 1),
        ] {
            assert_ne!(invalid, saved);
            fs::write(&path, &invalid).unwrap();
            let mut bad = Store::load(path.clone());
            assert!(bad.error.is_some());
            bad.update(Notifications::default());
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
    }

    impl Store {
        fn update(&mut self, notifications: Notifications) {
            let mut next = self.current.clone();
            next.notifications = notifications;
            self.replace(next);
        }
    }

    #[test]
    fn sleep_settings_need_schema_six_and_valid_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let original = "version = 5\n[display]\nrotation_locked = true\n";
        fs::write(&path, original).unwrap();
        let mut store = Store::load(path.clone());
        assert!(store.saved && store.error.is_none());
        assert_eq!(
            store.current.display.sleep,
            tarot_core::session_idle::Settings::default()
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let mut next = store.current.clone();
        next.display.sleep = tarot_core::session_idle::Settings {
            automatic: true,
            after_off_seconds: 300,
            on_external_power: true,
        };
        store.replace(next.clone());
        assert!(store.saved, "{:?}", store.error);
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.starts_with("version = 6\n") && saved.contains("[display.sleep]"));
        assert_eq!(Store::load(path.clone()).current, next);
        for invalid in [
            // An older schema cannot carry the newer section into a migration.
            saved.replacen("version = 6", "version = 5", 1),
            saved.replacen("after_off_seconds = 300", "after_off_seconds = 3601", 1),
            saved.replacen("automatic = true", "suspend = true", 1),
        ] {
            assert_ne!(invalid, saved);
            fs::write(&path, &invalid).unwrap();
            let mut bad = Store::load(path.clone());
            assert!(bad.error.is_some());
            bad.update(Notifications::default());
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
    }

    #[test]
    fn xdg_paths_require_absolute_values_and_have_a_home_fallback() {
        assert_eq!(
            config_path(Some("relative".into()), Some("/home/example".into())).unwrap(),
            PathBuf::from("/home/example/.config/tarot/settings.toml")
        );
        assert_eq!(
            config_path(Some("/config".into()), None).unwrap(),
            PathBuf::from("/config/tarot/settings.toml")
        );
        assert!(config_path(None, Some("relative".into())).is_err());
    }

    #[test]
    fn settings_survive_replacement_with_private_modes_and_no_temp_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tarot/settings.toml");
        let mut store = Store::load(path.clone());
        assert!(!path.exists());
        for settings in [
            Notifications {
                dnd: true,
                hide_previews: true,
            },
            Notifications {
                dnd: false,
                hide_previews: true,
            },
        ] {
            store.update(settings.clone());
            assert!(store.saved && store.error.is_none());
            assert_eq!(Store::load(path.clone()).current.notifications, settings);
        }
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn version_one_migrates_without_an_eager_write_and_preserves_both_sections() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        let original = "version = 1\n[notifications]\ndnd = true\nhide_previews = true\n";
        fs::write(&path, original).unwrap();
        let mut store = Store::load(path.clone());
        assert!(store.saved && store.current.notifications.dnd);
        assert_eq!(store.current.version, 6);
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let mut next = store.current.clone();
        next.display.rotation_locked = true;
        next.display.locked_orientation = tarot_core::orientation::Orientation::LeftUp;
        next.display.idle.lock_after_seconds = 60;
        store.replace(next.clone());
        assert!(store.saved);
        assert_eq!(Store::load(path.clone()).current, next);
        store.update(Notifications {
            dnd: false,
            hide_previews: true,
        });
        let restored = Store::load(path);
        assert_eq!(restored.current.display, next.display);
        assert!(!restored.current.notifications.dnd);
    }

    #[test]
    fn version_four_feedback_migration_is_lazy_and_rejects_mixed_schema() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        let original = "version = 4\n[notifications]\ndnd = true\n";
        fs::write(&path, original).unwrap();
        let mut store = Store::load(path.clone());
        assert!(
            store.saved && store.current.feedback.notifications && store.current.notifications.dnd
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        let mut next = store.current.clone();
        next.feedback.notifications = false;
        store.replace(next);
        let loaded = Store::load(path.clone());
        assert!(
            loaded.saved
                && !loaded.current.feedback.notifications
                && loaded.current.notifications.dnd
        );
        let mixed = original.to_owned() + "[feedback]\nnotifications = false\n";
        fs::write(&path, &mixed).unwrap();
        let mut bad = Store::load(path.clone());
        assert!(bad.error.is_some());
        bad.update(Notifications::default());
        assert_eq!(fs::read_to_string(&path).unwrap(), mixed);
    }

    #[test]
    fn invalid_newer_oversized_or_symlink_settings_are_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        for text in [
            "broken TOML".to_string(),
            "version = 7\n".into(),
            "version = 5\n[display.sleep]\nautomatic = true\n".into(),
            "version = 1\nunknown = true\n".into(),
            "version = 1\n[display]\nrotation_locked = true\n".into(),
            "version = 2\n[display.idle]\nlock_after_seconds = 0\n".into(),
            "version = 2\n[display.idle]\noff_after_seconds = 30\n".into(),
            "version = 2\n[quick_launch]\napplications = []\n".into(),
            "version = 3\n[quick_launch]\napplications = ['one.desktop','one.desktop']\n".into(),
            "version = 3\n[quick_launch]\napplications = ['../one.desktop']\n".into(),
            "x".repeat(MAX_BYTES as usize + 1),
        ] {
            fs::write(&path, &text).unwrap();
            let mut store = Store::load(path.clone());
            assert!(store.error.is_some());
            store.update(Notifications {
                dnd: true,
                hide_previews: true,
            });
            assert!(!store.saved);
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
        fs::remove_file(&path).unwrap();
        let target = directory.path().join("target.toml");
        fs::write(&target, "version = 1\n").unwrap();
        symlink(&target, &path).unwrap();
        let mut store = Store::load(path);
        store.update(Notifications::default());
        assert!(!store.saved);
        assert_eq!(fs::read_to_string(target).unwrap(), "version = 1\n");
    }

    #[test]
    fn failed_save_keeps_old_data_and_a_later_change_can_retry() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.toml");
        let mut store = Store::load(path.clone());
        store.update(Notifications::default());
        let previous = fs::read(&path).unwrap();
        fs::rename(&path, directory.path().join("backup")).unwrap();
        fs::create_dir(&path).unwrap();
        store.update(Notifications {
            dnd: true,
            hide_previews: false,
        });
        assert!(!store.saved && store.error.is_some());
        assert_eq!(fs::read(directory.path().join("backup")).unwrap(), previous);
        fs::remove_dir(&path).unwrap();
        store.update(Notifications {
            dnd: true,
            hide_previews: false,
        });
        assert!(store.saved && store.error.is_none());
        assert!(Store::load(path).current.notifications.dnd);
    }
}
