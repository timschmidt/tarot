//! Versioned top menus contain shell actions and declared capabilities only.
use crate::{CardDrop, Command, Effect, ShellModel, SystemPage, View, WindowId, launcher, search};
use serde::Serialize;
pub mod transition;
use std::time::Duration;
#[cfg(test)]
mod transition_tests;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash, Serialize)]
pub enum Panel {
    #[default]
    Root,
    Native(u64),
    Pages,
    Page(launcher::PageId),
    Arrange(WindowId),
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub revision: u64,
    pub owner: Option<View>,
    pub entry: SystemPage,
    pub history: Vec<SystemPage>,
    pub panel: Panel,
    pub selected: Option<Target>,
    pub restore_shell_text_focus: bool,
    motion: transition::Motion,
}

impl State {
    pub fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.selected = None;
        self.motion.pending = None;
    }
    pub fn open(&mut self, owner: View, entry: SystemPage) {
        self.owner = Some(owner);
        self.entry = entry;
        self.panel = Panel::Root;
        self.history.clear();
        self.restore_shell_text_focus = false;
        self.motion.open();
        self.invalidate();
    }
    pub fn dismiss(&mut self) {
        self.owner = None;
        self.panel = Panel::Root;
        self.history.clear();
        self.restore_shell_text_focus = false;
        self.motion.reset();
        self.invalidate();
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize)]
pub enum Target {
    NativePage {
        stamp: crate::app_menu::Stamp,
        id: u64,
    },
    NativeAction {
        stamp: crate::app_menu::Stamp,
        id: u64,
    },
    Page(SystemPage),
    Brightness {
        generation: u64,
        percent: u8,
    },
    Lock,
    Notifications,
    JustType,
    Launcher,
    Favorites,
    FinishFavorites,
    CancelName,
    OpenCard(WindowId),
    CloseCard(WindowId),
    Arrange(WindowId),
    Place {
        card: WindowId,
        anchor: WindowId,
        before: bool,
        inside: bool,
    },
    Unstack(WindowId),
    ClearSearch,
    CloseSearch,
    NewPage,
    Pages,
    PageActions(launcher::PageId),
    OpenPage(launcher::PageId),
    RenamePage(launcher::PageId),
    MovePage {
        page: launcher::PageId,
        at: usize,
    },
    RemovePage(launcher::PageId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    ToggleDevice,
    ToggleActivity,
    Back,
    Dismiss,
    Move(i32),
    Select(Target),
    Submit,
    Choose { revision: u64, target: Target },
}

pub struct Item {
    pub label: String,
    pub detail: String,
    pub target: Target,
    pub enabled: bool,
    pub submenu: bool,
    pub checked: bool,
}

impl Item {
    fn new(label: impl Into<String>, target: Target) -> Self {
        Self {
            label: label.into(),
            detail: String::new(),
            target,
            enabled: true,
            submenu: false,
            checked: false,
        }
    }
    fn submenu(label: impl Into<String>, target: Target) -> Self {
        Self {
            submenu: true,
            ..Self::new(label, target)
        }
    }
}

pub fn items(model: &ShellModel) -> Vec<Item> {
    use Target::*;
    if !model.system_menu_open {
        return vec![];
    }
    if model.system_page == SystemPage::Device {
        let wifi = model
            .network
            .networks
            .iter()
            .find(|n| n.connected)
            .map_or_else(
                || model.network.state.label().to_owned(),
                |n| n.label.clone(),
            );
        let sound = model.audio.default_output().map_or_else(
            || "Unavailable".into(),
            |o| {
                if o.muted {
                    "Muted".into()
                } else {
                    format!("{}%", o.percent)
                }
            },
        );
        let mut rows = vec![
            Item {
                detail: wifi,
                ..Item::submenu("Wi-Fi", Page(SystemPage::Wifi))
            },
            Item {
                detail: sound,
                ..Item::submenu("Sound", Page(SystemPage::Audio))
            },
            Item::submenu("Power & display", Page(SystemPage::Power)),
            Item::new("Notifications", Notifications),
        ];
        if model.lock_available {
            rows.push(Item::new("Lock", Lock));
        }
        return rows;
    }
    if model.system_page != SystemPage::Activity {
        return vec![];
    }
    match model.menus.panel {
        Panel::Native(id) => {
            if !model.app_menu_current() {
                return vec![];
            }
            match model.app_menu.node(id).map(|n| &n.kind) {
                Some(crate::app_menu::Kind::Submenu(nodes)) => {
                    native_items(nodes, model.app_menu.stamp)
                }
                _ => vec![],
            }
        }
        Panel::Pages => model
            .launcher
            .settings
            .pages
            .iter()
            .map(|p| Item {
                checked: p.id == model.launcher.settings.selected,
                ..Item::submenu(p.name.clone(), PageActions(p.id))
            })
            .collect(),
        Panel::Page(id) => {
            let Some(at) = model
                .launcher
                .settings
                .pages
                .iter()
                .position(|p| p.id == id)
            else {
                return vec![];
            };
            vec![
                Item::new("Open page", OpenPage(id)),
                Item::new("Rename page", RenamePage(id)),
                Item {
                    enabled: at > 0,
                    ..Item::new(
                        "Move earlier",
                        MovePage {
                            page: id,
                            at: at.saturating_sub(1),
                        },
                    )
                },
                Item {
                    enabled: at + 1 < model.launcher.settings.pages.len(),
                    ..Item::new(
                        "Move later",
                        MovePage {
                            page: id,
                            at: at + 1,
                        },
                    )
                },
                Item {
                    enabled: model.launcher.settings.pages.len() > 1,
                    ..Item::new("Remove page", RemovePage(id))
                },
            ]
        }
        Panel::Arrange(id) => {
            let Some(at) = model.cards.iter().position(|c| c.id == id && !c.closing) else {
                return vec![];
            };
            let card = &model.cards[at];
            let mut rows = vec![];
            for (label, neighbor, before) in [
                (
                    "Move left",
                    at.checked_sub(1).and_then(|i| model.cards.get(i)),
                    true,
                ),
                ("Move right", model.cards.get(at + 1), false),
            ] {
                if let Some(neighbor) = neighbor.filter(|n| !n.closing) {
                    let same = neighbor.stack == card.stack;
                    rows.push(Item::new(
                        label,
                        Place {
                            card: id,
                            anchor: neighbor.id,
                            before,
                            inside: same,
                        },
                    ));
                    if !same {
                        rows.push(Item::new(
                            format!("Stack with {}", neighbor.title),
                            Place {
                                card: id,
                                anchor: neighbor.id,
                                before: !before,
                                inside: true,
                            },
                        ));
                    }
                }
            }
            if model
                .cards
                .iter()
                .any(|c| c.id != id && c.stack == card.stack)
            {
                rows.push(Item::new("Remove from stack", Unstack(id)));
            }
            rows
        }
        Panel::Root if model.view == View::Launcher && model.editing_favorites => {
            vec![Item::new("Done editing favorites", FinishFavorites)]
        }
        Panel::Root if model.view == View::Launcher && model.launcher.rename.is_some() => {
            vec![Item::new("Cancel name editing", CancelName)]
        }
        Panel::Root if model.view == View::Launcher && model.just_type_active() => vec![
            Item::new("Clear search", ClearSearch),
            Item::new("Close Just Type", CloseSearch),
        ],
        Panel::Root if model.view == View::Launcher => vec![
            Item::new("Search applications", JustType),
            Item {
                enabled: model.launcher.settings.pages.len() < launcher::MAX_PAGES,
                ..Item::new("New page", NewPage)
            },
            Item::new("Quick Launch favorites", Favorites),
            Item::submenu("Launcher pages", Pages),
        ],
        Panel::Root => {
            let mut rows = vec![];
            if let View::App(id) = model.view {
                if model.app_menu_current() {
                    rows.extend(native_items(&model.app_menu.nodes, model.app_menu.stamp));
                }
                rows.push(Item::new("Show cards", Launcher));
                if model.cards.iter().any(|c| c.id == id && !c.closing) {
                    rows.push(Item::new("Close card", CloseCard(id)));
                }
            } else {
                rows.push(Item::new("Just type", JustType));
                rows.push(Item::new("Launcher", Launcher));
                rows.push(Item::new("Quick Launch favorites", Favorites));
                if let Some(card) = model
                    .cards
                    .iter()
                    .find(|c| Some(c.id) == model.selected && !c.closing)
                {
                    rows.push(Item::new("Open card", OpenCard(card.id)));
                    rows.push(Item::submenu("Arrange card", Arrange(card.id)));
                    rows.push(Item::new("Close card", CloseCard(card.id)));
                }
            }
            rows
        }
    }
}

fn native_items(nodes: &[crate::app_menu::Node], stamp: crate::app_menu::Stamp) -> Vec<Item> {
    use crate::app_menu::Kind;
    let mut rows = Vec::new();
    for node in nodes {
        match &node.kind {
            Kind::Section(children) => {
                if !node.label.is_empty() {
                    rows.push(Item {
                        enabled: false,
                        ..Item::new(&node.label, Target::NativePage { stamp, id: node.id })
                    });
                }
                rows.extend(native_items(children, stamp));
            }
            kind => rows.push(Item {
                label: node.label.clone(),
                detail: String::new(),
                enabled: node.enabled,
                checked: node.checked,
                submenu: matches!(kind, Kind::Submenu(_)),
                target: if matches!(kind, Kind::Submenu(_)) {
                    Target::NativePage { stamp, id: node.id }
                } else {
                    Target::NativeAction { stamp, id: node.id }
                },
            }),
        }
    }
    rows
}

impl ShellModel {
    /// Register/advance the compositor's monotonic clock. Static embedders can
    /// omit this; production compositor construction always registers it.
    pub fn advance_menus(&mut self, now: Duration) {
        self.menus.motion.track(now, self.reduced_motion);
        if !self.system_menu_open {
            self.menus.motion.reset();
        }
    }

