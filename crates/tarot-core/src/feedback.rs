//! Feedback policy contains only opaque notification identities, never content.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Profile {
    Full,
    Quiet,
    Silent,
}
impl Profile {
    pub const ALL: [Self; 3] = [Self::Full, Self::Quiet, Self::Silent];
    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Quiet => "quiet",
            Self::Silent => "silent",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == value)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => "Sound & vibration",
            Self::Quiet => "Vibration",
            Self::Silent => "Silent",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum UiAction {
    Profile { epoch: u64, profile: Profile },
    Test { epoch: u64 },
    StopTest,
    Notifications { enabled: bool },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub enabled: bool,
    pub available: bool,
    pub epoch: u64,
    pub generation: u64,
    pub profile: Option<Profile>,
    pub pending: Option<Profile>,
    pub test_active: bool,
    pub test_result: Option<TestResult>,
    pub active_notifications: usize,
    pub notifications_enabled: bool,
    pub settings_available: bool,
    pub settings_saved: bool,
    pub settings_error: Option<String>,
    pub error: Option<String>,
    pub action_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum TestResult {
    Ended,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub notifications: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            notifications: true,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Notification(u32),
    Test,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub key: Key,
    pub revision: u64,
    /// A cap on this event, never an override of the global daemon policy.
    pub profile: Profile,
}
#[derive(Clone, Copy, Debug)]
struct Entry {
    ticket: Ticket,
    queued: bool,
}

/// A bounded mailbox shared with the notification service. Revocation replaces
/// desired state synchronously, including while the backend awaits a D-Bus reply.
pub struct Mailbox {
    enabled: bool,
    dnd: bool,
    entries: BTreeMap<Key, Entry>,
    next_test: u64,
}
impl Mailbox {
    pub fn new(enabled: bool, dnd: bool) -> Self {
        Self {
            enabled,
            dnd,
            entries: BTreeMap::new(),
            next_test: 0,
        }
    }
    pub fn notifications_enabled(&self) -> bool {
        self.enabled
    }
    pub fn settings(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.clear_notifications();
        }
    }
    pub fn retain(&mut self, dnd: bool, live: &[(u32, u64)]) {
        self.dnd = dnd;
        self.entries.retain(|key, e| match key {
            Key::Test => true,
            Key::Notification(id) => {
                !dnd && self.enabled && live.contains(&(*id, e.ticket.revision))
            }
        });
    }
    pub fn notify(&mut self, id: u32, revision: u64, urgency: u8, profile: Option<Profile>) {
        let key = Key::Notification(id);
        self.entries.remove(&key);
        if !self.enabled || self.dnd || urgency == 0 {
            return;
        }
        let Some(profile) = profile else {
            return;
        };
        if self
            .entries
            .keys()
            .filter(|k| matches!(k, Key::Notification(_)))
            .count()
            >= crate::notifications::MAX_ENTRIES
        {
            return;
        }
        self.entries.insert(
            key,
            Entry {
                ticket: Ticket {
                    key,
                    revision,
                    profile,
                },
                queued: true,
            },
        );
    }
    pub fn test(&mut self) -> Option<Ticket> {
        if self.entries.contains_key(&Key::Test) {
            return None;
        }
        self.next_test = self.next_test.checked_add(1)?;
        let ticket = Ticket {
            key: Key::Test,
            revision: self.next_test,
            profile: Profile::Quiet,
        };
        self.entries.insert(
            Key::Test,
            Entry {
                ticket,
                queued: true,
            },
        );
        Some(ticket)
    }
    pub fn stop_test(&mut self) {
        self.entries.remove(&Key::Test);
    }
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    fn clear_notifications(&mut self) {
        self.entries.retain(|k, _| *k == Key::Test);
    }
    pub fn take(&mut self) -> Option<Ticket> {
        let entry = self.entries.values_mut().find(|e| e.queued)?;
        entry.queued = false;
        Some(entry.ticket)
    }
    pub fn has_pending(&self) -> bool {
        self.entries.values().any(|e| e.queued)
    }
    pub fn current(&self, ticket: Ticket) -> bool {
        self.entries
            .get(&ticket.key)
            .is_some_and(|e| e.ticket == ticket)
    }
    pub fn finish(&mut self, ticket: Ticket) {
        if self.current(ticket) {
            self.entries.remove(&ticket.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_and_late_completion_cannot_retire_a_new_notification() {
        let mut m = Mailbox::new(true, false);
        m.notify(1, 10, 1, Some(Profile::Full));
        let old = m.take().unwrap();
        assert!(m.take().is_none());
        m.notify(1, 11, 1, Some(Profile::Quiet));
        let new = m.take().unwrap();
        assert!(!m.current(old) && m.current(new));
        m.finish(old);
        assert!(m.current(new));
        m.finish(new);
        assert!(!m.current(new));
    }
    #[test]
    fn dnd_disable_close_and_service_loss_revoke_without_replaying() {
        let mut m = Mailbox::new(true, false);
        m.notify(1, 1, 2, Some(Profile::Full));
        let ticket = m.take().unwrap();
        m.retain(true, &[(1, 1)]);
        assert!(!m.current(ticket));
        m.retain(false, &[(1, 1)]);
        assert!(m.take().is_none());
        m.notify(1, 2, 1, Some(Profile::Full));
        let ticket = m.take().unwrap();
        m.settings(false);
        assert!(!m.current(ticket));
        m.settings(true);
        assert!(m.take().is_none());
        m.notify(1, 3, 1, Some(Profile::Full));
        m.retain(false, &[]);
        assert!(m.take().is_none());
        m.notify(1, 4, 1, Some(Profile::Full));
        m.clear();
        assert!(m.take().is_none());
    }
    #[test]
    fn notification_policy_does_not_interrupt_an_explicit_bounded_test() {
        let mut m = Mailbox::new(false, true);
        let test = m.test().unwrap();
        assert_eq!(test.profile, Profile::Quiet);
        assert!(m.test().is_none());
        m.retain(true, &[]);
        m.settings(false);
        assert!(m.current(test));
        m.stop_test();
        let next = m.test().unwrap();
        assert_ne!(test.revision, next.revision);
        m.finish(test);
        assert!(m.current(next));
    }
    #[test]
    fn passive_suppressed_and_flooded_notifications_stay_bounded() {
        let mut m = Mailbox::new(true, false);
        m.notify(1, 1, 0, Some(Profile::Full));
        m.notify(2, 2, 1, None);
        assert!(m.take().is_none());
        for id in 0..1000 {
            m.notify(id, id.into(), 1, Some(Profile::Silent));
        }
        assert_eq!(
            std::iter::from_fn(|| m.take()).count(),
            crate::notifications::MAX_ENTRIES
        );
        assert!(m.test().is_some());
    }
}
