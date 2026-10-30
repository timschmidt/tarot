//! Observed audio devices and routes; the platform owns recording and playback.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Availability {
    Unknown,
    No,
    Yes,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Port {
    pub id: u64,
    pub label: String,
    pub available: Availability,
    pub active: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Output {
    pub id: u64,
    pub label: String,
    pub percent: u32,
    pub muted: bool,
    pub default: bool,
    pub ports: Vec<Port>,
}
/// An input has the same observed controls; monitor sources are excluded.
pub type Input = Output;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub enabled: bool,
    pub available: bool,
    pub generation: u64,
    pub epoch: u64,
    pub outputs: Vec<Output>,
    pub inputs: Vec<Input>,
    pub pending: Option<UiAction>,
    pub error: Option<String>,
    pub action_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub enum UiAction {
    Volume { output: u64, percent: u8 },
    Mute { output: u64, muted: bool },
    Default { output: u64 },
    OutputPort { output: u64, port: u64 },
    InputVolume { input: u64, percent: u8 },
    InputMute { input: u64, muted: bool },
    DefaultInput { input: u64 },
    InputPort { input: u64, port: u64 },
}

impl UiAction {
    pub fn device(&self) -> u64 {
        match *self {
            Self::Volume { output, .. }
            | Self::Mute { output, .. }
            | Self::Default { output }
            | Self::OutputPort { output, .. } => output,
            Self::InputVolume { input, .. }
            | Self::InputMute { input, .. }
            | Self::DefaultInput { input }
            | Self::InputPort { input, .. } => input,
        }
    }
    pub fn is_input(&self) -> bool {
        matches!(
            self,
            Self::InputVolume { .. }
                | Self::InputMute { .. }
                | Self::DefaultInput { .. }
                | Self::InputPort { .. }
        )
    }
    pub fn port(&self) -> Option<u64> {
        match *self {
            Self::InputPort { port, .. } | Self::OutputPort { port, .. } => Some(port),
            _ => None,
        }
    }
    pub fn valid(&self) -> bool {
        self.device() != 0
            && self.port() != Some(0)
            && !matches!(
                self,
                Self::Volume { percent: 101.., .. } | Self::InputVolume { percent: 101.., .. }
            )
    }
    pub fn observed<'a>(&self, snapshot: &'a Snapshot) -> Option<&'a Output> {
        if !snapshot.available {
            return None;
        }
        let devices = if self.is_input() {
            &snapshot.inputs
        } else {
            &snapshot.outputs
        };
        devices.iter().find(|device| device.id == self.device())
    }
    pub fn selectable(&self, snapshot: &Snapshot) -> bool {
        self.valid()
            && self.observed(snapshot).is_some_and(|device| {
                self.port().is_none_or(|id| {
                    device
                        .ports
                        .iter()
                        .any(|port| port.id == id && port.available != Availability::No)
                })
            })
    }
    pub fn confirmed(&self, snapshot: &Snapshot) -> bool {
        self.selectable(snapshot)
            && self.observed(snapshot).is_some_and(|device| match *self {
                Self::Volume { percent, .. } | Self::InputVolume { percent, .. } => {
                    if percent == 0 {
                        device.percent == 0
                    } else {
                        device.percent.abs_diff(u32::from(percent)) <= 1
                    }
                }
                Self::Mute { muted, .. } | Self::InputMute { muted, .. } => device.muted == muted,
                Self::Default { .. } | Self::DefaultInput { .. } => device.default,
                Self::OutputPort { port, .. } | Self::InputPort { port, .. } => {
                    device.ports.iter().any(|p| p.id == port && p.active)
                }
            })
    }
}

/// Contains only public volume state; device labels and app metadata stay out
/// of the transient hardware-key feedback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct KeyFeedback {
    pub percent: Option<u32>,
    pub muted: bool,
    pub pending: bool,
    pub failed: bool,
}