    /// A timer reaching zero opacity does not authorize input restoration.
    /// The backend must acknowledge the matching fully retired menu frame.
    pub fn menu_retirement_token(&self) -> Option<u64> {
        let visual = self.menu_visual();
        (self.system_menu_open && visual.closing && !visual.moving).then_some(self.menus.revision)
    }

    pub fn present_menus(&mut self, revision: u64) -> Vec<Effect> {
        if self.menu_retirement_token() != Some(revision) {
            return vec![];
        }
        let pending = self.menus.motion.pending.take();
        if let Some(target) = pending
            && (self.menus.entry != SystemPage::Activity || self.menus.owner == Some(self.view))
            && items(self).iter().any(|i| i.enabled && i.target == target)
        {
            return self.finish_menu_choice(target);
        }
        self.apply(Command::SystemMenuOpen(false))
    }

    pub fn menu_visual(&self) -> transition::Visual {
        if !self.system_menu_open {
            return transition::Visual {
                opacity: 0.0,
                scrim: 0.0,
                offset_y: 0.0,
                moving: false,
                closing: false,
            };
        }
        if !self.menus.motion.clock_active() {
            return transition::Visual {
                opacity: 1.0,
                scrim: 1.0,
                offset_y: 0.0,
                moving: false,
                closing: false,
            };
        }
        self.menus.motion.visual()
    }

