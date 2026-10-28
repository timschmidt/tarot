//! Ordered desktop identities and shared Quick Launch hit geometry.
use crate::{Application, Layout, Rect};
use serde::{Deserialize, Serialize};

pub const CAPACITY: usize = 4;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Favorites {
    pub applications: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum Edit {
    Add(String),
    Remove(usize),
    Move { from: usize, to: usize },
}

impl Favorites {
    pub fn valid(&self) -> bool {
        self.applications.len() <= CAPACITY
            && self.applications.iter().enumerate().all(|(i, id)| {
                !id.is_empty()
                    && id.len() <= 255
                    && id.ends_with(".desktop")
                    && !id
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '/' | '\\'))
                    && !self.applications[..i].contains(id)
            })
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
        }
    }

    pub fn edit(&mut self, edit: Edit, catalog: &[Application]) -> bool {
        let mut next = self.clone();
        match edit {
            Edit::Add(id) => {
                if next.applications.len() == CAPACITY
                    || next.applications.contains(&id)
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

/// A fixed launcher position remains available even with four favorites.
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
        layout.gesture().y - crate::design::DOCK - 4.0
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
                applications: vec!["../bad.desktop".into()]
            }
            .valid()
        );
    }
}
