//! Immutable presentation data; transport addresses and parameters stay outside
//! the shell model. Every publication retires the old pressed-row identity.
use crate::{ShellModel, View, WindowId};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize)]
pub struct Stamp {
    pub source: u64,
    pub revision: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Node {
    pub id: u64,
    pub label: String,
    pub enabled: bool,
    pub checked: bool,
    pub kind: Kind,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum Kind {
    Action,
    Submenu(Vec<Node>),
    Section(Vec<Node>),
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub enum Status {
    #[default]
    Absent,
    Loading,
    Available,
    Unavailable,
    Uncertain,
}
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct State {
    pub owner: Option<WindowId>,
    pub stamp: Stamp,
    pub status: Status,
    pub nodes: Vec<Node>,
}
impl State {
    pub fn node(&self, id: u64) -> Option<&Node> {
        fn find(nodes: &[Node], id: u64) -> Option<&Node> {
            for n in nodes {
                if n.id == id {
                    return Some(n);
                }
                if let Kind::Submenu(children) | Kind::Section(children) = &n.kind
                    && let Some(found) = find(children, id)
                {
                    return Some(found);
                }
            }
            None
        }
        find(&self.nodes, id)
    }
    pub fn parent(&self, id: u64) -> Option<u64> {
        fn find(nodes: &[Node], id: u64, parent: Option<u64>) -> Option<Option<u64>> {
            for n in nodes {
                if n.id == id {
                    return Some(parent);
                }
                if let Kind::Submenu(c) | Kind::Section(c) = &n.kind
                    && let Some(v) = find(
                        c,
                        id,
                        if matches!(&n.kind, Kind::Submenu(_)) {
                            Some(n.id)
                        } else {
                            parent
                        },
                    )
                {
                    return Some(v);
                }
            }
            None
        }
        find(&self.nodes, id, None).flatten()
    }
}
impl ShellModel {
    pub fn update_app_menu(&mut self, state: State) {
        if self.app_menu == state {
            return;
        }
        self.app_menu = state;
        if self.system_menu_open && self.system_page == crate::SystemPage::Activity {
            self.menus.panel = crate::menu::Panel::Root;
            self.menus.invalidate();
        }
    }
    pub fn app_menu_current(&self) -> bool {
        matches!(self.view, View::App(id) if self.app_menu.owner==Some(id))
            && self.app_menu.status == Status::Available
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Command, Effect,
        menu::{Action, Target},
    };
    use std::time::Duration;
    fn model() -> ShellModel {
        let mut m = ShellModel::default();
        m.add_window(WindowId(1), "App".into(), "app".into());
        m.update_app_menu(State {
            owner: Some(WindowId(1)),
            stamp: Stamp {
                source: 4,
                revision: 2,
            },
            status: Status::Available,
            nodes: vec![Node {
                id: 1,
                label: "Do work".into(),
                enabled: true,
                checked: false,
                kind: Kind::Action,
            }],
        });
        m
    }
    #[test]
    fn native_choice_waits_for_matching_retirement_and_signal_retires_it() {
        let mut m = model();
        m.advance_menus(Duration::ZERO);
        m.apply(Command::Menu(Action::ToggleActivity));
        m.advance_menus(Duration::from_secs(1));
        let target = Target::NativeAction {
            stamp: m.app_menu.stamp,
            id: 1,
        };
        assert!(
            m.apply(Command::Menu(Action::Choose {
                revision: m.menus.revision,
                target
            }))
            .is_empty()
        );
        m.advance_menus(Duration::from_secs(2));
        let ticket = m.menu_retirement_token().unwrap();
        assert!(m.present_menus(ticket + 1).is_empty());
        let mut changed = m.app_menu.clone();
        changed.stamp.revision += 1;
        m.update_app_menu(changed);
        assert!(
            !m.present_menus(ticket)
                .iter()
                .any(|e| matches!(e, Effect::NativeMenu { .. }))
        );
    }
    #[test]
    fn unchanged_rows_with_new_stamp_retire_a_pressed_choice_and_device_menu_is_stable() {
        let mut m = model();
        m.apply(Command::Menu(Action::ToggleActivity));
        let revision = m.menus.revision;
        let target = Target::NativeAction {
            stamp: m.app_menu.stamp,
            id: 1,
        };
        let mut changed = m.app_menu.clone();
        changed.stamp.revision += 1;
        m.update_app_menu(changed);
        assert!(
            m.apply(Command::Menu(Action::Choose { revision, target }))
                .is_empty()
        );
        m.apply(Command::Menu(Action::ToggleDevice));
        let revision = m.menus.revision;
        let mut changed = m.app_menu.clone();
        changed.stamp.revision += 1;
        m.update_app_menu(changed);
        assert_eq!(m.menus.revision, revision);
    }
    #[test]
    fn successful_native_choice_emits_only_opaque_identity_after_presentation() {
        let mut m = model();
        m.advance_menus(Duration::ZERO);
        m.apply(Command::Menu(Action::ToggleActivity));
        m.advance_menus(Duration::from_secs(1));
        let stamp = m.app_menu.stamp;
        assert!(
            m.apply(Command::Menu(Action::Choose {
                revision: m.menus.revision,
                target: Target::NativeAction { stamp, id: 1 }
            }))
            .is_empty()
        );
        m.advance_menus(Duration::from_secs(2));
        let ticket = m.menu_retirement_token().unwrap();
        assert!(m.present_menus(ticket).contains(&Effect::NativeMenu {
            owner: WindowId(1),
            stamp,
            id: 1
        }));
        assert!(!m.system_menu_open);
    }
}