#[derive(Clone)]
struct KeyTarget {
    epoch: u64,
    output: u64,
    percent: u8,
    muted: bool,
}
/// Coalesces fast physical presses without inventing a confirmed volume or
/// carrying queued steps onto a newly selected/reconnected output.
#[derive(Default)]
pub struct KeyQueue {
    target: Option<KeyTarget>,
    sent: Option<UiAction>,
    pub failed: bool,
}
impl Snapshot {
    pub fn default_output(&self) -> Option<&Output> {
        if !self.available {
            return None;
        }
        let mut outputs = self.outputs.iter().filter(|o| o.default);
        let first = outputs.next()?;
        outputs.next().is_none().then_some(first)
    }
    pub fn default_identity(&self) -> Option<(u64, u64)> {
        self.default_output().map(|o| (self.epoch, o.id))
    }
}
impl KeyQueue {
    pub fn cancel(&mut self) {
        self.target = None;
        self.sent = None;
    }
    fn reconcile(&mut self, snapshot: &Snapshot) {
        if self
            .target
            .as_ref()
            .is_some_and(|t| snapshot.default_identity() != Some((t.epoch, t.output)))
        {
            self.cancel();
        }
    }
    pub fn input(&mut self, action: crate::hardware::Action, snapshot: &Snapshot) {
        self.reconcile(snapshot);
        self.failed = false;
        let Some(output) = snapshot.default_output() else {
            self.cancel();
            self.failed = true;
            return;
        };
        let target = self.target.get_or_insert(KeyTarget {
            epoch: snapshot.epoch,
            output: output.id,
            percent: output.percent.min(100) as u8,
            muted: output.muted,
        });
        match action {
            crate::hardware::Action::Volume(step) => {
                target.percent = (i16::from(target.percent) + i16::from(step)).clamp(0, 100) as u8;
            }
            crate::hardware::Action::ToggleMute => target.muted = !target.muted,
            crate::hardware::Action::TogglePanel => {}
        }
    }
    pub fn next(&mut self, snapshot: &Snapshot) -> Option<(u64, UiAction)> {
        self.reconcile(snapshot);
        if snapshot.pending.is_some() {
            return None;
        }
        if let Some(sent) = self.sent.take()
            && !sent.confirmed(snapshot)
        {
            self.reject();
            return None;
        }
        let target = self.target.as_ref()?;
        let output = snapshot.default_output()?;
        let action = if output.percent.abs_diff(u32::from(target.percent)) > 1
            || (target.percent == 0 && output.percent != 0)
        {
            UiAction::Volume {
                output: target.output,
                percent: target.percent,
            }
        } else if output.muted != target.muted {
            UiAction::Mute {
                output: target.output,
                muted: target.muted,
            }
        } else {
            self.target = None;
            return None;
        };
        Some((target.epoch, action))
    }
    pub fn sent(&mut self, action: UiAction) {
        self.sent = Some(action);
    }
    pub fn reject(&mut self) {
        self.cancel();
        self.failed = true;
    }
    pub fn feedback(&self, snapshot: &Snapshot) -> KeyFeedback {
        let output = snapshot.default_output();
        KeyFeedback {
            percent: output.map(|o| o.percent),
            muted: output.is_some_and(|o| o.muted),
            pending: self.target.is_some() || self.sent.is_some(),
            failed: self.failed || output.is_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn confirmation_requires_live_identity_and_observed_state() {
        let mut s = Snapshot {
            available: true,
            outputs: vec![Output {
                id: 7,
                label: "Speaker".into(),
                percent: 40,
                muted: false,
                default: true,
                ports: Vec::new(),
            }],
            ..Default::default()
        };
        assert!(
            UiAction::Volume {
                output: 7,
                percent: 40
            }
            .confirmed(&s)
        );
        assert!(
            !UiAction::Volume {
                output: 8,
                percent: 40
            }
            .confirmed(&s)
        );
        assert!(
            !UiAction::Mute {
                output: 7,
                muted: true
            }
            .confirmed(&s)
        );
        assert!(UiAction::Default { output: 7 }.confirmed(&s));
        s.available = false;
        assert!(!UiAction::Default { output: 7 }.confirmed(&s));
        assert!(
            !UiAction::Volume {
                output: 7,
                percent: 101
            }
            .valid()
        );
        assert!(!UiAction::Default { output: 0 }.valid());
    }
    #[test]
    fn input_and_output_routes_require_live_direction_and_available_port() {
        let mut state = snapshot();
        let mut input = state.outputs[0].clone();
        input.id = 8;
        input.ports = vec![Port {
            id: 12,
            label: "Microphone".into(),
            available: Availability::Unknown,
            active: true,
        }];
        state.inputs.push(input);
        let route = UiAction::InputPort { input: 8, port: 12 };
        assert!(route.confirmed(&state));
        assert!(
            !UiAction::OutputPort {
                output: 8,
                port: 12
            }
            .selectable(&state)
        );
        assert!(!UiAction::InputPort { input: 7, port: 12 }.selectable(&state));
        assert!(
            !UiAction::InputVolume {
                input: 8,
                percent: 101
            }
            .valid()
        );
        assert!(!UiAction::InputPort { input: 8, port: 0 }.valid());
        assert!(
            UiAction::InputMute {
                input: 8,
                muted: true
            }
            .confirmed(&state)
        );
        assert!(UiAction::DefaultInput { input: 8 }.confirmed(&state));
        state.inputs[0].ports[0].active = false;
        assert!(route.selectable(&state) && !route.confirmed(&state));
        state.inputs[0].ports[0].available = Availability::No;
        assert!(!route.selectable(&state));
        state.inputs[0].ports.clear();
        assert!(!route.selectable(&state));
        state.inputs.clear();
        assert!(
            !UiAction::InputMute {
                input: 8,
                muted: true
            }
            .confirmed(&state)
        );
    }
    fn snapshot() -> Snapshot {
        Snapshot {
            available: true,
            epoch: 3,
            outputs: vec![Output {
                id: 7,
                label: "Speaker".into(),
                percent: 40,
                muted: true,
                default: true,
                ports: Vec::new(),
            }],
            ..Default::default()
        }
    }
    #[test]
    fn hardware_steps_coalesce_while_pending_and_feedback_is_observed() {
        let mut s = snapshot();
        let mut q = KeyQueue::default();
        q.input(crate::hardware::Action::Volume(5), &s);
        let (epoch, action) = q.next(&s).unwrap();
        assert_eq!(epoch, 3);
        q.sent(action.clone());
        s.pending = Some(action);
        q.input(crate::hardware::Action::Volume(5), &s);
        q.input(crate::hardware::Action::Volume(5), &s);
        assert!(q.next(&s).is_none());
        assert_eq!(q.feedback(&s).percent, Some(40));
        s.pending = None;
        s.outputs[0].percent = 45;
        let (_, action) = q.next(&s).unwrap();
        assert_eq!(
            action,
            UiAction::Volume {
                output: 7,
                percent: 55
            }
        );
        q.sent(action);
        s.outputs[0].percent = 55;
        assert!(q.next(&s).is_none());
        assert!(!q.feedback(&s).pending);
        assert!(s.outputs[0].muted);
    }
    #[test]
    fn queued_hardware_steps_never_follow_an_output_or_epoch_change() {
        for epoch_change in [true, false] {
            let mut s = snapshot();
            let mut q = KeyQueue::default();
            q.input(crate::hardware::Action::Volume(5), &s);
            if epoch_change {
                s.epoch += 1;
            } else {
                s.outputs[0].id = 8;
            }
            assert!(q.next(&s).is_none());
        }
        let mut s = snapshot();
        let mut q = KeyQueue::default();
        q.input(crate::hardware::Action::Volume(100), &s);
        let (_, a) = q.next(&s).unwrap();
        assert_eq!(
            a,
            UiAction::Volume {
                output: 7,
                percent: 100
            }
        );
        q.sent(a);
        s.action_error = Some("denied".into());
        assert!(q.next(&s).is_none());
        assert!(q.feedback(&s).failed);
    }
}