    /// Geometry changes retire pending choices before anything can execute in
    /// the new coordinate system. Opening panels settle; closing panels retire.
    pub fn reconfigure_menus(&mut self, now: Duration) -> Vec<Effect> {
        self.menus.motion.track(now, self.reduced_motion);
        if !self.system_menu_open {
            self.menus.motion.reset();
            return vec![];
        }
        if self.menus.motion.closing {
            return self.apply(Command::SystemMenuOpen(false));
        }
        self.menus.invalidate();
        self.menus.motion.settle_open();
        vec![]
    }

    fn close_menu(&mut self, target: Option<Target>) -> Vec<Effect> {
        if !self.system_menu_open || self.menus.motion.closing {
            return vec![];
        }
        // Lock protection starts immediately and never waits for an animation.
        if target != Some(Target::Lock) && self.menus.motion.clock_active() {
            self.menus.invalidate();
            if self.menus.motion.close(target.clone()) {
                return vec![];
            }
        }
        match target {
            Some(target) => self.finish_menu_choice(target),
            None => self.apply(Command::SystemMenuOpen(false)),
        }
    }

    fn finish_menu_choice(&mut self, target: Target) -> Vec<Effect> {
        let previous = self.view;
        let mut effects = self.apply(Command::SystemMenuOpen(false));
        if let Target::NativeAction { stamp, id } = target {
            if let View::App(owner) = previous {
                effects.push(Effect::NativeMenu { owner, stamp, id });
            }
            return effects;
        }
        let command = match target {
            Target::Lock => Command::Lock,
            Target::Notifications => Command::NotificationsOpen(true),
            Target::JustType => Command::JustType(search::Action::Open),
            Target::Launcher if matches!(previous, View::App(_)) => Command::Overview,
            Target::Launcher => Command::Launcher,
            Target::Favorites => Command::EditFavorites(true),
            Target::FinishFavorites => Command::EditFavorites(false),
            Target::CancelName => Command::LauncherAction(launcher::Action::Dismiss),
            Target::OpenCard(id) => Command::Activate(id),
            Target::CloseCard(id) => Command::Close(id),
            Target::Place {
                card,
                anchor,
                before,
                inside,
            } => Command::PlaceCard {
                card,
                target: if inside {
                    CardDrop::Into { anchor, before }
                } else {
                    CardDrop::Separate { anchor, before }
                },
            },
            Target::Unstack(id) => Command::Unstack(id),
            Target::ClearSearch => Command::JustType(search::Action::Clear),
            Target::CloseSearch => Command::JustType(search::Action::Close),
            Target::NewPage => Command::LauncherAction(launcher::Action::BeginName(None)),
            Target::OpenPage(page) => {
                Command::LauncherAction(launcher::Action::Edit(launcher::Edit::Select(page)))
            }
            Target::RenamePage(page) => {
                Command::LauncherAction(launcher::Action::BeginName(Some(page)))
            }
            _ => unreachable!(),
        };
        effects.extend(self.apply(command));
        effects
    }

