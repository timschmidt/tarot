//! Sleep preparation and resume barriers, independent of a bus or renderer.
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Purpose {
    Prepare,
    Resume,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Barrier {
    pub epoch: u64,
    pub ticket: u64,
    pub purpose: Purpose,
}

#[derive(Debug, Default, Serialize)]
pub struct Gate {
    pub epoch: u64,
    pub preparing: bool,
    pub barrier: Option<Barrier>,
    pub protected_preparations: u64,
    pub protected_resumes: u64,
    sequence: u64,
}

impl Gate {
    pub fn connect(&mut self, epoch: u64) -> bool {
        if epoch <= self.epoch {
            return false;
        }
        self.epoch = epoch;
        self.preparing = false;
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("sleep ticket exhausted");
        self.barrier = Some(Barrier {
            epoch,
            ticket: self.sequence,
            purpose: Purpose::Resume,
        });
        true
    }
    pub fn prepare(&mut self, epoch: u64, start: bool) -> bool {
        if epoch != self.epoch || self.preparing == start {
            return false;
        }
        self.preparing = start;
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("sleep ticket exhausted");
        self.barrier = Some(Barrier {
            epoch,
            ticket: self.sequence,
            purpose: if start {
                Purpose::Prepare
            } else {
                Purpose::Resume
            },
        });
        true
    }
    pub fn blocks_unlock(&self) -> bool {
        self.preparing || self.barrier.is_some()
    }
    /// Only call after actual completion of the tagged protected frame.
    pub fn presented(&mut self, barrier: Barrier) -> Option<Purpose> {
        if self.barrier != Some(barrier) {
            return None;
        }
        self.barrier = None;
        match barrier.purpose {
            Purpose::Prepare => self.protected_preparations += 1,
            Purpose::Resume => self.protected_resumes += 1,
        }
        Some(barrier.purpose)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connection_requires_a_fresh_protected_frame() {
        let mut gate = Gate::default();
        gate.connect(1);
        assert!(gate.blocks_unlock());
        assert_eq!(gate.presented(gate.barrier.unwrap()), Some(Purpose::Resume));
        assert!(!gate.blocks_unlock());
    }
    #[test]
    fn old_queued_frames_and_duplicate_signals_cannot_release_a_new_cycle() {
        let mut gate = Gate::default();
        assert!(gate.connect(1));
        assert!(gate.prepare(1, true));
        let first = gate.barrier.unwrap();
        assert!(!gate.prepare(1, true));
        assert_eq!(gate.presented(first), Some(Purpose::Prepare));
        assert!(gate.blocks_unlock());
        assert!(gate.prepare(1, false));
        let resume = gate.barrier.unwrap();
        assert!(gate.blocks_unlock());
        assert_eq!(gate.presented(first), None);
        assert_eq!(gate.presented(resume), Some(Purpose::Resume));
        assert!(!gate.blocks_unlock());
        assert!(gate.prepare(1, true));
        assert_eq!(gate.presented(first), None);
        assert_eq!(gate.protected_preparations, 1);
    }
    #[test]
    fn replacement_owner_retires_pending_tickets_and_old_signals() {
        let mut gate = Gate::default();
        gate.connect(1);
        gate.prepare(1, true);
        let old = gate.barrier.unwrap();
        assert!(gate.connect(2));
        assert!(!gate.connect(1));
        assert!(!gate.prepare(1, false));
        assert_eq!(gate.presented(old), None);
        assert!(gate.prepare(2, true));
        assert_ne!(gate.barrier, Some(old));
    }
}
