//! Bluetooth presentation uses opaque, non-reused service tokens. Pairing codes
//! stay in redacted owned buffers and are never part of serialized diagnostics.
use crate::{Effect, ShellModel, SystemPage, network::Secret};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adapter {
    pub id: u64,
    pub label: String,
    pub powered: bool,
    pub discovering: bool,
    pub discoverable: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    pub id: u64,
    pub label: String,
    pub paired: bool,
    pub trusted: bool,
    pub connected: bool,
    pub blocked: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum PromptKind {
    Pin,
    Passkey,
    Confirm,
    DisplayPin,
    DisplayPasskey,
    AuthorizePairing,
    AuthorizeService,
}
impl PromptKind {
    pub fn needs_code(self) -> bool {
        matches!(self, Self::Pin | Self::Passkey)
    }
    pub fn display_only(self) -> bool {
        matches!(self, Self::DisplayPin | Self::DisplayPasskey)
    }
    pub fn accepts(self, code: &Secret) -> bool {
        match self {
            Self::Pin => {
                (1..=16).contains(&code.expose().len())
                    && code
                        .expose()
                        .bytes()
                        .all(|b| b.is_ascii_graphic() || b == b' ')
            }
            Self::Passkey => {
                !code.expose().is_empty()
                    && code.expose().len() <= 6
                    && code.expose().bytes().all(|b| b.is_ascii_digit())
            }
            _ => false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub id: u64,
    pub device: u64,
    pub label: String,
    pub kind: PromptKind,
    pub code: Option<Secret>,
    pub entered: u16,
    pub service: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Pending {
    Radio,
    Scan,
    Pair(u64),
    Connect(u64),
    Disconnect(u64),
    Forget(u64),
    Trust(u64),
    Visibility,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub enabled: bool,
    pub available: bool,
    pub generation: u64,
    pub epoch: u64,
    pub adapters: Vec<Adapter>,
    pub selected_adapter: Option<u64>,
    pub devices: Vec<Device>,
    pub scanning: bool,
    pub visible: bool,
    pub agent_available: bool,
    pub default_agent: bool,
    pub pending: Option<Pending>,
    pub busy: bool,
    pub prompt: Option<Prompt>,
    pub error: Option<String>,
    pub action_error: Option<String>,
}
impl Snapshot {
    pub fn adapter(&self) -> Option<&Adapter> {
        self.adapters
            .iter()
            .find(|a| Some(a.id) == self.selected_adapter)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub enum Answer {
    Accept,
    Reject,
    Code(Secret),
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum UiAction {
    Adapter { id: u64 },
    Radio { adapter: u64, enabled: bool },
    Scan { adapter: u64, enabled: bool },
    Visible { adapter: u64, enabled: bool },
    Pair { device: u64 },
    Connect { device: u64 },
    Disconnect { device: u64 },
    Forget { device: u64 },
    Trust { device: u64, trusted: bool },
    CancelPairing,
    Reply { request: u64, answer: Answer },
}
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Service(UiAction),
    Select(u64),
    Back,
    AskForget(u64),
    ConfirmForget,
    Text { request: u64, value: Secret },
    Backspace { request: u64 },
    Submit { request: u64 },
    Keyboard(bool),
}
#[derive(Clone, Debug, Default)]
pub struct View {
    pub selected: Option<u64>,
    pub forget: Option<u64>,
    pub input: Secret,
    pub keyboard: bool,
    prompt_id: Option<u64>,
    retired_prompt: u64,
}
impl ShellModel {
    pub fn bluetooth_keyboard_visible(&self) -> bool {
        self.system_menu_open
            && self.system_page == SystemPage::Bluetooth
            && self.bluetooth_ui.keyboard
            && self
                .bluetooth
                .prompt
                .as_ref()
                .is_some_and(|p| p.kind.needs_code())
    }
    pub fn cancel_bluetooth_input(&mut self) -> Vec<Effect> {
        let effect = self.bluetooth.prompt.take().map(|p| {
            self.bluetooth_ui.retired_prompt = self.bluetooth_ui.retired_prompt.max(p.id);
            Effect::Bluetooth(UiAction::Reply {
                request: p.id,
                answer: Answer::Reject,
            })
        });
        self.bluetooth_ui.input = Secret::default();
        self.bluetooth_ui.prompt_id = None;
        self.bluetooth_ui.keyboard = false;
        self.bluetooth_ui.forget = None;
        effect.into_iter().collect()
    }
    pub fn sync_bluetooth(&mut self, mut snapshot: Snapshot) -> Vec<Effect> {
        let mut effects = Vec::new();
        if self.bluetooth.adapters != snapshot.adapters
            || self.bluetooth.devices != snapshot.devices
            || self.bluetooth.selected_adapter != snapshot.selected_adapter
            || self.bluetooth.pending != snapshot.pending
            || self.bluetooth.busy != snapshot.busy
        {
            self.menus.invalidate();
        }
        if let Some(p) = &snapshot.prompt
            && (!self.system_menu_open
                || self.system_page != SystemPage::Bluetooth
                || p.id <= self.bluetooth_ui.retired_prompt)
        {
            effects.push(Effect::Bluetooth(UiAction::Reply {
                request: p.id,
                answer: Answer::Reject,
            }));
            snapshot.prompt = None;
        }
        let id = snapshot.prompt.as_ref().map(|p| p.id);
        if self.bluetooth_ui.prompt_id != id {
            self.bluetooth_ui.input = Secret::default();
            self.bluetooth_ui.keyboard = snapshot
                .prompt
                .as_ref()
                .is_some_and(|p| p.kind.needs_code());
            self.bluetooth_ui.prompt_id = id;
            self.menus.invalidate();
        }
        if self
            .bluetooth_ui
            .selected
            .is_some_and(|id| !snapshot.devices.iter().any(|d| d.id == id))
        {
            self.bluetooth_ui.selected = None;
            self.bluetooth_ui.forget = None;
        }
        self.bluetooth = snapshot;
        effects
    }
    pub(crate) fn bluetooth_action(&mut self, action: Action) -> Vec<Effect> {
        if !self.system_menu_open || self.system_page != SystemPage::Bluetooth {
            return vec![];
        }
        let current = |id| self.bluetooth.devices.iter().any(|d| d.id == id);
        match action {
            Action::Select(id) if current(id) => {
                self.bluetooth_ui.selected = Some(id);
                self.bluetooth_ui.forget = None;
                self.menus.invalidate();
            }
            Action::AskForget(id) if current(id) => {
                self.bluetooth_ui.forget = Some(id);
                self.menus.invalidate();
            }
            Action::ConfirmForget => {
                if let Some(id) = self.bluetooth_ui.forget.take().filter(|id| current(*id)) {
                    return vec![Effect::Bluetooth(UiAction::Forget { device: id })];
                }
            }
            Action::Back => {
                if self.bluetooth_ui.forget.take().is_none() {
                    self.bluetooth_ui.selected = None;
                }
                self.menus.invalidate();
            }
            Action::Keyboard(visible) => self.bluetooth_ui.keyboard = visible,
            Action::Text { request, value }
                if self
                    .bluetooth
                    .prompt
                    .as_ref()
                    .is_some_and(|p| p.id == request && p.kind.needs_code()) =>
            {
                self.bluetooth_ui.input.append(&value)
            }
            Action::Backspace { request }
                if self
                    .bluetooth
                    .prompt
                    .as_ref()
                    .is_some_and(|p| p.id == request && p.kind.needs_code()) =>
            {
                self.bluetooth_ui.input.backspace()
            }
            Action::Submit { request } => {
                if self
                    .bluetooth
                    .prompt
                    .as_ref()
                    .is_some_and(|p| p.id == request && p.kind.accepts(&self.bluetooth_ui.input))
                {
                    let code = std::mem::take(&mut self.bluetooth_ui.input);
                    self.bluetooth_ui.keyboard = false;
                    return vec![Effect::Bluetooth(UiAction::Reply {
                        request,
                        answer: Answer::Code(code),
                    })];
                }
            }
            Action::Service(UiAction::Forget { .. }) => {} // Forget requires the explicit confirmation above.
            Action::Service(UiAction::Reply { request, answer }) => {
                if self.bluetooth.prompt.as_ref().is_some_and(|p| {
                    p.id == request
                        && match &answer {
                            Answer::Accept => !p.kind.needs_code() && !p.kind.display_only(),
                            Answer::Reject => true,
                            Answer::Code(code) => p.kind.accepts(code),
                        }
                }) {
                    self.bluetooth_ui.input = Secret::default();
                    self.bluetooth_ui.keyboard = false;
                    return vec![Effect::Bluetooth(UiAction::Reply { request, answer })];
                }
            }
            Action::Service(action) => return vec![Effect::Bluetooth(action)],
            _ => {}
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;
    fn model() -> ShellModel {
        let mut m = ShellModel {
            system_menu_open: true,
            system_page: SystemPage::Bluetooth,
            ..Default::default()
        };
        m.bluetooth.devices = vec![Device {
            id: 4,
            label: "Keyboard".into(),
            paired: true,
            trusted: false,
            connected: false,
            blocked: false,
        }];
        m
    }
    #[test]
    fn forgotten_device_confirmation_cannot_target_a_replacement() {
        let mut m = model();
        m.apply(Command::Bluetooth(Action::AskForget(4)));
        m.sync_bluetooth(Snapshot {
            devices: vec![Device {
                id: 5,
                ..m.bluetooth.devices[0].clone()
            }],
            ..Default::default()
        });
        assert!(
            m.apply(Command::Bluetooth(Action::ConfirmForget))
                .is_empty()
        );
        m.apply(Command::Bluetooth(Action::AskForget(5)));
        assert_eq!(
            m.apply(Command::Bluetooth(Action::ConfirmForget)),
            vec![Effect::Bluetooth(UiAction::Forget { device: 5 })]
        );
    }
    #[test]
    fn prompt_replacement_and_dismissal_retire_codes_and_late_replies() {
        let mut m = model();
        let prompt = Prompt {
            id: 10,
            device: 4,
            label: "Keyboard".into(),
            kind: PromptKind::Passkey,
            code: None,
            entered: 0,
            service: None,
        };
        m.sync_bluetooth(Snapshot {
            prompt: Some(prompt.clone()),
            ..m.bluetooth.clone()
        });
        m.apply(Command::Bluetooth(Action::Text {
            request: 10,
            value: Secret::new("001234".into()),
        }));
        assert!(m.bluetooth_keyboard_visible());
        m.sync_bluetooth(Snapshot {
            prompt: Some(Prompt { id: 11, ..prompt }),
            ..m.bluetooth.clone()
        });
        assert!(m.bluetooth_ui.input.expose().is_empty());
        assert!(
            m.apply(Command::Bluetooth(Action::Submit { request: 10 }))
                .is_empty()
        );
        m.cancel_bluetooth_input();
        assert!(
            m.apply(Command::Bluetooth(Action::Service(UiAction::Reply {
                request: 11,
                answer: Answer::Accept
            })))
            .is_empty()
        );
    }
}