    pub(super) fn menu_action(&mut self, action: Action) -> Vec<Effect> {
        use Action::*;
        if self.menus.motion.closing && !matches!(action, ToggleDevice | ToggleActivity | Dismiss) {
            return vec![];
        }
        match action {
            ToggleDevice => {
                if self.system_menu_open
                    && self.menus.entry != SystemPage::Activity
                    && !self.menus.motion.closing
                {
                    return self.close_menu(None);
                }
                let mut effects = self.cancel_network_input();
                self.system_page = SystemPage::Device;
                effects.extend(self.apply(Command::SystemMenuOpen(true)));
                effects
            }
            ToggleActivity => {
                if self.system_menu_open
                    && self.menus.entry == SystemPage::Activity
                    && !self.menus.motion.closing
                {
                    return self.close_menu(None);
                }
                let mut effects = self.cancel_network_input();
                self.system_page = SystemPage::Activity;
                effects.extend(self.apply(Command::SystemMenuOpen(true)));
                self.launcher.menu_open = self.view == View::Launcher;
                effects
            }
            Dismiss => self.close_menu(None),
            Back => {
                if !self.system_menu_open {
                    return vec![];
                }
                if self.system_page == SystemPage::Activity && self.menus.panel != Panel::Root {
                    self.menus.panel = match self.menus.panel {
                        Panel::Native(id) => {
                            self.app_menu.parent(id).map_or(Panel::Root, Panel::Native)
                        }
                        Panel::Page(_) => Panel::Pages,
                        _ => Panel::Root,
                    };
                    self.menus.invalidate();
                    vec![]
                } else if let Some(page) = self.menus.history.pop() {
                    let effects = if page != SystemPage::Wifi {
                        self.cancel_network_input()
                    } else {
                        vec![]
                    };
                    self.system_page = page;
                    self.menus.invalidate();
                    effects
                } else {
                    self.close_menu(None)
                }
            }
            Move(delta) => {
                let targets: Vec<_> = items(self)
                    .into_iter()
                    .filter(|i| i.enabled)
                    .map(|i| i.target)
                    .collect();
                if targets.is_empty() {
                    return vec![];
                }
                let at = self
                    .menus
                    .selected
                    .as_ref()
                    .and_then(|t| targets.iter().position(|i| i == t));
                let next = at.map_or_else(
                    || if delta < 0 { targets.len() - 1 } else { 0 },
                    |at| (at as i64 + i64::from(delta)).rem_euclid(targets.len() as i64) as usize,
                );
                self.menus.selected = Some(targets[next].clone());
                vec![]
            }
            Submit => self.menus.selected.clone().map_or_else(Vec::new, |target| {
                self.menu_action(Choose {
                    revision: self.menus.revision,
                    target,
                })
            }),
            Select(target) => {
                if items(self).iter().any(|i| i.enabled && i.target == target) {
                    self.menus.selected = Some(target);
                }
                vec![]
            }
            Choose { revision, target } => {
                if !self.system_menu_open
                    || (self.menus.entry == SystemPage::Activity
                        && self.menus.owner != Some(self.view))
                    || revision != self.menus.revision
                {
                    return vec![];
                }
                if let Target::Brightness {
                    generation,
                    percent,
                } = target
                {
                    if self.system_page != SystemPage::Device
                        || !self.power.enabled
                        || generation != self.power.generation
                        || !(5..=100).contains(&percent)
                        || !self.power.brightness.can_set
                        || self.power.brightness.pending_percent.is_some()
                    {
                        return vec![];
                    }
                    return vec![Effect::Power(crate::power::UiAction::Brightness {
                        percent,
                    })];
                }
                if !items(self).iter().any(|i| i.enabled && i.target == target) {
                    return vec![];
                }
                match target {
                    Target::Page(page) => self.apply(Command::SystemPage(page)),
                    Target::NativePage { stamp, id }
                        if self.app_menu_current() && stamp == self.app_menu.stamp =>
                    {
                        self.menus.panel = Panel::Native(id);
                        self.menus.invalidate();
                        vec![]
                    }
                    Target::Pages => {
                        self.menus.panel = Panel::Pages;
                        self.menus.invalidate();
                        vec![]
                    }
                    Target::PageActions(page) => {
                        self.menus.panel = Panel::Page(page);
                        self.menus.invalidate();
                        vec![]
                    }
                    Target::Arrange(id) => {
                        self.menus.panel = Panel::Arrange(id);
                        self.menus.invalidate();
                        vec![]
                    }
                    Target::MovePage { page, at } => {
                        self.menu_page_edit(launcher::Edit::Reorder { page, at }, Panel::Page(page))
                    }
                    Target::RemovePage(page) => {
                        self.menu_page_edit(launcher::Edit::Remove(page), Panel::Pages)
                    }
                    _ => self.close_menu(Some(target)),
                }
            }
        }
    }

