//! Persistent user display policy, separate from observed output/idle state.
use crate::{idle, orientation::Orientation};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub rotation_locked: bool,
    pub locked_orientation: Orientation,
    pub idle: idle::Settings,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Snapshot {
    pub idle: idle::Settings,
    pub idle_configurable: bool,
    pub panel_power_available: bool,
    pub settings_available: bool,
    pub settings_saved: bool,
    pub settings_error: Option<String>,
}
