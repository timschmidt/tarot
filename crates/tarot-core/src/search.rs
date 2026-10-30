//! Just Type uses local snapshots. Only selecting an external target produces a URI.
use crate::{Application, ShellModel, SystemPage, View, notifications};
use serde::{Deserialize, Serialize};

pub const QUERY_CHARS: usize = 1024;
pub const QUERY_BYTES: usize = QUERY_CHARS * 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Setting {
    Wifi,
    Bluetooth,
    Audio,
    Display,
    Notifications,
    Power,
}

impl Setting {
    pub const ALL: [Self; 6] = [
        Self::Wifi,
        Self::Bluetooth,
        Self::Audio,
        Self::Display,
        Self::Notifications,
        Self::Power,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Wifi => "Wi-Fi",
            Self::Bluetooth => "Bluetooth",
            Self::Audio => "Sound",
            Self::Display => "Rotation & idle lock",
            Self::Notifications => "Notifications",
            Self::Power => "Power",
        }
    }
    pub fn detail(self) -> &'static str {
        match self {
            Self::Wifi => "Networks and saved connections",
            Self::Bluetooth => "Pair and connect Bluetooth devices",
            Self::Audio => "Volume, outputs and notification feedback",
            Self::Display => "Orientation, screen off and automatic locking",
            Self::Notifications => "Dashboard, Do Not Disturb and private previews",
            Self::Power => "Battery and session power controls",
        }
    }
    pub fn page(self) -> SystemPage {
        match self {
            Self::Wifi => SystemPage::Wifi,
            Self::Bluetooth => SystemPage::Bluetooth,
            Self::Audio => SystemPage::Audio,
            _ => SystemPage::Power,
        }
    }
    pub fn matches(self, words: &[String]) -> bool {
        let aliases = match self {
            Self::Wifi => "wifi wi fi wlan wireless internet",
            Self::Bluetooth => "bluetooth pairing paired headset headphones keyboard",
            Self::Audio => {
                "sound volume audio speaker speakers mute ringer vibrate vibration haptic feedback silent"
            }
            Self::Display => {
                "rotate rotation orientation display screen brightness idle lock automatic"
            }
            Self::Notifications => {
                "notifications notification dnd do not disturb dashboard privacy previews"
            }
            Self::Power => "power battery charge charging energy",
        };
        matches(words, [self.title(), self.detail(), aliases])
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Target {
    Application(String),
    Setting(Setting),
    Notification { id: u32, revision: u64 },
    OpenWebsite,
    DuckDuckGo,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Open,
    Close,
    Clear,
    Choose { revision: u64, target: Target },
    Move(i32),
    Submit,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub origin: Option<View>,
    pub revision: u64,
    /// Every validated selection, including selecting the same retained record again.
    pub selection_revision: u64,
    pub selected: Option<Target>,
}

impl State {
    pub fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.selected = None;
    }
}

pub fn valid_query(query: &str) -> bool {
    query.len() <= QUERY_BYTES
        && query.chars().count() <= QUERY_CHARS
        && !query.chars().any(char::is_control)
}

pub fn words(query: &str) -> Vec<String> {
    query.split_whitespace().map(str::to_lowercase).collect()
}

pub fn matches(words: &[String], parts: impl IntoIterator<Item = impl AsRef<str>>) -> bool {
    let text = parts
        .into_iter()
        .map(|s| s.as_ref().to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    words.iter().all(|word| text.contains(word))
}

pub fn application_matches(words: &[String], application: &Application) -> bool {
    matches(
        words,
        [&application.name, &application.description]
            .into_iter()
            .chain(application.keywords.iter()),
    )
}

/// A private-preview preference excludes both matching and disclosure of notification bodies.
pub fn notification_matches(
    words: &[String],
    item: &notifications::Notification,
    hidden: bool,
) -> bool {
    if hidden {
        matches(words, [&item.app_name])
    } else {
        matches(words, [&item.app_name, &item.summary, &item.body])
    }
}

pub fn targets(model: &ShellModel) -> Vec<Target> {
    let words = words(&model.launcher_query);
    let mut result = Vec::new();
    if !words.is_empty() {
        result.extend(
            model
                .applications
                .iter()
                .filter(|a| application_matches(&words, a))
                .map(|a| Target::Application(a.id.clone())),
        );
    }
    result.extend(
        Setting::ALL
            .into_iter()
            .filter(|s| s.matches(&words))
            .map(Target::Setting),
    );
    if !words.is_empty() {
        result.extend(
            model
                .notifications
                .items
                .iter()
                .filter(|n| notification_matches(&words, n, model.notifications.hide_previews))
                .map(|n| Target::Notification {
                    id: n.id,
                    revision: n.revision,
                }),
        );
        if website(&model.launcher_query).is_some() {
            result.push(Target::OpenWebsite);
        }
        result.push(Target::DuckDuckGo);
    }
    result
}

pub fn website(query: &str) -> Option<String> {
    if !valid_query(query) {
        return None;
    }
    let value = query.trim();
    if value.chars().any(char::is_whitespace) {
        return None;
    }
    let url = url::Url::parse(value).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.to_string())
}