    fn menu_page_edit(&mut self, edit: launcher::Edit, panel: Panel) -> Vec<Effect> {
        let changed = self.launcher.action(
            launcher::Action::Edit(edit),
            &self.applications,
            &mut self.launcher_query,
            &mut self.shell_text_focus,
        );
        self.menus.panel = panel;
        self.menus.invalidate();
        if changed {
            vec![Effect::Launcher(self.launcher.settings.clone())]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choose(model: &mut ShellModel, target: Target) -> Vec<Effect> {
        model.apply(Command::Menu(Action::Choose {
            revision: model.menus.revision,
            target,
        }))
    }

    #[test]
    fn native_card_menu_has_truthful_actions_and_rejects_reopened_item() {
        let mut model = ShellModel::default();
        model.add_window(WindowId(1), "Editor".into(), "editor".into());
        assert_eq!(
            model.apply(Command::Menu(Action::ToggleActivity)),
            vec![Effect::Focus(None)]
        );
        assert_eq!(
            items(&model)
                .into_iter()
                .map(|i| i.target)
                .collect::<Vec<_>>(),
            vec![Target::Launcher, Target::CloseCard(WindowId(1))]
        );
        let held = Command::Menu(Action::Choose {
            revision: model.menus.revision,
            target: Target::CloseCard(WindowId(1)),
        });
        model.apply(Command::Menu(Action::ToggleDevice));
        model.apply(Command::Menu(Action::ToggleActivity));
        assert!(model.apply(held).is_empty());
        assert!(!model.cards[0].closing);
        let effects = choose(&mut model, Target::CloseCard(WindowId(1)));
        assert!(effects.contains(&Effect::RequestClose(WindowId(1))));
        assert!(model.cards[0].closing);
        assert!(!model.system_menu_open);
    }

    #[test]
    fn client_death_retires_activity_and_does_not_focus_replacement_under_device_menu() {
        for activity in [false, true] {
            let mut model = ShellModel::default();
            model.add_window(WindowId(1), "One".into(), "one".into());
            model.add_window(WindowId(2), "Two".into(), "two".into());
            model.apply(Command::Menu(if activity {
                Action::ToggleActivity
            } else {
                Action::ToggleDevice
            }));
            let held = Command::Menu(Action::Choose {
                revision: model.menus.revision,
                target: Target::CloseCard(WindowId(2)),
            });
            assert_eq!(model.remove_window(WindowId(2)), vec![Effect::Focus(None)]);
            assert_eq!(model.view, View::App(WindowId(1)));
            assert_eq!(model.system_menu_open, !activity);
            assert!(model.menus.owner.is_none());
            assert!(model.apply(held).is_empty());
            if !activity {
                assert!(
                    !choose(&mut model, Target::Page(SystemPage::Audio))
                        .iter()
                        .any(|e| matches!(e, Effect::Focus(Some(_))))
                );
                assert_eq!(model.system_page, SystemPage::Audio);
                assert_eq!(
                    model.apply(Command::Menu(Action::Dismiss)),
                    vec![Effect::Focus(None)]
                );
            }
        }
        let mut model = ShellModel::default();
        model.add_window(WindowId(1), "One".into(), "one".into());
        model.apply(Command::Overview);
        model.apply(Command::Menu(Action::ToggleActivity));
        model.remove_window(WindowId(1));
        assert_eq!(model.view, View::Home);
        assert!(!model.system_menu_open);
    }

    #[test]
    fn submenu_back_cancels_secrets_and_restores_only_original_owner() {
        let mut model = ShellModel::default();
        model.add_window(WindowId(1), "Editor".into(), "editor".into());
        model.apply(Command::Menu(Action::ToggleDevice));
        choose(&mut model, Target::Page(SystemPage::Power));
        model.apply(Command::SystemPage(SystemPage::Wifi));
        model.network_input = Some(crate::network::Input {
            target: crate::network::InputTarget::Agent(7),
            label: "Private".into(),
            kind: crate::network::PasswordKind::Psk,
            password: crate::network::Secret::new("private-value".into()),
            remember: false,
            keyboard: true,
        });
        assert_eq!(
            model.apply(Command::Menu(Action::Back)),
            vec![Effect::Network(crate::network::UiAction::CancelSecrets {
                request: 7
            })]
        );
        assert!(model.network_input.is_none());
        assert_eq!(model.retired_secret_request, 7);
        assert_eq!(model.system_page, SystemPage::Power);
        model.apply(Command::Menu(Action::Back));
        assert_eq!(model.system_page, SystemPage::Device);
        assert_eq!(
            model.apply(Command::Menu(Action::Back)),
            vec![Effect::Focus(Some(WindowId(1)))]
        );
        assert!(!model.system_menu_open);
        model.apply(Command::Menu(Action::ToggleActivity));
        model.sync_network(crate::network::Snapshot {
            secret_prompt: Some(crate::network::SecretPrompt {
                id: 8,
                label: "Agent".into(),
                kind: crate::network::PasswordKind::Psk,
            }),
            ..Default::default()
        });
        assert_eq!(model.menus.entry, SystemPage::Wifi);
        assert!(!model.launcher.menu_open);
    }

    #[test]
    fn launcher_page_hierarchy_keeps_stable_identities_and_persistence() {
        use launcher::{Action as L, Edit};
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        let original = model.launcher.settings.selected;
        model.apply(Command::Menu(Action::ToggleActivity));
        choose(&mut model, Target::NewPage);
        assert!(model.shell_text_focus && model.launcher.rename.is_some());
        model.apply(Command::LauncherAction(L::Name("Work".into())));
        assert!(
            model
                .apply(Command::LauncherAction(L::AcceptName))
                .iter()
                .any(|e| matches!(e, Effect::Launcher(_)))
        );
        let work = model.launcher.settings.selected;
        model.apply(Command::LauncherAction(L::Edit(Edit::Place {
            application: "work.desktop".into(),
            page: work,
            at: 0,
        })));
        model.apply(Command::Menu(Action::ToggleActivity));
        choose(&mut model, Target::Pages);
        choose(&mut model, Target::PageActions(work));
        let effects = choose(&mut model, Target::MovePage { page: work, at: 0 });
        assert!(effects.iter().any(|e| matches!(e, Effect::Launcher(_))));
        assert_eq!(model.launcher.settings.pages[0].id, work);
        assert_eq!(model.launcher.settings.inbox, original);
        assert_eq!(model.menus.panel, Panel::Page(work));
        assert!(
            choose(&mut model, Target::RemovePage(work))
                .iter()
                .any(|e| matches!(e, Effect::Launcher(_)))
        );
        assert_eq!(model.menus.panel, Panel::Pages);
        choose(&mut model, Target::PageActions(original));
        assert!(choose(&mut model, Target::RemovePage(original)).is_empty());
        assert_eq!(model.launcher.settings.pages.len(), 1);
        assert!(model.launcher.settings.valid());
    }

    #[test]
    fn brightness_rejects_stale_service_and_menu_epochs_and_unsupported_values() {
        let mut model = ShellModel::default();
        model.power.enabled = true;
        model.power.generation = 4;
        model.power.brightness.can_set = true;
        assert_eq!(
            model.apply(Command::Menu(Action::ToggleDevice)),
            vec![Effect::Focus(None)]
        );
        for (generation, percent) in [(3, 50), (4, 0), (4, 101)] {
            assert!(
                choose(
                    &mut model,
                    Target::Brightness {
                        generation,
                        percent
                    }
                )
                .is_empty()
            );
        }
        assert_eq!(
            choose(
                &mut model,
                Target::Brightness {
                    generation: 4,
                    percent: 50
                }
            ),
            vec![Effect::Power(crate::power::UiAction::Brightness {
                percent: 50
            })]
        );
        let revision = model.menus.revision;
        model.power.brightness.pending_percent = Some(50);
        assert!(
            choose(
                &mut model,
                Target::Brightness {
                    generation: 4,
                    percent: 70
                }
            )
            .is_empty()
        );
        model.power.brightness.pending_percent = None;
        model.power.brightness.can_set = false;
        assert!(
            choose(
                &mut model,
                Target::Brightness {
                    generation: 4,
                    percent: 70
                }
            )
            .is_empty()
        );
        model.power.brightness.can_set = true;
        model.add_window_in_background(WindowId(1), "Background".into(), "app".into());
        model.add_window(WindowId(1), "Ticking title".into(), "app".into());
        assert_eq!(
            model.menus.revision, revision,
            "background titles must not interrupt brightness drags"
        );
        model.apply(Command::Menu(Action::ToggleDevice));
        model.apply(Command::Menu(Action::ToggleDevice));
        assert!(
            model
                .apply(Command::Menu(Action::Choose {
                    revision,
                    target: Target::Brightness {
                        generation: 4,
                        percent: 70
                    }
                }))
                .is_empty()
        );
    }

    #[test]
    fn keyboard_selection_skips_disabled_rows_and_does_not_submit_implicitly() {
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        let page = model.launcher.settings.selected;
        model.apply(Command::Menu(Action::ToggleActivity));
        choose(&mut model, Target::Pages);
        choose(&mut model, Target::PageActions(page));
        assert!(model.apply(Command::Menu(Action::Submit)).is_empty());
        model.apply(Command::Menu(Action::Select(Target::RemovePage(page))));
        assert!(model.menus.selected.is_none());
        model.apply(Command::Menu(Action::Move(-1)));
        assert_eq!(model.menus.selected, Some(Target::RenamePage(page)));
        model.apply(Command::Menu(Action::Submit));
        assert!(!model.system_menu_open);
        assert_eq!(model.launcher.rename.as_ref().unwrap().page, Some(page));
        assert!(model.shell_text_focus);
    }

    #[test]
    fn blank_just_type_requires_explicit_selection_before_enter() {
        let mut model = ShellModel::default();
        model.apply(Command::JustType(search::Action::Open));
        assert!(
            model
                .apply(Command::JustType(search::Action::Submit))
                .is_empty()
        );
        assert!(!model.system_menu_open);
        model.apply(Command::JustType(search::Action::Move(1)));
        let selected = model.search.selected.clone().unwrap();
        assert!(matches!(selected, search::Target::Setting(_)));
        model.apply(Command::JustType(search::Action::Submit));
        assert!(model.system_menu_open || model.notifications_open);
    }

    #[test]
    fn dismissal_restores_owned_search_focus_across_root_switches_but_navigation_retires_it() {
        let mut model = ShellModel::default();
        model.apply(Command::JustType(search::Action::Open));
        model.apply(Command::SearchApplications("volume".into()));
        model.apply(Command::Menu(Action::ToggleActivity));
        assert!(!model.shell_text_focus);
        model.apply(Command::Menu(Action::ToggleDevice));
        model.apply(Command::Menu(Action::Back));
        assert!(model.shell_text_focus);
        assert_eq!(model.launcher_query, "volume");
        model.apply(Command::Menu(Action::ToggleDevice));
        model.apply(Command::Home);
        model.apply(Command::Menu(Action::Dismiss));
        assert!(!model.shell_text_focus);
        assert_eq!(model.view, View::Home);
    }
}
