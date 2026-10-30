//! Polkit presentation state. Authority cookies and privileged PAM execution
//! never enter this model; responses have a bounded, non-serializable owner.
use serde::Deserialize;
use zeroize::{Zeroize, Zeroizing};

/// Linux-PAM helper framing: at most 510 bytes plus newline and terminator.
pub const RESPONSE_MAX: usize = 510;
#[derive(Default, PartialEq, Eq)]
pub struct Response(Zeroizing<String>);
impl Response {
    pub fn new(mut text: String) -> Option<Self> {
        let valid = text.len() <= RESPONSE_MAX && !text.chars().any(char::is_control);
        let result = valid.then(|| Self(Zeroizing::new(text.clone())));
        text.zeroize();
        result
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn append(&mut self, text: &Self) -> bool {
        if self.0.len() + text.0.len() > RESPONSE_MAX {
            return false;
        }
        self.0.push_str(text.expose());
        true
    }
    pub fn backspace(&mut self) {
        self.0.pop();
    }
}
impl Clone for Response {
    fn clone(&self) -> Self {
        Self(Zeroizing::new(self.0.to_string()))
    }
}
impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Response([redacted])")
    }
}
impl<'de> Deserialize<'de> for Response {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?)
            .ok_or_else(|| serde::de::Error::custom("Invalid authentication response"))
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub uid: u32,
    pub name: String,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Challenge {
    pub id: u64,
    pub prompt: String,
    pub echo: bool,
}
impl std::fmt::Debug for Challenge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Challenge")
            .field("id", &self.id)
            .field("echo", &self.echo)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    Choose,
    Starting,
    Question(Challenge),
    Checking,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Request {
    pub id: u64,
    pub action: String,
    pub message: String,
    pub identities: Vec<Identity>,
    pub selected: Option<u32>,
    pub phase: Phase,
    pub info: Option<String>,
}
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request")
            .field("id", &self.id)
            .field("selected", &self.selected)
            .field("phase", &self.phase)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub enabled: bool,
    pub available: bool,
    pub epoch: u64,
    pub generation: u64,
    pub request: Option<Request>,
    pub error: Option<String>,
}
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum UiAction {
    Select {
        request: u64,
        uid: u32,
    },
    Respond {
        request: u64,
        turn: u64,
        response: Response,
    },
    Cancel {
        request: u64,
    },
}
#[derive(Debug)]
pub enum Action {
    Service(UiAction),
    Text {
        request: u64,
        turn: u64,
        text: Response,
    },
    Backspace {
        request: u64,
        turn: u64,
    },
    Submit {
        request: u64,
        turn: u64,
    },
    Keyboard(bool),
}
#[derive(Debug, Default)]
pub struct State {
    pub snapshot: Snapshot,
    pub input: Response,
    pub keyboard: bool,
    pub input_error: bool,
    retired: u64,
}
impl State {
    pub fn question(&self) -> Option<(u64, &Challenge)> {
        let request = self.snapshot.request.as_ref()?;
        match &request.phase {
            Phase::Question(q) => Some((request.id, q)),
            _ => None,
        }
    }
    fn matching(&self, request: u64, turn: u64) -> bool {
        self.question()
            .is_some_and(|(r, q)| r == request && q.id == turn)
    }
    fn clear_input(&mut self) {
        self.input = Response::default();
        self.keyboard = false;
        self.input_error = false;
    }
    pub fn cancel(&mut self) -> Option<UiAction> {
        self.clear_input();
        self.snapshot.request.take().map(|r| {
            self.retired = self.retired.max(r.id);
            UiAction::Cancel { request: r.id }
        })
    }
    /// `may_present` is supplied by compositor protection policy. New requests
    /// may open the trusted modal; old canceled requests cannot reopen it.
    pub fn sync(&mut self, mut snapshot: Snapshot, may_present: bool) -> Vec<UiAction> {
        let mut actions = Vec::new();
        if let Some(r) = &snapshot.request
            && (!may_present || r.id <= self.retired)
        {
            self.retired = self.retired.max(r.id);
            actions.push(UiAction::Cancel { request: r.id });
            snapshot.request = None;
        }
        let old = self.snapshot.request.as_ref();
        let new = snapshot.request.as_ref();
        if old.map(|r| r.id) != new.map(|r| r.id)
            && let Some(action) = self.cancel()
            && !actions.contains(&action)
        {
            actions.push(action);
        }
        let old_key = self
            .snapshot
            .request
            .as_ref()
            .map(|r| (r.id, r.selected, &r.phase));
        let new_key = snapshot
            .request
            .as_ref()
            .map(|r| (r.id, r.selected, &r.phase));
        if old_key != new_key {
            self.clear_input();
            self.keyboard = new.is_some_and(|r| matches!(r.phase, Phase::Question(_)));
        }
        self.snapshot = snapshot;
        actions
    }
    pub fn apply(&mut self, action: Action) -> Vec<UiAction> {
        match action {
            Action::Service(UiAction::Cancel { request })
                if self
                    .snapshot
                    .request
                    .as_ref()
                    .is_some_and(|r| r.id == request) =>
            {
                return self.cancel().into_iter().collect();
            }
            Action::Service(UiAction::Select { request, uid }) => {
                if let Some(r) = &mut self.snapshot.request
                    && r.id == request
                    && r.phase == Phase::Choose
                    && r.identities.iter().any(|i| i.uid == uid)
                {
                    r.selected = Some(uid);
                    r.phase = Phase::Starting;
                    self.clear_input();
                    return vec![UiAction::Select { request, uid }];
                }
            }
            Action::Service(UiAction::Respond {
                request,
                turn,
                response,
            }) if self.matching(request, turn) => {
                self.clear_input();
                self.snapshot.request.as_mut().unwrap().phase = Phase::Checking;
                return vec![UiAction::Respond {
                    request,
                    turn,
                    response,
                }];
            }
            Action::Text {
                request,
                turn,
                text,
            } if self.matching(request, turn) => {
                self.input_error = !self.input.append(&text);
            }
            Action::Backspace { request, turn } if self.matching(request, turn) => {
                self.input.backspace();
                self.input_error = false;
            }
            Action::Submit { request, turn } if self.matching(request, turn) => {
                let response = std::mem::take(&mut self.input);
                return self.apply(Action::Service(UiAction::Respond {
                    request,
                    turn,
                    response,
                }));
            }
            Action::Keyboard(visible) => self.keyboard = visible && self.question().is_some(),
            _ => {}
        }
        Vec::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(id: u64, turn: u64) -> Snapshot {
        Snapshot {
            enabled: true,
            available: true,
            epoch: 1,
            request: Some(Request {
                id,
                action: "org.tarot.Test".into(),
                message: "Test authorization".into(),
                identities: vec![Identity {
                    uid: 10000,
                    name: "Test user".into(),
                }],
                selected: Some(10000),
                phase: Phase::Question(Challenge {
                    id: turn,
                    prompt: "Password:".into(),
                    echo: false,
                }),
                info: None,
            }),
            ..Default::default()
        }
    }
    #[test]
    fn changed_question_retires_input_and_stale_answers_cannot_reach_a_later_turn() {
        let mut state = State::default();
        state.sync(snapshot(1, 1), true);
        state.apply(Action::Text {
            request: 1,
            turn: 1,
            text: Response::new("synthetic-secret".into()).unwrap(),
        });
        state.sync(snapshot(1, 2), true);
        assert!(state.input.expose().is_empty());
        assert!(
            state
                .apply(Action::Submit {
                    request: 1,
                    turn: 1
                })
                .is_empty()
        );
        state.apply(Action::Text {
            request: 1,
            turn: 2,
            text: Response::new("002345".into()).unwrap(),
        });
        let actions = state.apply(Action::Submit {
            request: 1,
            turn: 2,
        });
        assert!(
            matches!(&actions[..],[UiAction::Respond{request:1,turn:2,response}] if response.expose()=="002345")
        );
        assert!(state.input.expose().is_empty() && !state.keyboard);
        assert!(
            state
                .apply(Action::Submit {
                    request: 1,
                    turn: 2
                })
                .is_empty()
        );
    }
    #[test]
    fn protection_or_dismissal_cancels_and_late_snapshots_cannot_reopen_the_request() {
        let mut state = State::default();
        state.sync(snapshot(1, 1), true);
        assert_eq!(state.cancel(), Some(UiAction::Cancel { request: 1 }));
        assert_eq!(
            state.sync(snapshot(1, 1), true),
            vec![UiAction::Cancel { request: 1 }]
        );
        assert!(state.snapshot.request.is_none());
        state.sync(snapshot(2, 1), true);
        assert!(state.keyboard);
        assert_eq!(
            state.sync(snapshot(2, 1), false),
            vec![UiAction::Cancel { request: 2 }]
        );
        assert!(state.snapshot.request.is_none() && !state.keyboard);
        state.sync(snapshot(3, 1), true);
        assert_eq!(state.snapshot.request.unwrap().id, 3);
    }
    #[test]
    fn identities_are_chosen_only_from_the_authoritys_current_offer() {
        let mut state = State::default();
        let mut s = snapshot(1, 1);
        s.request.as_mut().unwrap().phase = Phase::Choose;
        state.sync(s, true);
        assert!(
            state
                .apply(Action::Service(UiAction::Select { request: 1, uid: 0 }))
                .is_empty()
        );
        assert_eq!(
            state.apply(Action::Service(UiAction::Select {
                request: 1,
                uid: 10000
            })),
            vec![UiAction::Select {
                request: 1,
                uid: 10000
            }]
        );
        assert!(
            state
                .apply(Action::Service(UiAction::Select {
                    request: 1,
                    uid: 10000
                }))
                .is_empty()
        );
    }
}
