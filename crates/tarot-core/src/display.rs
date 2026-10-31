//! Persistent user display policy, separate from observed output/idle state.
use crate::{idle, orientation::Orientation, session_idle};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub rotation_locked: bool,
    pub locked_orientation: Orientation,
    pub idle: idle::Settings,
    pub sleep: session_idle::Settings,
}

/// What the phone shows at boot: the distribution greeter, or Tarot directly.
/// Direct boot still starts protected; actual PAM precedes any content.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum BootDestination {
    Greeter,
    Tarot,
}

/// Observed system configuration, never assumed from a request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct BootSnapshot {
    pub available: bool,
    pub current: Option<BootDestination>,
    pub pending: Option<BootDestination>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
    pub boot: BootSnapshot,
    pub idle: idle::Settings,
    pub sleep: session_idle::Settings,
    /// This session runs the idle publisher; otherwise sleep settings only persist.
    pub sleep_available: bool,
    pub sleep_readiness: session_idle::Readiness,
    /// The session manager's configured idle action, once observed.
    pub sleep_manager_action: Option<String>,
    pub sleep_error: bool,
    pub idle_configurable: bool,
    pub panel_power_available: bool,
    pub settings_available: bool,
    pub settings_saved: bool,
    pub settings_error: Option<String>,
}
