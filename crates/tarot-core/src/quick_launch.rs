//! Ordered desktop identities and shared Quick Launch hit geometry.
use crate::{Application, Layout, Rect};
use serde::{Deserialize, Serialize};

pub const CAPACITY: usize = 4;
/// The fifth position: the Launcher unless the owner chose an application.
pub const FIFTH: usize = CAPACITY;

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 255
        && id.ends_with(".desktop")
        && !id
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\'))
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Favorites {
    pub applications: Vec<String>,
    /// An application in the fifth position, replacing the Launcher there.
    /// The Launcher stays reachable by swiping up and from the System Menu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fifth: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum Edit {
    Add(String),
    Remove(usize),
    Move {
        from: usize,
        to: usize,
    },
    Place {
        id: String,
        at: usize,
    },
    /// Set or clear the fifth position.
    Fifth(Option<String>),
}

impl Favorites {
    /// The application in a dock position, if any (`FIFTH` included).
    pub fn at(&self, index: usize) -> Option<&String> {
        if index == FIFTH {
            self.fifth.as_ref()
        } else {
            self.applications.get(index)
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.applications.iter().any(|entry| entry == id) || self.fifth.as_deref() == Some(id)
    }

    pub fn valid(&self) -> bool {
        self.fifth
            .as_ref()
            .is_none_or(|id| valid_id(id) && !self.applications.contains(id))
            && self.applications.len() <= CAPACITY
            && self
                .applications
                .iter()
                .enumerate()
                .all(|(i, id)| valid_id(id) && !self.applications[..i].contains(id))
    }

    pub fn initial(catalog: &[Application]) -> Self {
        // Preserve the classic role order where the platform provides it.
        // Missing applications leave free positions rather than dead launchers.
        let roles: &[&[&str]] = &[
            &["org.gnome.Calls.desktop", "org.kde.phone.dialer.desktop"],
            &["org.gnome.Contacts.desktop", "org.kde.phonebook.desktop"],
            &["org.gnome.Geary.desktop", "org.kde.kmail2.desktop"],
            &["org.gnome.Calendar.desktop", "org.kde.kalendar.desktop"],
        ];
        Self {
            applications: roles
                .iter()
                .filter_map(|role| {
                    role.iter().find(|id| {
                        catalog
                            .iter()
                            .any(|a| a.id == **id && a.unavailable.is_none())
                    })
                })
                .map(|id| (*id).to_owned())
                .collect(),
            fifth: None,
        }
    }

    pub fn edit(&mut self, edit: Edit, catalog: &[Application]) -> bool {
        let mut next = self.clone();
        let available = |id: &str| {
            catalog
                .iter()
                .any(|a| a.id == id && a.unavailable.is_none())
        };
        match edit {
            Edit::Place { id, at: FIFTH } | Edit::Fifth(Some(id)) => {
                if !available(&id) {
                    return false;
                }
                next.applications.retain(|entry| *entry != id);
                next.fifth = Some(id);
            }
            Edit::Fifth(None) | Edit::Remove(FIFTH) => next.fifth = None,
            Edit::Place { id, at } if at < CAPACITY && next.fifth.as_ref() == Some(&id) => {
                // Moving the fifth application into an ordinary position.
                if next.applications.len() == CAPACITY {
                    return false;
                }
                next.fifth = None;
                next.applications
                    .insert(at.min(next.applications.len()), id);
            }
            Edit::Place { id, at } if at < CAPACITY => {
                if let Some(from) = next.applications.iter().position(|entry| *entry == id) {
                    next.applications.remove(from);
                } else if next.applications.len() == CAPACITY
                    || !catalog
                        .iter()
                        .any(|a| a.id == id && a.unavailable.is_none())
                {
                    return false;
                }
                next.applications
                    .insert(at.min(next.applications.len()), id);
            }
            Edit::Add(id) => {
                if next.applications.len() == CAPACITY
                    || next.contains(&id)
                    || !catalog
                        .iter()
                        .any(|a| a.id == id && a.unavailable.is_none())
                {
                    return false;
                }
                next.applications.push(id);
            }
            Edit::Remove(i) if i < next.applications.len() => {
                next.applications.remove(i);
            }
            Edit::Move { from, to }
                if from < next.applications.len() && to < next.applications.len() =>
            {
                let id = next.applications.remove(from);
                next.applications.insert(to, id);
            }
            _ => return false,
        }
        if !next.valid() || next == *self {
            return false;
        }
        *self = next;
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Wave {
    pub finger: (f32, f32),
    pub selected: Option<usize>,
}

/// Four favorites, then the fifth position (the Launcher by default).
pub fn slot(layout: Layout, index: usize, wave: Option<Wave>) -> Rect {
    let width = ((layout.width - 6.0) / 5.0).max(1.0);
    let x = 3.0 + index as f32 * width;
    let y = if let Some(wave) = wave {
        let distance = ((x + width / 2.0 - wave.finger.0) / width).abs().min(4.0);
        (wave.finger.1 - 42.0 + distance * 7.0).clamp(
            Layout::STATUS + 28.0,
            (layout.gesture().y - 64.0).max(Layout::STATUS + 28.0),
        )
    } else {
        layout.tray_bottom() - crate::design::DOCK - 4.0
    };
    Rect {
        x,
        y,
        width,
        height: crate::design::DOCK,
    }
}

pub fn wave(layout: Layout, finger: (f32, f32)) -> Wave {
    let selected = if finger.0 >= 3.0
        && finger.0 < layout.width - 3.0
        && finger.1 >= Layout::STATUS + 28.0
        && finger.1 < layout.gesture().y - 16.0
    {
        Some(((finger.0 - 3.0) / ((layout.width - 6.0) / 5.0).max(1.0)).floor() as usize)
            .filter(|i| *i <= CAPACITY)
    } else {
        None
    };
    Wave { finger, selected }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn favorite_activation_focuses_existing_identity_or_launches_once() {
        use crate::{Command, Effect, ShellModel, WindowId};
        let mut m = ShellModel::default();
        m.applications.push(Application {
            id: "app.desktop".into(),
            name: "App".into(),
            description: String::new(),
            icon: None,
            keywords: vec![],
            unavailable: None,
        });
        m.favorites.applications.push("app.desktop".into());
        assert_eq!(
            m.apply(Command::QuickLaunch(0)),
            vec![Effect::Launch("app.desktop".into())]
        );
        assert!(m.apply(Command::QuickLaunch(0)).is_empty());
        m.add_window(WindowId(7), "Already open".into(), "app".into());
        m.apply(Command::Home);
        assert_eq!(
            m.apply(Command::QuickLaunch(0)),
            vec![Effect::Focus(Some(WindowId(7)))]
        );
        m.system_menu_open = true;
        assert!(m.apply(Command::QuickLaunch(0)).is_empty());
        assert!(m.apply(Command::QuickLaunch(5)).is_empty());
    }
    #[test]
    fn favorites_keep_order_identity_and_explicit_empty_state() {
        let catalog: Vec<_> = (0..6)
            .map(|n| Application {
                id: format!("app{n}.desktop"),
                name: format!("App {n}"),
                description: String::new(),
                icon: None,
                keywords: vec![],
                unavailable: None,
            })
            .collect();
        let mut f = Favorites::initial(&catalog);
        assert!(f.applications.is_empty());
        for app in &catalog[..4] {
            assert!(f.edit(Edit::Add(app.id.clone()), &catalog));
        }
        assert!(!f.edit(Edit::Add(catalog[4].id.clone()), &catalog));
        assert!(!f.edit(Edit::Add(catalog[0].id.clone()), &catalog));
        assert!(f.edit(Edit::Move { from: 0, to: 3 }, &catalog));
        assert_eq!(f.applications[3], catalog[0].id);
        assert!(!f.edit(Edit::Move { from: 0, to: 4 }, &catalog));
        for _ in 0..4 {
            assert!(f.edit(Edit::Remove(0), &catalog));
        }
        assert!(f.valid() && f.applications.is_empty());
        assert!(
            !Favorites {
                applications: vec!["../bad.desktop".into()],
                fifth: None,
            }
            .valid()
        );
    }
    #[test]
    fn the_fifth_position_holds_one_distinct_application_or_the_launcher() {
        use crate::{Command, Effect, ShellModel, View};
        let catalog: Vec<_> = (0..6)
            .map(|n| Application {
                id: format!("app{n}.desktop"),
                name: format!("App {n}"),
                description: String::new(),
                icon: None,
                keywords: vec![],
                unavailable: (n == 5).then(|| "Gone".into()),
            })
            .collect();
        let mut f = Favorites::default();
        for app in &catalog[..4] {
            assert!(f.edit(Edit::Add(app.id.clone()), &catalog));
        }
        // An unavailable application cannot take the fifth position.
        assert!(!f.edit(Edit::Fifth(Some(catalog[5].id.clone())), &catalog));
        assert!(f.edit(Edit::Fifth(Some(catalog[4].id.clone())), &catalog));
        assert_eq!(f.at(FIFTH), Some(&catalog[4].id));
        assert!(!f.edit(Edit::Add(catalog[4].id.clone()), &catalog));
        // Placing a favorite there moves it out of the ordinary positions.
        assert!(f.edit(
            Edit::Place {
                id: catalog[0].id.clone(),
                at: FIFTH
            },
            &catalog
        ));
        assert_eq!(f.fifth.as_ref(), Some(&catalog[0].id));
        assert_eq!(f.applications.len(), 3);
        // And back, when there is room.
        assert!(f.edit(
            Edit::Place {
                id: catalog[0].id.clone(),
                at: 0
            },
            &catalog
        ));
        assert!(f.fifth.is_none() && f.applications[0] == catalog[0].id);
        assert!(f.edit(Edit::Fifth(Some(catalog[4].id.clone())), &catalog));
        assert!(!f.edit(
            Edit::Place {
                id: catalog[4].id.clone(),
                at: 0
            },
            &catalog
        ));
        assert!(f.valid());
        assert!(
            !Favorites {
                applications: vec!["a.desktop".into()],
                fifth: Some("a.desktop".into())
            }
            .valid()
        );
        // Tapping it launches the application; cleared, it is the Launcher again.
        let mut m = ShellModel::default();
        m.applications = catalog.clone();
        m.favorites = f.clone();
        assert_eq!(
            m.apply(Command::QuickLaunch(FIFTH)),
            vec![Effect::Launch(catalog[4].id.clone())]
        );
        let mut m = ShellModel::default();
        m.applications = catalog.clone();
        assert!(f.edit(Edit::Remove(FIFTH), &catalog));
        m.favorites = f;
        m.apply(Command::QuickLaunch(FIFTH));
        assert_eq!(m.view, View::Launcher);
    }
}
