//! Platform-independent shell state. Views emit commands; effects run after UI evaluation.

use serde::{Deserialize, Serialize};

pub mod app_menu;
pub mod audio;
pub mod authorization;
pub mod bluetooth;
pub mod deck;
pub mod design;
pub mod display;
pub mod favorite_drag;
pub mod frame;
pub mod gesture;
pub mod hardware;
pub mod icons;
pub mod idle;
pub mod launcher;
pub mod menu;
pub mod motion;
pub mod network;
pub mod notifications;
pub mod orientation;
pub mod power;
pub mod quick_launch;
pub mod search;
pub mod session;
pub mod sleep;
pub mod touch;
pub use deck::CardDrop;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct WindowId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct StackId(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum View {
    #[default]
    Home,
    App(WindowId),
    Overview,
    Launcher,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub id: WindowId,
    pub title: String,
    pub app_id: String,
    pub stack: StackId,
    pub closing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Application {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub keywords: Vec<String>,
    pub unavailable: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LaunchFeedback {
    pub generation: u64,
    pub application_id: String,
    pub message: String,
    pub pending: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KeyboardAction {
    Text(String),
    Backspace,
    Enter,
    Hide,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SystemPage {
    #[default]
    Power,
    Wifi,
    Bluetooth,
    Authorization,
    Audio,
    Device,
    Activity,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    LoadIcon(String),
    IdleTimeouts(idle::Settings),
    RotationLock(bool),
    Lock,
    Home,
    Overview,
    Launcher,
    SearchApplications(String),
    JustType(search::Action),
    Menu(menu::Action),
    LauncherAction(launcher::Action),
    ShellTextFocus(bool),
    ShellKey(KeyboardAction),
    ToggleKeyboard,
    Launch(String),
    QuickLaunch(usize),
    Favorite(quick_launch::Edit),
    EditFavorites(bool),
    DismissLaunchFeedback,
    Activate(WindowId),
    BrowseCards(i32),
    Close(WindowId),
    Stack { card: WindowId, with: WindowId },
    Unstack(WindowId),
    PlaceCard { card: WindowId, target: CardDrop },
    CancelGesture,
    CardMenuOpen(bool),
    NotificationsOpen(bool),
    Notification(notifications::UiAction),
    SystemMenuOpen(bool),
    Power(power::UiAction),
    Audio(audio::UiAction),
    Network(network::UiAction),
    Bluetooth(bluetooth::Action),
    Authorization(authorization::Action),
    SystemPage(SystemPage),
    NetworkSetup(u64),
    CredentialText(network::Secret),
    CredentialBackspace,
    CredentialKeyboard(bool),
    CredentialRemember(bool),
    CredentialSubmit,
    CredentialCancel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    NativeMenu {
        owner: WindowId,
        stamp: app_menu::Stamp,
        id: u64,
    },
    LoadIcon {
        id: String,
        icon: String,
    },
    IdleTimeouts(idle::Settings),
    RotationLock(bool),
    Lock,
    Focus(Option<WindowId>),
    RequestClose(WindowId),
    Launch(String),
    Favorites(quick_launch::Favorites),
    OpenUri {
        generation: u64,
        uri: String,
    },
    Launcher(launcher::Settings),
    ShellKey(KeyboardAction),
    ToggleKeyboard,
    Notification(notifications::UiAction),
    Power(power::UiAction),
    Audio(audio::UiAction),
    Network(network::UiAction),
    Bluetooth(bluetooth::UiAction),
    Authorization(authorization::UiAction),
}

#[derive(Clone, Debug, Default)]
pub struct ShellModel {
    pub app_menu: app_menu::State,
    pub clock_label: String,
    pub date_label: String,
    pub calendar_day: Option<u8>,
    pub view: View,
    pub cards: Vec<Card>,
    pub selected: Option<WindowId>,
    pub applications: Vec<Application>,
    pub icons: icons::State,
    pub favorites: quick_launch::Favorites,
    pub editing_favorites: bool,
    pub favorites_saved: bool,
    pub favorites_error: Option<String>,
    pub launcher_query: String,
    pub launcher: launcher::State,
    pub search: search::State,
    pub menus: menu::State,
    pub reduced_motion: bool,
    pub shell_text_focus: bool,
    pub keyboard_available: bool,
    pub lock_available: bool,
    pub card_menu_open: bool,
    pub notifications_open: bool,
    pub notifications: notifications::Snapshot,
    pub system_menu_open: bool,
    pub power: power::Snapshot,
    pub sensors: orientation::SensorSnapshot,
    pub rotation_locked: bool,
    pub display: display::Snapshot,
    pub audio: audio::Snapshot,
    pub volume_feedback: Option<audio::KeyFeedback>,
    pub network: network::Snapshot,
    pub authorization: authorization::State,
    pub bluetooth: bluetooth::Snapshot,
    pub bluetooth_ui: bluetooth::View,
    pub system_page: SystemPage,
    pub network_input: Option<network::Input>,
    retired_secret_request: u64,
    pub launch_feedback: Option<LaunchFeedback>,
    launch_generation: u64,
    next_stack: u64,
}

impl ShellModel {
    pub fn just_type_active(&self) -> bool {
        self.view == View::Launcher
            && self.launcher.search_open
            && self.launcher.rename.is_none()
            && !self.editing_favorites
    }

    fn clear_search(&mut self) {
        self.search.origin = None;
        self.search.invalidate();
        self.launcher_query.clear();
    }

    fn just_type(&mut self, action: search::Action) -> Vec<Effect> {
        use search::{Action, Target};
        if self.system_menu_open || self.editing_favorites || self.launcher.rename.is_some() {
            return Vec::new();
        }
        if action == Action::Open {
            if !matches!(self.view, View::Home | View::Overview | View::Launcher) {
                return Vec::new();
            }
            self.notifications_open = false;
            if !self.just_type_active() {
                self.search.origin = Some(self.view);
                self.search.invalidate();
                self.launcher_query.clear();
            }
            self.view = View::Launcher;
            self.launcher.search_open = true;
            self.launcher.menu_open = false;
            self.shell_text_focus = true;
            self.card_menu_open = false;
            return vec![Effect::Focus(None)];
        }
        if !self.just_type_active() {
            return Vec::new();
        }
        match action {
            Action::Open => unreachable!(),
            Action::Close => {
                let origin = self.search.origin.unwrap_or(View::Launcher);
                self.launcher.dismiss();
                self.shell_text_focus = false;
                self.view = if origin == View::Overview && self.cards.is_empty() {
                    View::Home
                } else {
                    origin
                };
                self.clear_search();
                vec![Effect::Focus(None)]
            }
            Action::Clear => {
                self.notifications_open = false;
                self.launcher_query.clear();
                self.search.invalidate();
                self.launcher.menu_open = false;
                self.shell_text_focus = true;
                Vec::new()
            }
            _ if self.launcher.menu_open => Vec::new(),
            Action::Move(delta) => {
                let targets = search::targets(self);
                if targets.is_empty() {
                    return Vec::new();
                }
                let index = self
                    .search
                    .selected
                    .as_ref()
                    .and_then(|s| targets.iter().position(|t| t == s));
                let next = index.map_or(0, |i| {
                    (i as i64 + i64::from(delta)).clamp(0, targets.len() as i64 - 1) as usize
                });
                self.search.selected = Some(targets[next].clone());
                Vec::new()
            }
            Action::Submit => {
                if self.launcher_query.trim().is_empty() && self.search.selected.is_none() {
                    return Vec::new();
                }
                // Enter can accept the first local result; a remote provider always
                // requires explicit selection, including arrow selection plus Enter.
                let target = self.search.selected.clone().or_else(|| {
                    search::targets(self)
                        .into_iter()
                        .find(|t| !matches!(t, Target::DuckDuckGo | Target::OpenWebsite))
                });
                target.map_or_else(Vec::new, |target| {
                    self.just_type(Action::Choose {
                        revision: self.search.revision,
                        target,
                    })
                })
            }
            Action::Choose { revision, target } => {
                if revision != self.search.revision || !search::targets(self).contains(&target) {
                    return Vec::new();
                }
                self.search.selection_revision = self.search.selection_revision.wrapping_add(1);
                self.search.selected = Some(target.clone());
                match target {
                    Target::Application(id) => {
                        let app_id = id.strip_suffix(".desktop").unwrap_or(&id);
                        let existing = self
                            .cards
                            .iter()
                            .rev()
                            .find(|c| !c.closing && (c.app_id == app_id || c.app_id == id))
                            .map(|c| c.id);
                        self.apply(existing.map_or(Command::Launch(id), Command::Activate))
                    }
                    Target::Setting(setting) => {
                        self.shell_text_focus = false;
                        if setting == search::Setting::Notifications {
                            self.apply(Command::NotificationsOpen(true))
                        } else {
                            self.system_page = setting.page();
                            self.apply(Command::SystemMenuOpen(true))
                        }
                    }
                    Target::Notification { id, revision } => {
                        let notification = self
                            .notifications
                            .items
                            .iter()
                            .find(|n| n.id == id && n.revision == revision)
                            .unwrap();
                        self.shell_text_focus = false;
                        if notification.live
                            && !notification.orphaned
                            && notification.actions.iter().any(|a| a.key == "default")
                        {
                            vec![Effect::Notification(notifications::UiAction::Invoke {
                                id,
                                revision,
                                key: "default".into(),
                            })]
                        } else {
                            self.apply(Command::NotificationsOpen(true))
                        }
                    }
                    Target::OpenWebsite | Target::DuckDuckGo => {
                        if self
                            .launch_feedback
                            .as_ref()
                            .is_some_and(|f| f.pending && f.application_id == "@just-type-web")
                        {
                            return Vec::new();
                        }
                        let uri = if target == Target::OpenWebsite {
                            search::website(&self.launcher_query)
                        } else {
                            search::web_search(&self.launcher_query)
                        };
                        let Some(uri) = uri else {
                            return Vec::new();
                        };
                        self.notifications_open = false;
                        self.launch_generation += 1;
                        self.launch_feedback = Some(LaunchFeedback {
                            generation: self.launch_generation,
                            application_id: "@just-type-web".into(),
                            message: "Opening the default browser…".into(),
                            pending: true,
                        });
                        vec![Effect::OpenUri {
                            generation: self.launch_generation,
                            uri,
                        }]
                    }
                }
            }
        }
    }

    pub fn cancel_network_input(&mut self) -> Vec<Effect> {
        match self.network_input.take().map(|i| i.target) {
            Some(network::InputTarget::Agent(request)) => {
                self.retired_secret_request = self.retired_secret_request.max(request);
                vec![Effect::Network(network::UiAction::CancelSecrets {
                    request,
                })]
            }
            _ => Vec::new(),
        }
    }
    pub fn cancel_platform_input(&mut self) -> Vec<Effect> {
        let mut effects = self.cancel_network_input();
        effects.extend(self.cancel_bluetooth_input());
        effects.extend(self.authorization.cancel().map(Effect::Authorization));
        effects
    }
    pub fn credential_keyboard_visible(&self) -> bool {
        (self.system_menu_open
            && self.system_page == SystemPage::Authorization
            && self.authorization.keyboard
            && self.authorization.question().is_some())
            || self.bluetooth_keyboard_visible()
            || self.system_menu_open
                && self.system_page == SystemPage::Wifi
                && self
                    .network_input
                    .as_ref()
                    .is_some_and(|i| i.keyboard && i.kind != network::PasswordKind::Open)
    }
    pub fn sync_network(&mut self, snapshot: network::Snapshot) -> Vec<Effect> {
        let valid = self
            .network_input
            .as_ref()
            .is_none_or(|input| match input.target {
                network::InputTarget::New(id) => {
                    snapshot.available
                        && snapshot.networks.iter().any(|n| n.id == id)
                        && !snapshot.busy
                }
                network::InputTarget::Agent(id) => {
                    snapshot.secret_prompt.as_ref().is_some_and(|p| p.id == id)
                }
            });
        let mut effects = if valid {
            Vec::new()
        } else {
            self.cancel_platform_input()
        };
        if let Some(prompt) = &snapshot.secret_prompt
            && prompt.id > self.retired_secret_request
            && self
                .network_input
                .as_ref()
                .is_none_or(|i| i.target != network::InputTarget::Agent(prompt.id))
        {
            effects.extend(self.cancel_platform_input());
            self.system_page = SystemPage::Wifi;
            effects.extend(self.apply(Command::SystemMenuOpen(true)));
            self.network_input = Some(network::Input {
                target: network::InputTarget::Agent(prompt.id),
                label: prompt.label.clone(),
                kind: prompt.kind,
                password: Default::default(),
                remember: false,
                keyboard: true,
            });
        }
        self.network = snapshot;
        effects
    }
    pub fn add_window(&mut self, id: WindowId, title: String, app_id: String) -> Vec<Effect> {
        if let Some(card) = self.cards.iter_mut().find(|card| card.id == id) {
            let changed = card.app_id != app_id;
            card.title = title;
            card.app_id = app_id;
            if changed && self.system_menu_open && self.menus.entry == SystemPage::Activity {
                self.menus.invalidate();
            }
            return Vec::new();
        }
        self.add_window_in_background(id, title, app_id);
        if self.system_menu_open || self.notifications_open {
            return Vec::new();
        }
        self.apply(Command::Activate(id))
    }

    pub fn add_window_in_background(&mut self, id: WindowId, title: String, app_id: String) {
        if self.cards.iter().any(|c| c.id == id) {
            self.update_window(id, title, app_id);
            return;
        }
        if self.system_menu_open && self.menus.entry == SystemPage::Activity {
            self.menus.invalidate();
        }
        let stack = self.new_stack();
        self.cards.push(Card {
            id,
            title,
            app_id,
            stack,
            closing: false,
        });
        self.launch_feedback = None;
    }

    pub fn update_window(&mut self, id: WindowId, title: String, app_id: String) {
        if let Some(card) = self.cards.iter_mut().find(|card| card.id == id) {
            let changed = card.app_id != app_id;
            card.title = title;
            card.app_id = app_id;
            if changed && self.system_menu_open && self.menus.entry == SystemPage::Activity {
                self.menus.invalidate();
            }
        }
    }

    pub fn remove_window(&mut self, id: WindowId) -> Vec<Effect> {
        let menu_owner_removed = self.system_menu_open && self.menus.owner == Some(View::App(id));
        if self.system_menu_open {
            if self.menus.entry == SystemPage::Activity || menu_owner_removed {
                self.menus.invalidate();
            }
            if self.menus.owner == Some(View::App(id)) {
                self.menus.owner = None;
                if self.menus.entry == SystemPage::Activity {
                    self.system_menu_open = false;
                    self.launcher.menu_open = false;
                    self.menus.dismiss();
                }
            }
            if self.menus.panel == menu::Panel::Arrange(id) {
                self.menus.panel = menu::Panel::Root;
            }
        }
        let old_index = self
            .cards
            .iter()
            .position(|card| card.id == id)
            .unwrap_or(0);
        let old_stack = self
            .cards
            .iter()
            .find(|card| card.id == id)
            .map(|card| card.stack);
        self.cards.retain(|card| card.id != id);
        if self.selected == Some(id) {
            self.selected = self
                .cards
                .iter()
                .find(|card| Some(card.stack) == old_stack)
                .or_else(|| {
                    self.cards
                        .get(old_index.min(self.cards.len().saturating_sub(1)))
                })
                .map(|card| card.id);
        }
        if self.view == View::App(id) {
            if let Some(next) = self.selected {
                self.view = View::App(next);
                return vec![Effect::Focus(if menu_owner_removed {
                    None
                } else {
                    Some(next)
                })];
            }
            self.view = View::Home;
            return vec![Effect::Focus(None)];
        }
        if self.cards.is_empty() && self.view == View::Overview {
            self.view = View::Home;
            if self.system_menu_open && self.menus.entry == SystemPage::Activity {
                self.system_menu_open = false;
                self.menus.dismiss();
                self.launcher.menu_open = false;
            }
        }
        Vec::new()
    }

    pub fn apply(&mut self, command: Command) -> Vec<Effect> {
        if (self.network_input.is_some()
            || self.bluetooth.prompt.is_some()
            || self.authorization.snapshot.request.is_some())
            && matches!(
                command,
                Command::Home
                    | Command::Overview
                    | Command::Launcher
                    | Command::Activate(_)
                    | Command::Launch(_)
                    | Command::QuickLaunch(_)
                    | Command::EditFavorites(_)
            )
        {
            let mut effects = self.cancel_platform_input();
            effects.extend(self.apply(command));
            return effects;
        }
        match command {
            Command::JustType(action) => return self.just_type(action),
            Command::Menu(action) => return self.menu_action(action),
            Command::LoadIcon(id) => {
                if let Some(icon) = self
                    .applications
                    .iter()
                    .find(|a| a.id == id)
                    .and_then(|a| a.icon.as_ref())
                    .filter(|s| !s.is_empty() && s.len() <= 4096)
                    && self.icons.begin(&id)
                {
                    return vec![Effect::LoadIcon {
                        id,
                        icon: icon.clone(),
                    }];
                }
                return Vec::new();
            }
            Command::EditFavorites(open) => {
                self.clear_search();
                self.editing_favorites = open;
                self.view = View::Launcher;
                self.shell_text_focus = false;
                self.launcher.dismiss();
            }
            Command::Favorite(edit) => {
                return if self.favorites.edit(edit, &self.applications) {
                    vec![Effect::Favorites(self.favorites.clone())]
                } else {
                    Vec::new()
                };
            }
            Command::QuickLaunch(index) => {
                if index > quick_launch::CAPACITY || self.system_menu_open || self.card_menu_open {
                    return Vec::new();
                }
                if index == quick_launch::CAPACITY {
                    return self.apply(Command::Launcher);
                }
                let Some(id) = self.favorites.applications.get(index) else {
                    return self.apply(Command::EditFavorites(true));
                };
                let app_id = id.strip_suffix(".desktop").unwrap_or(id);
                let matching = self
                    .cards
                    .iter()
                    .rev()
                    .filter(|c| !c.closing && (c.app_id == app_id || c.app_id == *id));
                let selected = matching
                    .clone()
                    .find(|c| Some(c.id) == self.selected)
                    .or_else(|| matching.into_iter().next())
                    .map(|c| c.id);
                return self.apply(if let Some(window) = selected {
                    Command::Activate(window)
                } else {
                    Command::Launch(id.clone())
                });
            }
            Command::NetworkSetup(id) => {
                if self.system_menu_open
                    && self.system_page == SystemPage::Wifi
                    && !self.network.busy
                    && self.network.create_permission.allowed()
                    && let Some(row) = self.network.networks.iter().find(|r| r.id == id)
                    && let Some(kind) = row.password_kind
                {
                    self.network_input = Some(network::Input {
                        target: network::InputTarget::New(id),
                        label: row.label.clone(),
                        kind,
                        password: Default::default(),
                        remember: true,
                        keyboard: kind != network::PasswordKind::Open,
                    });
                }
                return Vec::new();
            }
            Command::CredentialText(text) => {
                if let Some(input) = &mut self.network_input {
                    input.password.append(&text);
                }
                return Vec::new();
            }
            Command::CredentialBackspace => {
                if let Some(input) = &mut self.network_input {
                    input.password.backspace();
                }
                return Vec::new();
            }
            Command::CredentialKeyboard(show) => {
                if let Some(input) = &mut self.network_input {
                    input.keyboard = show;
                }
                return Vec::new();
            }
            Command::CredentialRemember(remember) => {
                if let Some(input) = &mut self.network_input {
                    input.remember = remember;
                }
                return Vec::new();
            }
            Command::CredentialSubmit => {
                if self
                    .network_input
                    .as_ref()
                    .is_some_and(|i| i.kind.accepts(&i.password))
                {
                    let input = self.network_input.take().unwrap();
                    if let network::InputTarget::Agent(id) = input.target {
                        self.retired_secret_request = self.retired_secret_request.max(id);
                    }
                    return vec![Effect::Network(match input.target {
                        network::InputTarget::New(network) => network::UiAction::Create {
                            network,
                            password: input.password,
                            remember: input.remember,
                        },
                        network::InputTarget::Agent(request) => network::UiAction::Secrets {
                            request,
                            password: input.password,
                        },
                    })];
                }
                return Vec::new();
            }
            Command::CredentialCancel => return self.cancel_platform_input(),
            Command::SystemMenuOpen(open) => {
                let was_open = self.system_menu_open;
                let restore_owner = !was_open || self.menus.owner == Some(self.view);
                let restore_text = self.shell_text_focus
                    || was_open && restore_owner && self.menus.restore_shell_text_focus;
                let canceled = if !open {
                    self.cancel_platform_input()
                } else {
                    Vec::new()
                };
                self.system_menu_open = open;
                self.notifications_open = false;
                self.card_menu_open = false;
                if open {
                    self.shell_text_focus = false;
                    self.launcher.menu_open =
                        self.system_page == SystemPage::Activity && self.view == View::Launcher;
                    self.menus.open(self.view, self.system_page);
                    self.menus.restore_shell_text_focus = restore_text;
                } else {
                    self.launcher.menu_open = false;
                    self.shell_text_focus = restore_owner
                        && restore_text
                        && self.view == View::Launcher
                        && !self.editing_favorites
                        && (self.just_type_active() || self.launcher.rename.is_some());
                    self.menus.dismiss();
                }
                let mut effects = canceled;
                effects.push(Effect::Focus(
                    if !open
                        && restore_owner
                        && let View::App(id) = self.view
                    {
                        Some(id)
                    } else {
                        None
                    },
                ));
                return effects;
            }
            Command::Lock => {
                return if self.lock_available {
                    vec![Effect::Lock]
                } else {
                    Vec::new()
                };
            }
            Command::Power(action) => return vec![Effect::Power(action)],
            Command::IdleTimeouts(settings) => {
                return vec![Effect::IdleTimeouts(settings)];
            }
            Command::RotationLock(locked) => {
                self.rotation_locked = locked;
                return vec![Effect::RotationLock(locked)];
            }
            Command::Audio(action) => return vec![Effect::Audio(action)],
            Command::Network(action) => return vec![Effect::Network(action)],
            Command::Authorization(action) => {
                if self.system_menu_open && self.system_page == SystemPage::Authorization {
                    return self
                        .authorization
                        .apply(action)
                        .into_iter()
                        .map(Effect::Authorization)
                        .collect();
                }
                return Vec::new();
            }
            Command::Bluetooth(action) => return self.bluetooth_action(action),
            Command::SystemPage(page) => {
                let effects = if page != SystemPage::Wifi {
                    self.cancel_platform_input()
                } else {
                    let mut effects = self.cancel_bluetooth_input();
                    effects.extend(self.authorization.cancel().map(Effect::Authorization));
                    effects
                };
                if self.system_menu_open && self.system_page != page {
                    if self.menus.history.len() >= 8 {
                        self.menus.history.remove(1);
                    }
                    self.menus.history.push(self.system_page);
                }
                self.system_page = page;
                self.menus.invalidate();
                return effects;
            }
            Command::NotificationsOpen(open) => {
                let canceled = self.cancel_platform_input();
                self.system_menu_open = false;
                self.menus.dismiss();
                self.notifications_open = open;
                self.card_menu_open = false;
                if open {
                    self.shell_text_focus = false;
                    self.launcher.menu_open = false;
                }
                let mut effects = canceled;
                effects.push(Effect::Focus(if let View::App(id) = self.view {
                    Some(id)
                } else {
                    None
                }));
                return effects;
            }
            Command::Notification(action) => return vec![Effect::Notification(action)],
            Command::CancelGesture => return Vec::new(),
            Command::CardMenuOpen(open) => {
                self.card_menu_open = open && self.view == View::Overview && !self.system_menu_open;
                if self.card_menu_open {
                    self.notifications_open = false;
                }
                return Vec::new();
            }
            Command::ToggleKeyboard => {
                if matches!(self.view, View::Home | View::Overview) {
                    return self.just_type(search::Action::Open);
                }
                if self.view == View::Launcher && !self.system_menu_open && !self.launcher.menu_open
                {
                    if self.shell_text_focus {
                        self.shell_text_focus = false;
                    } else if self.launcher.rename.is_some() {
                        self.shell_text_focus = true;
                    } else {
                        self.launcher.action(
                            launcher::Action::Search(true),
                            &self.applications,
                            &mut self.launcher_query,
                            &mut self.shell_text_focus,
                        );
                    }
                    if self.shell_text_focus {
                        self.notifications_open = false;
                    }
                    return Vec::new();
                }
                return if matches!(self.view, View::App(_)) && self.keyboard_available {
                    vec![Effect::ToggleKeyboard]
                } else {
                    Vec::new()
                };
            }
            Command::ShellTextFocus(focused) => {
                self.shell_text_focus = focused
                    && self.view == View::Launcher
                    && !self.system_menu_open
                    && !self.launcher.menu_open;
                if self.shell_text_focus {
                    self.notifications_open = false;
                    if self.launcher.rename.is_none() {
                        self.launcher.search_open = true;
                    }
                }
                return Vec::new();
            }
            Command::ShellKey(action) => {
                return if self.shell_text_focus && self.view == View::Launcher {
                    vec![Effect::ShellKey(action)]
                } else {
                    Vec::new()
                };
            }
            Command::Home => self.view = View::Home,
            Command::Overview => {
                self.view = if self.cards.is_empty() {
                    View::Home
                } else {
                    View::Overview
                }
            }
            Command::Launcher => {
                let keep_search = self.view == View::Launcher
                    && self.launcher.search_open
                    && self.launcher.rename.is_none()
                    && !self.editing_favorites;
                self.view = View::Launcher;
                self.editing_favorites = false;
                if keep_search {
                    self.launcher.menu_open = false;
                } else {
                    self.launcher.dismiss();
                    self.shell_text_focus = false;
                }
            }
            Command::LauncherAction(action) => {
                if self.view == View::Launcher
                    && let launcher::Action::Menu(open) = action
                {
                    return if open {
                        if self.system_menu_open && self.system_page == SystemPage::Activity {
                            Vec::new()
                        } else {
                            self.menu_action(menu::Action::ToggleActivity)
                        }
                    } else {
                        self.apply(Command::SystemMenuOpen(false))
                    };
                }
                if self.view == View::Launcher
                    && self.system_menu_open
                    && self.system_page == SystemPage::Activity
                    && action == launcher::Action::Dismiss
                {
                    return self.menu_action(menu::Action::Back);
                }
                if self.view == View::Launcher
                    && self.system_menu_open
                    && self.system_page == SystemPage::Activity
                    && matches!(
                        action,
                        launcher::Action::Search(_)
                            | launcher::Action::BeginName(_)
                            | launcher::Action::Edit(_)
                    )
                {
                    let mut effects = self.apply(Command::SystemMenuOpen(false));
                    effects.extend(self.apply(Command::LauncherAction(action)));
                    return effects;
                }
                if self.view != View::Launcher || self.system_menu_open || self.editing_favorites {
                    return Vec::new();
                }
                if action == launcher::Action::Search(true) {
                    return self.just_type(search::Action::Open);
                }
                if self.just_type_active() {
                    if action == launcher::Action::Search(false)
                        || (action == launcher::Action::Dismiss && !self.launcher.menu_open)
                    {
                        return self.just_type(search::Action::Close);
                    }
                    if action == launcher::Action::Dismiss {
                        self.launcher.menu_open = false;
                        return Vec::new();
                    }
                }
                let changed = self.launcher.action(
                    action,
                    &self.applications,
                    &mut self.launcher_query,
                    &mut self.shell_text_focus,
                );
                if self.shell_text_focus {
                    self.notifications_open = false;
                }
                return if changed {
                    vec![Effect::Launcher(self.launcher.settings.clone())]
                } else {
                    Vec::new()
                };
            }
            Command::SearchApplications(query) => {
                if !search::valid_query(&query) {
                    return Vec::new();
                }
                if self.launcher_query != query {
                    self.search.invalidate();
                }
                self.launcher_query = query;
                if self.view == View::Launcher {
                    self.launcher.search_open = true;
                }
                return Vec::new();
            }
            Command::DismissLaunchFeedback => {
                self.launch_feedback = None;
                return Vec::new();
            }
            Command::Launch(id) => {
                if self
                    .launch_feedback
                    .as_ref()
                    .is_some_and(|feedback| feedback.pending && feedback.application_id == id)
                {
                    return Vec::new();
                }
                let Some(application) = self.applications.iter().find(|app| app.id == id) else {
                    return Vec::new();
                };
                self.notifications_open = false;
                self.launch_generation += 1;
                self.launch_feedback = Some(LaunchFeedback {
                    generation: self.launch_generation,
                    application_id: id.clone(),
                    message: application
                        .unavailable
                        .clone()
                        .unwrap_or_else(|| format!("Opening {}…", application.name)),
                    pending: application.unavailable.is_none(),
                });
                return if application.unavailable.is_none() {
                    vec![Effect::Launch(id)]
                } else {
                    Vec::new()
                };
            }
            Command::Activate(id) => {
                let Some(card) = self.cards.iter_mut().find(|card| card.id == id) else {
                    return Vec::new();
                };
                // The client may have canceled its close or displayed a prompt.
                card.closing = false;
                self.selected = Some(id);
                self.view = View::App(id);
                self.shell_text_focus = false;
                self.card_menu_open = false;
                self.notifications_open = false;
                self.system_menu_open = false;
                self.launcher.dismiss();
                self.clear_search();
                return vec![Effect::Focus(Some(id))];
            }
            Command::BrowseCards(steps) => {
                if self.view != View::Overview || self.cards.is_empty() {
                    return Vec::new();
                }
                let index = self
                    .cards
                    .iter()
                    .position(|card| Some(card.id) == self.selected)
                    .unwrap_or(0);
                let next = (index as i64 + i64::from(steps)).clamp(0, self.cards.len() as i64 - 1)
                    as usize;
                self.selected = Some(self.cards[next].id);
            }
            Command::Close(id) => {
                if let Some(card) = self
                    .cards
                    .iter_mut()
                    .find(|card| card.id == id && !card.closing)
                {
                    card.closing = true;
                    return vec![Effect::RequestClose(id)];
                }
                return Vec::new();
            }
            Command::Stack { card, with } => {
                self.place_card(
                    card,
                    CardDrop::Into {
                        anchor: with,
                        before: false,
                    },
                );
                return Vec::new();
            }
            Command::Unstack(id) => {
                if let Some(anchor) = self.cards.iter().find(|c| c.id == id).and_then(|card| {
                    self.cards
                        .iter()
                        .find(|c| c.stack == card.stack && c.id != id)
                        .map(|c| c.id)
                }) {
                    self.place_card(
                        id,
                        CardDrop::Separate {
                            anchor,
                            before: false,
                        },
                    );
                }
                return Vec::new();
            }
            Command::PlaceCard { card, target } => {
                self.place_card(card, target);
                return Vec::new();
            }
        }
        if self.view != View::Launcher {
            self.editing_favorites = false;
            self.launcher.dismiss();
            self.shell_text_focus = false;
            self.clear_search();
        }
        if self.view != View::Overview {
            self.card_menu_open = false;
        }
        self.notifications_open = false;
        self.system_menu_open = false;
        self.menus.dismiss();
        vec![Effect::Focus(None)]
    }

    fn new_stack(&mut self) -> StackId {
        self.next_stack += 1;
        StackId(self.next_stack)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.width && y < self.y + self.height
    }

    pub fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }

    pub fn translated(self, x: f32, y: f32) -> Self {
        Self {
            x: self.x + x,
            y: self.y + y,
            ..self
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    pub keyboard_height: f32,
    /// Shell-owned notification space immediately above the keyboard or strip.
    pub notification_height: f32,
}

impl Layout {
    pub const STATUS: f32 = design::STATUS;
    pub const GESTURE: f32 = design::GESTURE;
    pub const DOCK: f32 = design::DOCK;

    pub fn content(self) -> Rect {
        Rect {
            x: 0.0,
            y: Self::STATUS,
            width: self.width.max(1.0),
            height: (self.notification_well().y - Self::STATUS).max(1.0),
        }
    }

    pub fn with_notifications(mut self, model: &ShellModel) -> Self {
        self.notification_height = notifications::reserved_height(model, self);
        self
    }

    pub fn notification_well(self) -> Rect {
        let bottom = self.keyboard().y;
        let height = self
            .notification_height
            .max(0.0)
            .min((bottom - Self::STATUS - 1.0).max(0.0));
        Rect {
            x: 0.0,
            y: bottom - height,
            width: self.width.max(1.0),
            height,
        }
    }

    /// Bottom edge of app content, immediately above the notification well.
    pub fn foreground_bottom(self) -> f32 {
        self.notification_well().y
    }

    /// A favorite hold may hide the keyboard without moving its tray targets.
    pub fn tray_bottom(self) -> f32 {
        self.gesture().y - self.notification_well().height
    }

    pub fn gesture(self) -> Rect {
        Rect {
            x: 0.0,
            y: (self.height - Self::GESTURE).max(0.0),
            width: self.width,
            height: Self::GESTURE,
        }
    }
    pub fn volume_feedback(self) -> Rect {
        let width = (self.width - 24.0).clamp(1.0, 260.0);
        Rect {
            x: (self.width - width) / 2.0,
            y: Self::STATUS + 12.0,
            width,
            height: 64.0,
        }
    }
    pub fn launch_feedback(self) -> Rect {
        Rect {
            x: 12.0,
            y: (self.gesture().y - self.keyboard_height - Self::DOCK - 76.0).max(Self::STATUS),
            width: (self.width - 24.0).max(1.0),
            height: 64.0,
        }
    }

    pub fn keyboard(self) -> Rect {
        Rect {
            x: 0.0,
            y: (self.height - Self::GESTURE - self.keyboard_height).max(Self::STATUS),
            width: self.width,
            height: self.keyboard_height,
        }
    }

    pub fn card(self, index: usize, selected_index: usize) -> Rect {
        let content = self.content();
        let scale = design::CARD_SCALE.min(
            ((self.height - Self::STATUS - Self::GESTURE - Self::DOCK - 24.0) / content.height)
                .max(0.1),
        );
        let width = content.width * scale;
        let height = content.height * scale;
        Rect {
            x: (self.width - width) / 2.0 + (index as f32 - selected_index as f32) * (width + 18.0),
            y: (self.height * design::CARD_Y / 720.0)
                .min(self.tray_bottom() - Self::DOCK - 12.0 - height)
                .max(Self::STATUS + 12.0),
            width,
            height,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authentication_modal_dismissal_retires_entry_and_restores_the_origin() {
        use authorization::{Action, Challenge, Phase, Request, Response, Snapshot, UiAction};
        let mut m = model();
        let origin = m.selected;
        m.apply(Command::SystemPage(SystemPage::Authorization));
        assert!(
            m.apply(Command::SystemMenuOpen(true))
                .contains(&Effect::Focus(None))
        );
        let snapshot = Snapshot {
            enabled: true,
            available: true,
            request: Some(Request {
                id: 7,
                action: "org.tarot.Test".into(),
                message: "Test authentication".into(),
                identities: Vec::new(),
                selected: None,
                phase: Phase::Question(Challenge {
                    id: 1,
                    prompt: "Password:".into(),
                    echo: false,
                }),
                info: None,
            }),
            ..Default::default()
        };
        assert!(m.authorization.sync(snapshot.clone(), true).is_empty());
        m.apply(Command::Authorization(Action::Text {
            request: 7,
            turn: 1,
            text: Response::new("synthetic-secret".into()).unwrap(),
        }));
        assert!(m.credential_keyboard_visible());
        let effects = m.apply(Command::SystemMenuOpen(false));
        assert!(effects.contains(&Effect::Authorization(UiAction::Cancel { request: 7 })));
        assert!(effects.contains(&Effect::Focus(origin)));
        assert!(m.authorization.input.expose().is_empty());
        assert!(!m.credential_keyboard_visible());
        assert!(!m.authorization.may_open(7));
        m.authorization.sync(snapshot, true);
        assert!(m.authorization.snapshot.request.is_none());
        assert!(
            m.apply(Command::Authorization(Action::Submit {
                request: 7,
                turn: 1
            }))
            .is_empty()
        );
        assert_eq!(m.selected, origin);
    }
    #[test]
    fn credential_cancel_and_submission_cannot_reopen_a_retired_request() {
        let mut m = model();
        let selected = m.selected;
        let snapshot = network::Snapshot {
            available: true,
            secret_prompt: Some(network::SecretPrompt {
                id: 3,
                label: "Fixture".into(),
                kind: network::PasswordKind::Psk,
            }),
            ..Default::default()
        };
        m.sync_network(snapshot.clone());
        assert!(m.credential_keyboard_visible());
        assert_eq!(m.selected, selected);
        m.apply(Command::CredentialText(network::Secret::new(
            "password".into(),
        )));
        let effects = m.apply(Command::CredentialSubmit);
        assert!(matches!(
            &effects[..],
            [Effect::Network(network::UiAction::Secrets {
                request: 3,
                ..
            })]
        ));
        m.sync_network(snapshot);
        assert!(m.network_input.is_none());
        let next = network::Snapshot {
            available: true,
            secret_prompt: Some(network::SecretPrompt {
                id: 4,
                label: "Fixture".into(),
                kind: network::PasswordKind::Psk,
            }),
            ..Default::default()
        };
        m.sync_network(next.clone());
        m.apply(Command::CredentialText(network::Secret::new(
            "password".into(),
        )));
        assert!(
            m.apply(Command::SystemMenuOpen(false))
                .iter()
                .any(|e| matches!(
                    e,
                    Effect::Network(network::UiAction::CancelSecrets { request: 4 })
                ))
        );
        m.sync_network(next);
        assert!(m.network_input.is_none());
        assert_eq!(m.selected, selected);
    }

    fn model() -> ShellModel {
        let mut model = ShellModel::default();
        model.add_window(WindowId(1), "One".into(), "test".into());
        model.add_window(WindowId(2), "Two".into(), "test".into());
        model
    }

    #[test]
    fn browsing_cards_keeps_overview_and_clamps_at_both_ends() {
        let mut model = model();
        assert!(model.apply(Command::BrowseCards(-1)).is_empty());
        assert_eq!(model.view, View::App(WindowId(2)));
        model.apply(Command::Overview);
        assert_eq!(
            model.apply(Command::BrowseCards(i32::MIN)),
            vec![Effect::Focus(None)]
        );
        assert_eq!(model.selected, Some(WindowId(1)));
        model.apply(Command::BrowseCards(i32::MAX));
        assert_eq!(model.selected, Some(WindowId(2)));
        assert_eq!(model.view, View::Overview);
    }

    #[test]
    fn returning_to_a_pending_close_allows_the_client_prompt_and_another_close() {
        let mut model = model();
        model.apply(Command::Close(WindowId(2)));
        assert!(model.cards[1].closing);
        model.apply(Command::Activate(WindowId(2)));
        assert!(!model.cards[1].closing);
        assert_eq!(
            model.apply(Command::Close(WindowId(2))),
            vec![Effect::RequestClose(WindowId(2))]
        );
    }

    #[test]
    fn close_is_a_single_request_and_retains_card_until_client_exits() {
        let mut model = model();
        assert_eq!(
            model.apply(Command::Close(WindowId(1))),
            vec![Effect::RequestClose(WindowId(1))]
        );
        assert!(model.apply(Command::Close(WindowId(1))).is_empty());
        assert_eq!(model.cards.len(), 2);
        model.remove_window(WindowId(1));
        assert_eq!(model.cards.len(), 1);
    }

    #[test]
    fn closing_active_window_selects_remaining_window_then_home() {
        let mut model = model();
        assert_eq!(
            model.remove_window(WindowId(2)),
            vec![Effect::Focus(Some(WindowId(1)))]
        );
        assert_eq!(model.view, View::App(WindowId(1)));
        assert_eq!(model.remove_window(WindowId(1)), vec![Effect::Focus(None)]);
        assert_eq!(model.view, View::Home);
    }

    #[test]
    fn stale_activation_cannot_focus_a_missing_client() {
        let mut model = model();
        let before = model.view;
        assert!(model.apply(Command::Activate(WindowId(999))).is_empty());
        assert_eq!(model.view, before);
    }

    #[test]
    fn same_application_windows_are_independent_cards() {
        let mut model = model();
        model.apply(Command::Overview);
        assert_ne!(model.cards[0].stack, model.cards[1].stack);
        model.apply(Command::Stack {
            card: WindowId(1),
            with: WindowId(2),
        });
        assert_eq!(model.cards[0].stack, model.cards[1].stack);
        model.remove_window(WindowId(2));
        assert_eq!(model.cards[0].id, WindowId(1));
    }

    #[test]
    fn geometry_keeps_gesture_strip_out_of_app() {
        let layout = Layout {
            width: 360.0,
            height: 720.0,
            notification_height: 0.0,
            keyboard_height: 0.0,
        };
        assert!(!layout.content().contains(180.0, 710.0));
        assert!(layout.gesture().contains(180.0, 710.0));
    }

    #[test]
    fn keyboard_reserves_app_space_and_leaves_the_gesture_strip_accessible() {
        let mut layout = Layout {
            width: 360.0,
            height: 720.0,
            notification_height: 0.0,
            keyboard_height: 240.0,
        };
        let app = layout.content();
        let keyboard = layout.keyboard();
        assert_eq!(app.y + app.height, keyboard.y);
        assert_eq!(keyboard.y + keyboard.height, layout.gesture().y);
        assert!(!app.contains(180.0, 600.0));
        assert!(keyboard.contains(180.0, 600.0));
        assert!(!keyboard.contains(180.0, 710.0));
        layout.keyboard_height = 0.0;
        assert_eq!(layout.content().height, 652.0);
        assert_eq!(layout.gesture().y, 684.0);
    }

    #[test]
    fn pending_launch_is_deduplicated_and_failed_launch_can_be_retried() {
        let mut model = ShellModel::default();
        model.applications.push(Application {
            id: "example.desktop".into(),
            name: "Example".into(),
            description: String::new(),
            icon: None,
            keywords: Vec::new(),
            unavailable: None,
        });
        assert_eq!(
            model.apply(Command::Launch("example.desktop".into())),
            vec![Effect::Launch("example.desktop".into())]
        );
        assert!(
            model
                .apply(Command::Launch("example.desktop".into()))
                .is_empty()
        );
        model.launch_feedback.as_mut().unwrap().pending = false;
        assert_eq!(
            model.apply(Command::Launch("example.desktop".into())),
            vec![Effect::Launch("example.desktop".into())]
        );
    }

    #[test]
    fn a_new_card_cannot_dismiss_a_modal_or_replace_its_underlying_selection() {
        for system in [true, false] {
            for existing in [true, false] {
                let mut model = ShellModel::default();
                if existing {
                    model.add_window(WindowId(1), "One".into(), "app".into());
                }
                let before = (model.view, model.selected);
                let open = if system {
                    Command::SystemMenuOpen(true)
                } else {
                    Command::NotificationsOpen(true)
                };
                model.apply(open);
                assert!(
                    model
                        .add_window(WindowId(2), "Two".into(), "other".into())
                        .is_empty()
                );
                assert_eq!((model.view, model.selected), before);
                assert_eq!(model.system_menu_open, system);
                assert_eq!(model.notifications_open, !system);
                let close = if system {
                    Command::SystemMenuOpen(false)
                } else {
                    Command::NotificationsOpen(false)
                };
                assert_eq!(model.apply(close), vec![Effect::Focus(before.1)]);
                assert_eq!(
                    model.apply(Command::Activate(WindowId(2))),
                    vec![Effect::Focus(Some(WindowId(2)))]
                );
            }
        }
    }

    #[test]
    fn system_controls_are_modal_and_restore_the_same_native_app() {
        let mut model = ShellModel::default();
        model.add_window(WindowId(1), "Example".into(), "example".into());
        model.apply(Command::NotificationsOpen(true));
        let cards = model.cards.clone();
        assert_eq!(
            model.apply(Command::SystemMenuOpen(true)),
            vec![Effect::Focus(None)]
        );
        assert!(model.system_menu_open && !model.notifications_open);
        let action = power::UiAction::Brightness { percent: 40 };
        assert_eq!(
            model.apply(Command::Power(action.clone())),
            vec![Effect::Power(action)]
        );
        assert_eq!(model.view, View::App(WindowId(1)));
        assert_eq!(model.cards, cards);
        assert_eq!(
            model.apply(Command::SystemMenuOpen(false)),
            vec![Effect::Focus(Some(WindowId(1)))]
        );
        model.apply(Command::SystemMenuOpen(true));
        model.apply(Command::Overview);
        assert!(!model.system_menu_open);
    }

    #[test]
    fn unavailable_application_reports_reason_without_a_spawn_effect() {
        let mut model = ShellModel::default();
        model.applications.push(Application {
            id: "example.desktop".into(),
            name: "Example".into(),
            description: String::new(),
            icon: None,
            keywords: Vec::new(),
            unavailable: Some("Executable missing".into()),
        });
        assert!(
            model
                .apply(Command::Launch("example.desktop".into()))
                .is_empty()
        );
        let feedback = model.launch_feedback.unwrap();
        assert!(!feedback.pending);
        assert_eq!(feedback.message, "Executable missing");
    }
}
