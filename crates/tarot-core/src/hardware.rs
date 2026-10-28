//! Physical button sequences, independent of a renderer or system service.
use std::{collections::BTreeMap, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Power,
    VolumeUp,
    VolumeDown,
    Mute,
}
impl Key {
    /// Both pinned Smithay backends add XKB's eight-code offset to evdev codes.
    pub fn from_xkb(code: u32) -> Option<Self> {
        match code {
            124 => Some(Self::Power),
            123 => Some(Self::VolumeUp),
            122 => Some(Self::VolumeDown),
            121 => Some(Self::Mute),
            _ => None,
        }
    }
    pub fn action(self) -> Action {
        match self {
            Self::Power => Action::TogglePanel,
            Self::VolumeUp => Action::Volume(5),
            Self::VolumeDown => Action::Volume(-5),
            Self::Mute => Action::ToggleMute,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    TogglePanel,
    Volume(i8),
    ToggleMute,
}

#[derive(Default)]
pub struct Buttons {
    // False means canceled until release, rather than a fresh press on resume.
    held: BTreeMap<(String, Key), bool>,
    repeating: Option<Key>,
    repeat_at: Option<Duration>,
}
impl Buttons {
    const HOLD: Duration = Duration::from_millis(400);
    const REPEAT: Duration = Duration::from_millis(150);

    pub fn event(
        &mut self,
        device: &str,
        key: Key,
        pressed: bool,
        now: Duration,
    ) -> Option<Action> {
        let id = (device.to_owned(), key);
        if !pressed {
            self.held.remove(&id);
            self.refresh_repeat(now);
            return None;
        }
        if self.held.contains_key(&id) || self.held.len() >= 64 || device.len() > 256 {
            return None;
        }
        self.held.insert(id, true);
        self.refresh_repeat(now);
        Some(key.action())
    }
    fn refresh_repeat(&mut self, now: Duration) {
        let up = self
            .held
            .iter()
            .any(|((_, k), active)| *active && *k == Key::VolumeUp);
        let down = self
            .held
            .iter()
            .any(|((_, k), active)| *active && *k == Key::VolumeDown);
        let next = match (up, down) {
            (true, false) => Some(Key::VolumeUp),
            (false, true) => Some(Key::VolumeDown),
            _ => None,
        };
        if next != self.repeating {
            self.repeating = next;
            self.repeat_at = next.map(|_| now + Self::HOLD);
        }
    }
    pub fn repeat(&mut self, now: Duration) -> Option<Action> {
        if !self.repeat_at.is_some_and(|d| now >= d) {
            return None;
        }
        // Never catch up with a burst after a stalled event loop or suspend.
        self.repeat_at = Some(now + Self::REPEAT);
        self.repeating.map(Key::action)
    }
    pub fn deadline(&self) -> Option<Duration> {
        self.repeat_at
    }
    pub fn remove(&mut self, device: &str, now: Duration) {
        self.held.retain(|(owner, _), _| owner != device);
        self.refresh_repeat(now);
    }
    pub fn cancel(&mut self) {
        self.held.values_mut().for_each(|active| *active = false);
        self.repeating = None;
        self.repeat_at = None;
    }
    pub fn cancel_volume(&mut self) {
        for ((_, key), active) in &mut self.held {
            if *key != Key::Power {
                *active = false;
            }
        }
        self.repeating = None;
        self.repeat_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }
    #[test]
    fn power_is_one_action_per_physical_sequence_even_across_cancel() {
        let mut b = Buttons::default();
        assert_eq!(Key::from_xkb(124), Some(Key::Power));
        assert_eq!(Key::from_xkb(38), None);
        assert_eq!(
            b.event("power", Key::Power, true, ms(0)),
            Some(Action::TogglePanel)
        );
        assert_eq!(b.event("power", Key::Power, true, ms(10)), None);
        assert_eq!(b.repeat(ms(5000)), None);
        b.cancel();
        assert_eq!(b.event("power", Key::Power, true, ms(6000)), None);
        assert_eq!(b.event("power", Key::Power, false, ms(6010)), None);
        assert_eq!(
            b.event("power", Key::Power, true, ms(6020)),
            Some(Action::TogglePanel)
        );
        b.remove("power", ms(6030));
        assert_eq!(b.event("power", Key::Power, false, ms(6040)), None);
    }
    #[test]
    fn volume_repeat_stops_on_opposition_removal_and_canceled_holds() {
        let mut b = Buttons::default();
        assert_eq!(
            b.event("volume", Key::VolumeUp, true, ms(0)),
            Some(Action::Volume(5))
        );
        assert_eq!(b.repeat(ms(399)), None);
        assert_eq!(b.repeat(ms(400)), Some(Action::Volume(5)));
        assert_eq!(b.repeat(ms(5000)), Some(Action::Volume(5)));
        assert_eq!(b.repeat(ms(5000)), None);
        b.event("volume", Key::VolumeDown, true, ms(5010));
        assert_eq!(b.repeat(ms(9000)), None);
        b.event("volume", Key::VolumeDown, false, ms(9010));
        assert_eq!(b.repeat(ms(9410)), Some(Action::Volume(5)));
        b.cancel_volume();
        assert_eq!(b.event("volume", Key::VolumeUp, true, ms(9500)), None);
        b.remove("volume", ms(9600));
        assert_eq!(b.repeat(ms(20000)), None);
    }
}
