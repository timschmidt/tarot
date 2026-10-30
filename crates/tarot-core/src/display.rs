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

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
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
