//! Live power state and display controls, independent of platform APIs.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Battery {
    pub available: bool,
    pub present: bool,
    pub percent: Option<u8>,
    pub coarse_level: Option<String>,
    pub state: ChargeState,
    pub external_power: Option<bool>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum ChargeState {
    #[default]
    Unknown,
    Charging,
    Discharging,
    Empty,
    Full,
    PendingCharge,
    PendingDischarge,
}

impl ChargeState {
    pub fn from_upower(value: u32) -> Self {
        match value {
            1 => Self::Charging,
            2 => Self::Discharging,
            3 => Self::Empty,
            4 => Self::Full,
            5 => Self::PendingCharge,
            6 => Self::PendingDischarge,
            _ => Self::Unknown,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Unknown => "Charge state unknown",
            Self::Charging => "Charging",
            Self::Discharging => "Discharging",
            Self::Empty => "Empty",
            Self::Full => "Fully charged",
            Self::PendingCharge => "Waiting to charge",
            Self::PendingDischarge => "Waiting to discharge",
        }
    }
}

impl Battery {
    pub fn level_label(&self) -> String {
        if !self.available {
            return "Battery unavailable".into();
        }
        if !self.present {
            return "No battery".into();
        }
        self.coarse_level
            .clone()
            .or_else(|| self.percent.map(|p| format!("{p}%")))
            .unwrap_or_else(|| "Battery level unknown".into())
    }
}

pub fn percentage(value: f64) -> Option<u8> {
    (value.is_finite() && (0.0..=100.0).contains(&value)).then(|| value.round() as u8)
}

pub fn coarse_level(value: u32) -> Option<String> {
    match value {
        3 => Some("Low"),
        4 => Some("Critical"),
        6 => Some("Normal"),
        7 => Some("High"),
        8 => Some("Full"),
        _ => None,
    }
    .map(str::to_owned)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Brightness {
    pub device: Option<String>,
    pub maximum: u32,
    pub actual: Option<u32>,
    pub requested: Option<u32>,
    pub can_set: bool,
    pub pending_percent: Option<u8>,
    pub error: Option<String>,
    pub action_error: Option<String>,
}

impl Brightness {
    pub fn percent(&self) -> Option<u8> {
        let actual = self.actual?;
        (self.maximum > 0 && actual <= self.maximum).then(|| {
            ((u64::from(actual) * 100 + u64::from(self.maximum) / 2) / u64::from(self.maximum))
                as u8
        })
    }
}

/// Reserve display-off for its own coordinated idle/lock policy.
pub fn brightness_value(percent: u8, maximum: u32) -> Option<u32> {
    if !(5..=100).contains(&percent) || maximum == 0 {
        return None;
    }
    Some(((u64::from(maximum) * u64::from(percent) + 50) / 100).max(1) as u32)
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub generation: u64,
    pub enabled: bool,
    pub battery: Battery,
    pub brightness: Brightness,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum UiAction {
    Brightness { percent: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_and_coarse_levels_do_not_invent_charge_state() {
        assert_eq!(percentage(f64::NAN), None);
        assert_eq!(percentage(-1.0), None);
        assert_eq!(percentage(101.0), None);
        assert_eq!(percentage(44.4), Some(44));
        let battery = Battery {
            available: true,
            present: true,
            percent: Some(44),
            coarse_level: coarse_level(3),
            state: ChargeState::Discharging,
            external_power: Some(true),
            error: None,
        };
        assert_eq!(battery.level_label(), "Low");
        assert_eq!(battery.state.label(), "Discharging");
        assert_eq!(ChargeState::from_upower(900), ChargeState::Unknown);
    }
    #[test]
    fn brightness_uses_driver_range_and_actual_readback() {
        assert_eq!(brightness_value(0, 51), None);
        assert_eq!(brightness_value(101, 51), None);
        assert_eq!(brightness_value(50, 0), None);
        assert_eq!(brightness_value(5, 1), Some(1));
        assert_eq!(brightness_value(50, 51), Some(26));
        assert_eq!(brightness_value(100, u32::MAX), Some(u32::MAX));
        let display = Brightness {
            maximum: 51,
            actual: Some(15),
            requested: Some(26),
            ..Default::default()
        };
        assert_eq!(display.percent(), Some(29));
    }
}
