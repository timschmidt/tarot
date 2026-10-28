//! Observed playback outputs and explicit controls; the platform owns routing.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Output {
    pub id: u64,
    pub label: String,
    pub percent: u32,
    pub muted: bool,
    pub default: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub enabled: bool,
    pub available: bool,
    pub generation: u64,
    pub epoch: u64,
    pub outputs: Vec<Output>,
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
}

impl UiAction {
    pub fn output(&self) -> u64 {
        match *self {
            Self::Volume { output, .. } | Self::Mute { output, .. } | Self::Default { output } => {
                output
            }
        }
    }
    pub fn valid(&self) -> bool {
        self.output() != 0 && !matches!(self, Self::Volume { percent: 101.., .. })
    }
    pub fn confirmed(&self, snapshot: &Snapshot) -> bool {
        snapshot.available
            && snapshot.outputs.iter().any(|o| {
                o.id == self.output()
                    && match *self {
                        Self::Volume { percent, .. } => {
                            if percent == 0 {
                                o.percent == 0
                            } else {
                                o.percent.abs_diff(u32::from(percent)) <= 1
                            }
                        }
                        Self::Mute { muted, .. } => o.muted == muted,
                        Self::Default { .. } => o.default,
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
