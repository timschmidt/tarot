//! Readiness for the session manager's automatic idle action, not a sleep call.
use serde::{Deserialize, Serialize};

/// Persistent user sleep policy. Off by default; the session manager's own
/// idle action must still be configured before any sleep can follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub automatic: bool,
    /// Protected screen-off time before the session reports idle.
    pub after_off_seconds: u32,
    pub on_external_power: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            automatic: false,
            after_off_seconds: 60,
            on_external_power: false,
        }
    }
}
impl Settings {
    pub const DELAYS: [u32; 8] = [0, 30, 60, 120, 300, 600, 900, 1800];
    pub fn valid(self) -> bool {
        self.after_off_seconds <= 3600
    }
}

/// Evidence of a protected KMS completion for the current power-off transaction.
/// Locker generation/output geometry may change while that panel stays off;
/// the caller separately requires current protection and no pending wake.
pub fn protected_off_proof(
    phase: crate::session::LockPhase,
    proof_count: u64,
    current_count: u64,
) -> bool {
    phase != crate::session::LockPhase::Unlocked && proof_count != 0 && proof_count == current_count
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Conditions {
    pub enabled: bool,
    pub protected: bool,
    pub output_off: bool,
    pub protected_off_proof: bool,
    pub seat_active: bool,
    pub sleep_armed: bool,
    pub audio_observed: bool,
    pub audio_block_ready: bool,
    /// None means that external-power observation is unavailable.
    pub external_power: Option<bool>,
    pub allow_external_power: bool,
    /// The configured delay has elapsed since this protected power-off.
    pub delay_elapsed: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Readiness {
    #[default]
    Disabled,
    Active,
    ProtectionPending,
    SeatInactive,
    SleepUnavailable,
    AudioUnavailable,
    AudioBlockPending,
    PowerUnknown,
    ExternalPower,
    Delay,
    Ready,
}

impl Conditions {
    pub fn readiness(self) -> Readiness {
        if !self.enabled {
            Readiness::Disabled
        } else if !self.protected || !self.output_off {
            Readiness::Active
        } else if !self.protected_off_proof {
            Readiness::ProtectionPending
        } else if !self.seat_active {
            Readiness::SeatInactive
        } else if !self.sleep_armed {
            Readiness::SleepUnavailable
        } else if !self.audio_observed {
            Readiness::AudioUnavailable
        } else if !self.audio_block_ready {
            Readiness::AudioBlockPending
        } else {
            match self.external_power {
                None => Readiness::PowerUnknown,
                Some(true) if !self.allow_external_power => Readiness::ExternalPower,
                Some(_) if !self.delay_elapsed => Readiness::Delay,
                Some(_) => Readiness::Ready,
            }
        }
    }
}

/// A confirmed idle hint is an observation, not a connection-owned lease.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Publisher {
    pub available: bool,
    pub observed_idle: Option<bool>,
    pub manager_action: Option<String>,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Conditions {
        Conditions {
            enabled: true,
            protected: true,
            output_off: true,
            protected_off_proof: true,
            seat_active: true,
            sleep_armed: true,
            audio_observed: true,
            audio_block_ready: true,
            external_power: Some(false),
            allow_external_power: false,
            delay_elapsed: true,
        }
    }

    #[test]
    fn display_off_alone_never_declares_session_idle() {
        assert_eq!(ready().readiness(), Readiness::Ready);
        for c in [
            Conditions {
                protected: false,
                ..ready()
            },
            Conditions {
                output_off: false,
                ..ready()
            },
        ] {
            assert_eq!(c.readiness(), Readiness::Active);
        }
        assert_eq!(
            Conditions {
                protected_off_proof: false,
                ..ready()
            }
            .readiness(),
            Readiness::ProtectionPending
        );
    }

    #[test]
    fn session_and_services_must_be_ready() {
        for (c, expected) in [
            (
                Conditions {
                    enabled: false,
                    ..ready()
                },
                Readiness::Disabled,
            ),
            (
                Conditions {
                    seat_active: false,
                    ..ready()
                },
                Readiness::SeatInactive,
            ),
            (
                Conditions {
                    sleep_armed: false,
                    ..ready()
                },
                Readiness::SleepUnavailable,
            ),
            (
                Conditions {
                    audio_observed: false,
                    ..ready()
                },
                Readiness::AudioUnavailable,
            ),
            (
                Conditions {
                    audio_block_ready: false,
                    ..ready()
                },
                Readiness::AudioBlockPending,
            ),
        ] {
            assert_eq!(c.readiness(), expected);
        }
    }

    #[test]
    fn external_power_requires_an_explicit_policy_and_unknown_stays_unready() {
        assert_eq!(
            Conditions {
                external_power: Some(true),
                ..ready()
            }
            .readiness(),
            Readiness::ExternalPower
        );
        assert_eq!(
            Conditions {
                external_power: Some(true),
                allow_external_power: true,
                ..ready()
            }
            .readiness(),
            Readiness::Ready
        );
        assert_eq!(
            Conditions {
                external_power: None,
                allow_external_power: true,
                ..ready()
            }
            .readiness(),
            Readiness::PowerUnknown
        );
    }
    #[test]
    fn sleep_delay_is_checked_only_after_every_safety_and_power_condition() {
        let waiting = Conditions {
            delay_elapsed: false,
            ..ready()
        };
        assert_eq!(waiting.readiness(), Readiness::Delay);
        for (c, expected) in [
            (
                Conditions {
                    enabled: false,
                    ..waiting
                },
                Readiness::Disabled,
            ),
            (
                Conditions {
                    output_off: false,
                    ..waiting
                },
                Readiness::Active,
            ),
            (
                Conditions {
                    external_power: Some(true),
                    ..waiting
                },
                Readiness::ExternalPower,
            ),
            (
                Conditions {
                    external_power: None,
                    ..waiting
                },
                Readiness::PowerUnknown,
            ),
        ] {
            assert_eq!(c.readiness(), expected);
        }
    }
    #[test]
    fn settings_default_off_with_bounded_delays() {
        let settings = Settings::default();
        assert!(!settings.automatic && !settings.on_external_power && settings.valid());
        assert!(Settings::DELAYS.contains(&settings.after_off_seconds));
        assert!(Settings::DELAYS.iter().all(|&after_off_seconds| {
            Settings {
                after_off_seconds,
                ..settings
            }
            .valid()
        }));
        assert!(
            !Settings {
                after_off_seconds: 3601,
                ..settings
            }
            .valid()
        );
    }
    #[test]
    fn historical_protection_survives_owner_changes_but_not_a_different_power_transaction() {
        use crate::session::LockPhase;
        for phase in [
            LockPhase::Awaiting,
            LockPhase::Preparing,
            LockPhase::Locked,
            LockPhase::Orphaned,
        ] {
            assert!(protected_off_proof(phase, 7, 7));
            assert!(!protected_off_proof(phase, 6, 7));
            assert!(!protected_off_proof(phase, 0, 0));
        }
        assert!(!protected_off_proof(LockPhase::Unlocked, 7, 7));
    }
}
