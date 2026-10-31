//! Media players observed through MPRIS. Only display text and capabilities
//! cross into the shell; controls name a player by its observed identity.
use serde::{Deserialize, Serialize};

pub const MAX_PLAYERS: usize = 8;
pub const MAX_TEXT: usize = 160;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Status {
    Playing,
    Paused,
    #[default]
    Stopped,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Player {
    /// Stable for one exporter process; a new owner of the name gets a new id.
    pub id: u64,
    pub identity: String,
    pub title: String,
    pub artist: String,
    pub status: Status,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_go_next: bool,
    pub can_go_previous: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Action {
    PlayPause,
    /// Headset buttons send explicit play and pause, not a toggle.
    Play,
    Pause,
    Next,
    Previous,
}

impl Action {
    /// Media keys, as evdev codes plus XKB's eight-code offset.
    pub fn from_xkb(code: u32) -> Option<Self> {
        match code {
            172 => Some(Self::PlayPause),
            208 => Some(Self::Play),
            209 => Some(Self::Pause),
            171 => Some(Self::Next),
            173 => Some(Self::Previous),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub player: u64,
    pub generation: u64,
    pub action: Action,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub enabled: bool,
    pub available: bool,
    /// Increases with every observed change, so a stale control is refused.
    pub generation: u64,
    /// Most recently changed first.
    pub players: Vec<Player>,
    pub pending: Option<Control>,
    pub error: Option<String>,
}

impl Snapshot {
    /// The player shown in the dashboard: a playing one, else the most recent.
    pub fn active(&self) -> Option<&Player> {
        self.players
            .iter()
            .find(|p| p.status == Status::Playing)
            .or_else(|| self.players.first())
    }

    /// Accept one control at a time, only for a capability the player offers.
    pub fn request(&mut self, control: Control) -> bool {
        let allowed = self.available
            && self.pending.is_none()
            && control.generation == self.generation
            && self.players.iter().any(|p| {
                p.id == control.player
                    && match control.action {
                        Action::PlayPause => p.can_play || p.can_pause,
                        Action::Play => p.can_play,
                        Action::Pause => p.can_pause,
                        Action::Next => p.can_go_next,
                        Action::Previous => p.can_go_previous,
                    }
            });
        if allowed {
            self.pending = Some(control);
            self.error = None;
        }
        allowed
    }

    /// A media key controls the player the dashboard shows.
    pub fn key(&mut self, action: Action) -> Option<Control> {
        let control = Control {
            player: self.active()?.id,
            generation: self.generation,
            action,
        };
        self.request(control).then_some(control)
    }
}

/// Bidirectional overrides could reorder how a title is displayed.
fn bidi_control(c: char) -> bool {
    matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// Display text: control and bidi-override characters removed, whitespace
/// collapsed, length bounded.
pub fn clean(text: &str) -> String {
    let mut out = String::new();
    let filtered: String = text.chars().filter(|c| !bidi_control(*c)).collect();
    for word in filtered.split(|c: char| c.is_whitespace() || c.is_control()) {
        if word.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    if out.chars().count() > MAX_TEXT {
        out = out.chars().take(MAX_TEXT - 1).collect::<String>() + "…";
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(id: u64, status: Status) -> Player {
        Player {
            id,
            identity: format!("Player {id}"),
            status,
            can_play: true,
            can_pause: true,
            can_go_next: id != 2,
            ..Default::default()
        }
    }

    #[test]
    fn a_playing_player_is_preferred_over_a_more_recent_paused_one() {
        let mut snapshot = Snapshot {
            available: true,
            players: vec![player(1, Status::Paused), player(2, Status::Playing)],
            ..Default::default()
        };
        assert_eq!(snapshot.active().unwrap().id, 2);
        snapshot.players[1].status = Status::Stopped;
        assert_eq!(snapshot.active().unwrap().id, 1);
        snapshot.players.clear();
        assert!(snapshot.active().is_none());
    }

    #[test]
    fn controls_are_single_current_and_capability_checked() {
        let mut snapshot = Snapshot {
            available: true,
            generation: 4,
            players: vec![player(1, Status::Playing), player(2, Status::Paused)],
            ..Default::default()
        };
        let control = |player, generation, action| Control {
            player,
            generation,
            action,
        };
        assert!(
            !snapshot.request(control(1, 3, Action::Next)),
            "stale generation"
        );
        assert!(
            !snapshot.request(control(2, 4, Action::Next)),
            "no next capability"
        );
        assert!(
            !snapshot.request(control(9, 4, Action::PlayPause)),
            "unknown player"
        );
        assert!(snapshot.request(control(1, 4, Action::Next)));
        assert!(
            !snapshot.request(control(1, 4, Action::PlayPause)),
            "one at a time"
        );
        snapshot.pending = None;
        snapshot.available = false;
        assert!(!snapshot.request(control(1, 4, Action::PlayPause)));
    }

    #[test]
    fn text_is_single_line_and_bounded() {
        assert_eq!(clean(" A\n\tsong\u{0007}  title "), "A song title");
        let long = "x".repeat(400);
        let cleaned = clean(&long);
        assert_eq!(cleaned.chars().count(), MAX_TEXT);
        assert!(cleaned.ends_with('…'));
        assert_eq!(clean("ab\u{202e}cd\u{2066}"), "abcd");
    }

    #[test]
    fn media_keys_control_the_shown_player_by_capability() {
        assert_eq!(Action::from_xkb(208), Some(Action::Play));
        assert_eq!(Action::from_xkb(209), Some(Action::Pause));
        assert_eq!(Action::from_xkb(172), Some(Action::PlayPause));
        assert_eq!(Action::from_xkb(123), None);
        let mut snapshot = Snapshot {
            available: true,
            generation: 7,
            players: vec![player(1, Status::Paused), player(2, Status::Playing)],
            ..Default::default()
        };
        let pause = snapshot.key(Action::Pause).unwrap();
        assert_eq!((pause.player, pause.generation), (2, 7));
        assert_eq!(snapshot.key(Action::Play), None, "one control at a time");
        snapshot.pending = None;
        snapshot.players[1].can_pause = false;
        assert_eq!(snapshot.key(Action::Pause), None);
        snapshot.players.clear();
        assert_eq!(snapshot.key(Action::PlayPause), None);
    }
}
