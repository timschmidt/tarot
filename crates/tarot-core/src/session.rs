//! Session protection is separate from ordinary shell navigation.
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum LockPhase {
    #[default]
    Unlocked,
    Awaiting,
    Preparing,
    Locked,
    Orphaned,
}

/// A single-output presentation barrier. A page flip queued before protection
/// began cannot acknowledge a later lock. A dead locker never opens the gate.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LockGate {
    pub phase: LockPhase,
    pub generation: u64,
}

impl LockGate {
    pub fn protected(&self) -> bool {
        self.phase != LockPhase::Unlocked
    }
    /// Reserve protection before starting an untrusted-to-be-responsive child.
    pub fn reserve(&mut self) -> Option<u64> {
        if !matches!(self.phase, LockPhase::Unlocked | LockPhase::Orphaned) {
            return None;
        }
        self.generation = self.generation.checked_add(1)?;
        self.phase = LockPhase::Awaiting;
        Some(self.generation)
    }
    pub fn begin(&mut self) -> Option<u64> {
        if !matches!(
            self.phase,
            LockPhase::Unlocked | LockPhase::Orphaned | LockPhase::Awaiting
        ) {
            return None;
        }
        self.generation = self.generation.checked_add(1)?;
        self.phase = LockPhase::Preparing;
        Some(self.generation)
    }
    pub fn presented(&mut self, generation: u64) -> bool {
        if self.phase != LockPhase::Preparing || self.generation != generation {
            return false;
        }
        self.phase = LockPhase::Locked;
        true
    }
    pub fn orphan(&mut self) {
        if self.protected() {
            self.phase = LockPhase::Orphaned;
        }
    }
    /// The caller must independently authenticate the current lock owner.
    pub fn release(&mut self, generation: u64) -> bool {
        if self.phase != LockPhase::Locked || self.generation != generation {
            return false;
        }
        self.phase = LockPhase::Unlocked;
        true
    }
}

/// A bounded recovery budget, separate from authentication retry policy.
#[derive(Default)]
pub struct RecoveryBudget {
    pub failures: u32,
}
impl RecoveryBudget {
    pub fn failed(&mut self, alive_for: std::time::Duration) -> Option<std::time::Duration> {
        if alive_for >= std::time::Duration::from_secs(30) {
            self.failures = 0;
        }
        self.failures = self.failures.saturating_add(1);
        (self.failures <= 5).then(|| std::time::Duration::from_millis(250 << (self.failures - 1)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_and_user_reservation_cannot_acknowledge_or_unlock_without_an_owner() {
        let mut gate = LockGate::default();
        let reserved = gate.reserve().unwrap();
        assert!(gate.protected());
        assert!(!gate.presented(reserved));
        assert!(!gate.release(reserved));
        assert!(gate.reserve().is_none());
        let owner = gate.begin().unwrap();
        assert!(!gate.presented(reserved));
        assert!(gate.presented(owner));
        assert!(gate.release(owner));
        assert!(gate.reserve().unwrap() > owner);
    }
    #[test]
    fn crash_recovery_is_bounded_and_a_healthy_interval_resets_it() {
        use std::time::Duration;
        let mut budget = RecoveryBudget::default();
        for ms in [250, 500, 1000, 2000, 4000] {
            assert_eq!(
                budget.failed(Duration::ZERO),
                Some(Duration::from_millis(ms))
            );
        }
        assert_eq!(budget.failed(Duration::ZERO), None);
        assert_eq!(
            budget.failed(Duration::from_secs(30)),
            Some(Duration::from_millis(250))
        );
    }
    #[test]
    fn acknowledgement_requires_this_lock_frame_and_a_live_owner() {
        let mut gate = LockGate::default();
        let first = gate.begin().unwrap();
        assert!(gate.protected());
        assert!(!gate.release(first));
        assert!(!gate.presented(first - 1));
        assert!(gate.begin().is_none());
        gate.orphan();
        assert!(gate.protected());
        assert!(!gate.presented(first));
        let recovered = gate.begin().unwrap();
        assert!(!gate.presented(first));
        assert!(gate.presented(recovered));
        assert!(!gate.presented(recovered));
        assert!(!gate.release(first));
        assert!(gate.release(recovered));
        assert!(!gate.protected());
    }
}
