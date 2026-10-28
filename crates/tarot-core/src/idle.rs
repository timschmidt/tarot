//! Idle locking and panel power barriers. Time is monotonic, supplied by host.
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// User timeouts in seconds; command-line millisecond overrides are separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub lock_after_seconds: u32,
    pub screen_off_enabled: bool,
    pub off_after_seconds: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            lock_after_seconds: 120,
            screen_off_enabled: true,
            off_after_seconds: 180,
        }
    }
}
impl Settings {
    pub fn valid(self) -> bool {
        (15..=86400).contains(&self.lock_after_seconds)
            && (15..=86400).contains(&self.off_after_seconds)
            && (!self.screen_off_enabled || self.off_after_seconds >= self.lock_after_seconds)
    }
    pub fn lock_after(self) -> Duration {
        Duration::from_secs(self.lock_after_seconds.into())
    }
    pub fn off_after(self) -> Option<Duration> {
        self.screen_off_enabled
            .then(|| Duration::from_secs(self.off_after_seconds.into()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Phase {
    Awake,
    WaitingForProtected(u64),
    Off,
    Waking(u64),
}
#[derive(Debug, Serialize)]
pub struct Policy {
    pub lock_after: Duration,
    pub off_after: Option<Duration>,
    pub phase: Phase,
    pub inhibited: bool,
    last_activity: Duration,
    sequence: u64,
    manual_off: bool,
}
impl Policy {
    /// Changing preferences must not discard an outstanding presentation ticket.
    pub fn set_timeouts(&mut self, settings: Settings, panel_power: bool) -> bool {
        if !settings.valid() {
            return false;
        }
        self.lock_after = settings.lock_after();
        self.off_after = panel_power.then(|| settings.off_after()).flatten();
        true
    }
    pub fn new(lock_after: Duration, off_after: Option<Duration>, now: Duration) -> Option<Self> {
        if lock_after.is_zero() || off_after.is_some_and(|off| off < lock_after) {
            return None;
        }
        Some(Self {
            lock_after,
            off_after,
            phase: Phase::Awake,
            inhibited: false,
            last_activity: now,
            sequence: 0,
            manual_off: false,
        })
    }
    fn ticket(&mut self) -> u64 {
        self.sequence = self.sequence.checked_add(1).expect("idle ticket exhausted");
        self.sequence
    }
    pub fn activity(&mut self, now: Duration) {
        if self.manual_off_waiting() {
            return;
        }
        self.manual_off = false;
        self.last_activity = now;
        match self.phase {
            Phase::Off => {
                let ticket = self.ticket();
                self.phase = Phase::Waking(ticket);
            }
            Phase::WaitingForProtected(_) => self.phase = Phase::Awake,
            _ => {}
        }
    }
    /// Explicit power intent overrides automatic-idle inhibition and timeouts.
    /// Ordinary input cannot cancel this ticket before its protected frame.
    pub fn request_off(&mut self, now: Duration) -> bool {
        if !matches!(self.phase, Phase::Awake | Phase::WaitingForProtected(_)) {
            return false;
        }
        self.manual_off = true;
        self.last_activity = now;
        let ticket = self.ticket();
        self.phase = Phase::WaitingForProtected(ticket);
        true
    }
    pub fn manual_off_waiting(&self) -> bool {
        self.manual_off && matches!(self.phase, Phase::WaitingForProtected(_))
    }
    /// Power toggling and authoritative sleep/resume may cancel a manual wait.
    pub fn system_activity(&mut self, now: Duration) {
        self.manual_off = false;
        self.activity(now);
    }
    pub fn inhibit(&mut self, active: bool, now: Duration) {
        if self.inhibited != active {
            self.inhibited = active;
            self.last_activity = now;
        }
    }
    pub fn deadline(&self, protected: bool) -> Option<Duration> {
        if self.phase != Phase::Awake || self.inhibited {
            return None;
        }
        let delay = if protected {
            self.off_after?
        } else {
            self.lock_after
        };
        self.last_activity.checked_add(delay)
    }
    pub fn lock_due(&self, now: Duration, protected: bool) -> bool {
        !protected
            && self.phase == Phase::Awake
            && !self.inhibited
            && now.saturating_sub(self.last_activity) >= self.lock_after
    }
    pub fn begin_off(&mut self, now: Duration) -> bool {
        if self.phase != Phase::Awake
            || self.inhibited
            || !self
                .off_after
                .is_some_and(|off| now.saturating_sub(self.last_activity) >= off)
        {
            return false;
        }
        let ticket = self.ticket();
        self.phase = Phase::WaitingForProtected(ticket);
        true
    }
    pub fn barrier(&self) -> Option<u64> {
        match self.phase {
            Phase::WaitingForProtected(ticket) | Phase::Waking(ticket) => Some(ticket),
            _ => None,
        }
    }
    /// The host must independently check the queued lock generation/completion.
    pub fn presented(&mut self, ticket: u64) -> bool {
        match self.phase {
            Phase::WaitingForProtected(current) if current == ticket => {
                self.phase = Phase::Off;
                true
            }
            Phase::Waking(current) if current == ticket => {
                self.phase = Phase::Awake;
                true
            }
            _ => false,
        }
    }
    pub fn blocks_unlock(&self) -> bool {
        self.phase != Phase::Awake
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }
    #[test]
    fn manual_power_ignores_auto_inhibition_and_requires_its_own_frame() {
        let mut p = Policy::new(ms(100), None, ms(0)).unwrap();
        p.inhibit(true, ms(0));
        assert!(p.request_off(ms(1)));
        let off = p.barrier().unwrap();
        p.activity(ms(2));
        assert_eq!(p.barrier(), Some(off));
        assert!(!p.presented(off + 1));
        assert!(p.presented(off));
        assert!(p.blocks_unlock());
        p.activity(ms(3));
        let wake = p.barrier().unwrap();
        assert_ne!(wake, off);
        assert!(!p.presented(off));
        assert!(p.presented(wake));
        assert!(!p.blocks_unlock());
    }
    #[test]
    fn system_wake_cancels_manual_wait_without_accepting_stale_completion() {
        let mut p = Policy::new(ms(100), Some(ms(200)), ms(0)).unwrap();
        p.begin_off(ms(200));
        let old = p.barrier().unwrap();
        p.request_off(ms(201));
        let manual = p.barrier().unwrap();
        assert_ne!(old, manual);
        assert!(!p.presented(old));
        p.system_activity(ms(202));
        assert_eq!(p.phase, Phase::Awake);
        assert!(!p.presented(manual));
    }
    #[test]
    fn preference_changes_preserve_off_wake_barriers_and_reject_invalid_timeouts() {
        let mut p = Policy::new(ms(100), Some(ms(200)), ms(0)).unwrap();
        p.begin_off(ms(200));
        let off = p.barrier().unwrap();
        assert!(p.set_timeouts(Settings::default(), true));
        assert_eq!(p.barrier(), Some(off));
        assert!(p.presented(off));
        assert!(p.set_timeouts(
            Settings {
                screen_off_enabled: false,
                ..Settings::default()
            },
            true
        ));
        assert!(p.blocks_unlock());
        p.activity(ms(220));
        let wake = p.barrier().unwrap();
        assert!(p.set_timeouts(Settings::default(), false));
        assert_eq!(p.barrier(), Some(wake));
        assert_eq!(p.off_after, None);
        let before = p.lock_after;
        assert!(!p.set_timeouts(
            Settings {
                lock_after_seconds: 0,
                ..Settings::default()
            },
            true
        ));
        assert!(!p.set_timeouts(
            Settings {
                off_after_seconds: 30,
                ..Settings::default()
            },
            true
        ));
        assert_eq!(p.lock_after, before);
        assert!(p.presented(wake));
    }
    #[test]
    fn lock_and_off_deadlines_share_real_activity_without_a_polling_cycle() {
        let mut p = Policy::new(ms(100), Some(ms(200)), ms(0)).unwrap();
        assert_eq!(p.deadline(false), Some(ms(100)));
        assert!(!p.lock_due(ms(99), false));
        assert!(p.lock_due(ms(100), false));
        assert_eq!(p.deadline(true), Some(ms(200)));
        assert!(!p.begin_off(ms(199)));
        assert!(p.begin_off(ms(200)));
        assert!(p.blocks_unlock());
        assert_eq!(p.deadline(true), None);
        let ticket = p.barrier().unwrap();
        assert!(!p.presented(ticket + 1));
        assert!(p.presented(ticket));
        assert_eq!(p.phase, Phase::Off);
        p.activity(ms(210));
        let wake = p.barrier().unwrap();
        assert!(p.blocks_unlock());
        assert!(!p.presented(ticket));
        assert!(p.presented(wake));
        assert!(!p.blocks_unlock());
        assert_eq!(p.deadline(true), Some(ms(410)));
    }
    #[test]
    fn activity_cancels_queued_off_and_stale_completion_cannot_blank_a_new_cycle() {
        let mut p = Policy::new(ms(100), Some(ms(200)), ms(0)).unwrap();
        p.begin_off(ms(200));
        let old = p.barrier().unwrap();
        p.activity(ms(201));
        assert!(!p.presented(old));
        assert_eq!(p.phase, Phase::Awake);
        p.begin_off(ms(401));
        assert!(!p.presented(old));
        assert!(p.presented(p.barrier().unwrap()));
    }
    #[test]
    fn inhibiting_and_releasing_grants_a_fresh_interval_without_waking_an_off_panel() {
        let mut p = Policy::new(ms(100), Some(ms(200)), ms(0)).unwrap();
        p.inhibit(true, ms(99));
        assert_eq!(p.deadline(false), None);
        assert!(!p.lock_due(ms(500), false));
        p.inhibit(false, ms(500));
        assert_eq!(p.deadline(false), Some(ms(600)));
        assert!(!p.lock_due(ms(599), false));
        assert!(p.begin_off(ms(700)));
        p.presented(p.barrier().unwrap());
        p.inhibit(true, ms(701));
        assert_eq!(p.phase, Phase::Off);
        assert!(Policy::new(ms(100), Some(ms(99)), ms(0)).is_none());
    }
}