pub fn web_search(query: &str) -> Option<String> {
    if !valid_query(query) || query.trim().is_empty() {
        return None;
    }
    let mut url = url::Url::parse("https://duckduckgo.com/").ok()?;
    // Preserve the query as one value. Shell metacharacters and URL delimiters stay data.
    url.query_pairs_mut().append_pair("q", query);
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Command, Effect, WindowId};
    fn model() -> ShellModel {
        ShellModel {
            applications: vec![Application {
                id: "org.example.Calc.desktop".into(),
                name: "Calculator".into(),
                description: "Arithmetic".into(),
                keywords: vec!["math".into()],
                icon: None,
                unavailable: None,
            }],
            ..Default::default()
        }
    }
    fn open(m: &mut ShellModel, query: &str) {
        m.apply(Command::JustType(Action::Open));
        assert!(m.just_type_active());
        assert!(
            m.apply(Command::SearchApplications(query.into()))
                .is_empty()
        );
    }
    fn choose(m: &mut ShellModel, target: Target) -> Vec<Effect> {
        m.apply(Command::JustType(Action::Choose {
            revision: m.search.revision,
            target,
        }))
    }
    #[test]
    fn entry_returns_to_its_origin_without_persisting_query_or_changing_pages() {
        for origin in [View::Home, View::Overview, View::Launcher] {
            let mut m = model();
            m.add_window(WindowId(1), "Original".into(), "original".into());
            m.view = origin;
            let pages = m.launcher.settings.clone();
            let cards = m.cards.clone();
            open(&mut m, "café");
            assert_eq!(m.search.origin, Some(origin));
            assert!(
                m.apply(Command::JustType(Action::Close))
                    .iter()
                    .all(|e| !matches!(
                        e,
                        Effect::Launcher(_) | Effect::Favorites(_) | Effect::OpenUri { .. }
                    ))
            );
            assert_eq!(m.view, origin);
            assert!(m.launcher_query.is_empty());
            assert_eq!(m.launcher.settings, pages);
            assert_eq!(m.cards, cards);
            assert!(!m.shell_text_focus);
        }
    }
    #[test]
    fn matching_is_local_and_enter_never_implicitly_submits_an_external_query() {
        let mut m = model();
        open(&mut m, "MATH");
        assert!(targets(&m).contains(&Target::Application("org.example.Calc.desktop".into())));
        assert_eq!(
            m.apply(Command::JustType(Action::Submit)),
            vec![Effect::Launch("org.example.Calc.desktop".into())]
        );
        open(&mut m, "no-local-match & λ=one");
        assert_eq!(targets(&m), [Target::DuckDuckGo]);
        assert!(m.apply(Command::JustType(Action::Submit)).is_empty());
        let effects = choose(&mut m, Target::DuckDuckGo);
        let [Effect::OpenUri { uri, .. }] = effects.as_slice() else {
            panic!("expected one explicit URI handoff")
        };
        let parsed = url::Url::parse(uri).unwrap();
        assert_eq!(parsed.host_str(), Some("duckduckgo.com"));
        assert_eq!(
            parsed.query_pairs().collect::<Vec<_>>(),
            [("q".into(), "no-local-match & λ=one".into())]
        );
        assert!(choose(&mut m, Target::DuckDuckGo).is_empty());
    }
    #[test]
    fn stale_choices_and_modal_input_cannot_launch_or_submit() {
        let mut m = model();
        open(&mut m, "arbitrary");
        let old = m.search.revision;
        m.apply(Command::SearchApplications("changed".into()));
        m.apply(Command::SearchApplications("arbitrary".into()));
        assert!(
            m.apply(Command::JustType(Action::Choose {
                revision: old,
                target: Target::DuckDuckGo
            }))
            .is_empty()
        );
        m.apply(Command::SystemMenuOpen(true));
        assert!(choose(&mut m, Target::DuckDuckGo).is_empty());
        assert_eq!(m.launcher_query, "arbitrary");
        m.apply(Command::SystemMenuOpen(false));
        m.apply(Command::JustType(Action::Move(1)));
        assert_eq!(m.search.selected, Some(Target::DuckDuckGo));
        assert!(matches!(
            m.apply(Command::JustType(Action::Submit)).as_slice(),
            [Effect::OpenUri { .. }]
        ));
    }
    #[test]
    fn selecting_an_installed_app_focuses_its_existing_card_and_settings_retain_query() {
        let mut m = model();
        m.add_window(WindowId(7), "Calculator".into(), "org.example.Calc".into());
        m.apply(Command::Overview);
        open(&mut m, "calc");
        assert_eq!(
            choose(
                &mut m,
                Target::Application("org.example.Calc.desktop".into())
            ),
            [Effect::Focus(Some(WindowId(7)))]
        );
        assert_eq!(m.view, View::App(WindowId(7)));
        assert_eq!(m.cards.len(), 1);
        assert!(m.launcher_query.is_empty());
        m.apply(Command::Overview);
        open(&mut m, "volume");
        choose(&mut m, Target::Setting(Setting::Audio));
        assert!(m.system_menu_open);
        assert_eq!(m.system_page, SystemPage::Audio);
        assert_eq!(m.launcher_query, "volume");
        assert!(!m.shell_text_focus);
    }
    #[test]
    fn notification_privacy_and_replacement_identity_are_preserved() {
        let mut m = model();
        m.notifications.items.push(notifications::Notification {
            source: 3,
            id: 3,
            revision: 8,
            app_name: "Mail".into(),
            summary: "Secret meeting".into(),
            body: "private needle".into(),
            actions: vec![notifications::Action {
                key: "default".into(),
                label: "Open".into(),
            }],
            urgent: false,
            live: true,
            orphaned: false,
        });
        m.notifications.hide_previews = true;
        open(&mut m, "needle");
        assert!(
            !targets(&m)
                .iter()
                .any(|t| matches!(t, Target::Notification { .. }))
        );
        m.notifications.hide_previews = false;
        m.apply(Command::SearchApplications("meeting".into()));
        let target = Target::Notification { id: 3, revision: 8 };
        assert!(targets(&m).contains(&target));
        m.notifications.items[0].revision = 9;
        let selection = m.search.selection_revision;
        assert!(choose(&mut m, target).is_empty());
        assert_eq!(m.search.selection_revision, selection);
        assert_eq!(
            choose(&mut m, Target::Notification { id: 3, revision: 9 }),
            [Effect::Notification(notifications::UiAction::Invoke {
                id: 3,
                revision: 9,
                key: "default".into()
            })]
        );
        assert_eq!(m.search.selection_revision, selection + 1);
        choose(&mut m, Target::Notification { id: 3, revision: 9 });
        assert_eq!(m.search.selection_revision, selection + 2);
    }
    #[test]
    fn common_setting_aliases_open_the_real_matching_controls() {
        for (query, setting) in [
            ("WiFi", Setting::Wifi),
            ("wlan", Setting::Wifi),
            ("dnd", Setting::Notifications),
            ("rotate", Setting::Display),
            ("brightness", Setting::Display),
            ("mute", Setting::Audio),
            ("charging", Setting::Power),
        ] {
            let mut m = model();
            open(&mut m, query);
            let target = Target::Setting(setting);
            assert!(targets(&m).contains(&target));
            choose(&mut m, target);
            assert_eq!(m.launcher_query, query);
            if setting == Setting::Notifications {
                assert!(m.notifications_open);
            } else {
                assert!(m.system_menu_open);
                assert_eq!(m.system_page, setting.page());
            }
        }
    }
    #[test]
    fn input_bounds_and_website_actions_reject_credentials_and_non_web_schemes() {
        for value in [
            "javascript:alert(1)",
            "file:///tmp/a",
            "https://name:secret@example.org/",
            "https://example.org/ bad",
            "https://example.org/\n",
        ] {
            assert!(website(value).is_none(), "unexpected website");
        }
        assert_eq!(
            website("https://example.org/a?b=1&c=2").as_deref(),
            Some("https://example.org/a?b=1&c=2")
        );
        let mut m = model();
        open(&mut m, "original");
        for invalid in ["a".repeat(QUERY_CHARS + 1), "hidden\ntext".into()] {
            m.apply(Command::SearchApplications(invalid));
            assert_eq!(m.launcher_query, "original");
        }
        assert!(valid_query(&"🙂".repeat(QUERY_CHARS)));
        m.apply(Command::JustType(Action::Close));
        m.apply(Command::Launcher);
        m.apply(Command::LauncherAction(crate::launcher::Action::BeginName(
            None,
        )));
        let name = m.launcher.rename.clone();
        assert!(m.apply(Command::JustType(Action::Open)).is_empty());
        assert_eq!(m.launcher.rename, name);
    }
}
