//! Readiness for the session manager's automatic idle action, not a sleep call.
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default)]
pub struct Conditions {
    pub enabled: bool,
    pub locked: bool,
    pub output_off: bool,
    pub protected_off_proof: bool,
    pub seat_active: bool,
    pub sleep_armed: bool,
    pub audio_observed: bool,
    pub audio_block_ready: bool,
    /// None means that external-power observation is unavailable.
    pub external_power: Option<bool>,
    pub allow_external_power: bool,
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
    Ready,
}

impl Conditions {
    pub fn readiness(self) -> Readiness {
        if !self.enabled {
            Readiness::Disabled
        } else if !self.locked || !self.output_off {
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
            locked: true,
            output_off: true,
            protected_off_proof: true,
            seat_active: true,
            sleep_armed: true,
            audio_observed: true,
            audio_block_ready: true,
            external_power: Some(false),
            allow_external_power: false,
        }
    }

    #[test]
    fn display_off_alone_never_declares_session_idle() {
        assert_eq!(ready().readiness(), Readiness::Ready);
        for c in [
            Conditions {
                locked: false,
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
}
