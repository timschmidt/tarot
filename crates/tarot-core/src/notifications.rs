//! Notification lifetime and user actions, independent of D-Bus and rendering.
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub fn banner_rect(layout: crate::Layout) -> crate::Rect {
    let bottom = if layout.keyboard_height > 0.0 {
        layout.keyboard().y
    } else {
        layout.gesture().y
    };
    crate::Rect {
        x: 8.0,
        y: (bottom - 100.0).max(crate::Layout::STATUS),
        width: layout.width - 16.0,
        height: 92.0,
    }
}

pub fn overlay_contains(model: &crate::ShellModel, layout: crate::Layout, x: f32, y: f32) -> bool {
    model.notifications_open
        || model.system_menu_open
        || (model.notifications.banner.is_some() && banner_rect(layout).contains(x, y))
}

pub const MAX_ENTRIES: usize = 64;
const BANNER_TIME: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Action {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Notification {
    pub id: u32,
    pub revision: u64,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<Action>,
    pub urgent: bool,
    pub live: bool,
    pub orphaned: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub generation: u64,
    pub items: Vec<Notification>,
    pub banner: Option<(u32, u64)>,
    pub dnd: bool,
    pub hide_previews: bool,
    pub available: bool,
    pub error: Option<String>,
    pub settings_error: Option<String>,
    pub settings_saved: bool,
    pub settings_available: bool,
}

#[derive(Clone, Debug)]
pub struct Incoming {
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<Action>,
    pub urgency: u8,
    pub expire_ms: i32,
    pub resident: bool,
    pub transient: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum UiAction {
    Dismiss { id: u32, revision: u64 },
    Invoke { id: u32, revision: u64, key: String },
    HideBanner { id: u32, revision: u64 },
    Settings { dnd: bool, hide_previews: bool },
    Clear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    Closed { owner: String, id: u32, reason: u32 },
    Invoked { owner: String, id: u32, key: String },
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Invalid,
    Unknown,
    NotOwner,
    Full,
}

struct Entry {
    notification: Notification,
    owner: String,
    expires: Option<Duration>,
    banner_until: Option<Duration>,
    resident: bool,
    transient: bool,
}

#[derive(Default)]
pub struct Store {
    entries: Vec<Entry>,
    next_id: u32,
    revision: u64,
    dnd: bool,
    hide_previews: bool,
    signals: Vec<Signal>,
}

impl Store {
    pub fn notify(
        &mut self,
        owner: &str,
        replaces: u32,
        input: Incoming,
        now: Duration,
    ) -> Result<u32, Error> {
        if self.signals.len() > MAX_ENTRIES * 3 {
            return Err(Error::Full);
        }
        if input.app_name.len() > 256
            || input.summary.len() > 512
            || input.body.len() > 4096
            || input.actions.len() > 8
            || input
                .actions
                .iter()
                .any(|a| a.key.is_empty() || a.key.len() > 128 || a.label.len() > 256)
            || input.urgency > 2
            || input.expire_ms < -1
        {
            return Err(Error::Invalid);
        }
        if input
            .actions
            .iter()
            .enumerate()
            .any(|(i, a)| input.actions[..i].iter().any(|old| old.key == a.key))
        {
            return Err(Error::Invalid);
        }
        let existing = self
            .entries
            .iter()
            .position(|e| e.notification.id == replaces && e.notification.live);
        if existing.is_some_and(|i| self.entries[i].owner != owner) {
            return Err(Error::NotOwner);
        }
        let id = if let Some(i) = existing {
            self.entries[i].notification.id
        } else {
            if self.entries.len() >= MAX_ENTRIES {
                if let Some(i) = self.entries.iter().position(|e| !e.notification.live) {
                    self.entries.remove(i);
                } else {
                    return Err(Error::Full);
                }
            }
            self.next_id = self
                .next_id
                .checked_add(1)
                .filter(|id| *id != 0)
                .ok_or(Error::Full)?;
            self.next_id
        };
        self.revision += 1;
        let expires = if input.urgency == 2
            || input.expire_ms == 0
            || (input.expire_ms == -1 && !input.transient)
        {
            None
        } else {
            Some(
                now + Duration::from_millis(if input.expire_ms == -1 {
                    5_000
                } else {
                    input.expire_ms as u64
                }),
            )
        };
        let entry = Entry {
            notification: Notification {
                id,
                revision: self.revision,
                app_name: input.app_name,
                summary: input.summary,
                body: input.body,
                actions: input.actions,
                urgent: input.urgency == 2,
                live: true,
                orphaned: false,
            },
            owner: owner.into(),
            expires,
            banner_until: (!self.dnd).then_some(now + BANNER_TIME),
            resident: input.resident,
            transient: input.transient,
        };
        if let Some(i) = existing {
            self.entries[i] = entry;
        } else {
            self.entries.push(entry);
        }
        Ok(id)
    }

    fn close_at(&mut self, index: usize, reason: u32) {
        if self.signals.len() >= MAX_ENTRIES * 4 {
            return;
        }
        let entry = &mut self.entries[index];
        if !entry.notification.live {
            return;
        }
        self.revision += 1;
        entry.notification.revision = self.revision;
        entry.notification.live = false;
        entry.notification.actions.clear();
        entry.expires = None;
        entry.banner_until = None;
        self.signals.push(Signal::Closed {
            owner: entry.owner.clone(),
            id: entry.notification.id,
            reason,
        });
        if entry.transient {
            self.entries.remove(index);
        }
    }

    pub fn close(&mut self, owner: &str, id: u32) -> Result<(), Error> {
        if self.signals.len() >= MAX_ENTRIES * 4 {
            return Err(Error::Full);
        }
        let index = self
            .entries
            .iter()
            .position(|e| e.notification.id == id && e.notification.live)
            .ok_or(Error::Unknown)?;
        if self.entries[index].owner != owner {
            return Err(Error::NotOwner);
        }
        self.close_at(index, 3);
        Ok(())
    }

    pub fn owner_gone(&mut self, owner: &str) -> bool {
        let mut changed = false;
        for entry in &mut self.entries {
            if entry.owner == owner && !entry.notification.orphaned {
                self.revision += 1;
                entry.notification.revision = self.revision;
                entry.notification.orphaned = true;
                entry.notification.actions.clear();
                changed = true;
            }
        }
        changed
    }

    /// A new bus may reuse unique sender names. No live ownership or queued
    /// signal can cross that boundary; IDs remain monotonic in this process.
    pub fn disconnected(&mut self) {
        self.signals.clear();
        let previous_len = self.entries.len();
        self.entries.retain(|entry| !entry.transient);
        if previous_len != self.entries.len() {
            self.revision += 1;
        }
        for entry in &mut self.entries {
            if !entry.notification.live && entry.notification.orphaned {
                continue;
            }
            self.revision += 1;
            entry.notification.revision = self.revision;
            entry.notification.live = false;
            entry.notification.orphaned = true;
            entry.notification.actions.clear();
            entry.expires = None;
            entry.banner_until = None;
        }
    }

    pub fn ui(&mut self, action: UiAction) {
        match action {
            UiAction::Settings { dnd, hide_previews } => {
                self.revision += 1;
                self.dnd = dnd;
                self.hide_previews = hide_previews;
                if dnd {
                    for entry in &mut self.entries {
                        entry.banner_until = None;
                    }
                }
            }
            UiAction::Clear => {
                if self.signals.len() + self.entries.iter().filter(|e| e.notification.live).count()
                    > MAX_ENTRIES * 4
                {
                    return;
                }
                self.revision += 1;
                for i in (0..self.entries.len()).rev() {
                    self.close_at(i, 2);
                }
                self.entries.clear();
            }
            UiAction::Dismiss { id, revision } => {
                if let Some(i) = self
                    .entries
                    .iter()
                    .position(|e| e.notification.id == id && e.notification.revision == revision)
                {
                    if self.entries[i].notification.live && self.signals.len() >= MAX_ENTRIES * 4 {
                        return;
                    }
                    self.revision += 1;
                    self.close_at(i, 2);
                    // Explicit dismissal also removes retained history.
                    if let Some(i) = self.entries.iter().position(|e| e.notification.id == id) {
                        self.entries.remove(i);
                    }
                }
            }
            UiAction::HideBanner { id, revision } => {
                if let Some(e) = self
                    .entries
                    .iter_mut()
                    .find(|e| e.notification.id == id && e.notification.revision == revision)
                {
                    self.revision += 1;
                    e.banner_until = None;
                }
            }
            UiAction::Invoke { id, revision, key } => {
                if self.signals.len() + 2 > MAX_ENTRIES * 4 {
                    return;
                }
                if let Some(i) = self.entries.iter().position(|e| {
                    e.notification.id == id
                        && e.notification.revision == revision
                        && e.notification.live
                        && !e.notification.orphaned
                        && e.notification.actions.iter().any(|a| a.key == key)
                }) {
                    self.revision += 1;
                    self.signals.push(Signal::Invoked {
                        owner: self.entries[i].owner.clone(),
                        id,
                        key,
                    });
                    if !self.entries[i].resident {
                        self.close_at(i, 2);
                    }
                }
            }
        }
    }

    pub fn tick(&mut self, now: Duration) {
        for i in (0..self.entries.len()).rev() {
            if self.entries[i].expires.is_some_and(|t| t <= now) {
                self.close_at(i, 1);
            } else if self.entries[i].banner_until.is_some_and(|t| t <= now) {
                self.revision += 1;
                self.entries[i].banner_until = None;
            }
        }
    }

    pub fn deadline(&self) -> Option<Duration> {
        self.entries
            .iter()
            .flat_map(|e| [e.expires, e.banner_until])
            .flatten()
            .min()
    }

    pub fn signals(&mut self) -> Vec<Signal> {
        std::mem::take(&mut self.signals)
    }

    pub fn action_owner(&self, id: u32, revision: u64) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| {
                e.notification.id == id
                    && e.notification.revision == revision
                    && e.notification.live
                    && !e.notification.orphaned
            })
            .map(|e| e.owner.as_str())
    }

    pub fn snapshot(&self) -> Snapshot {
        let banner = self
            .entries
            .iter()
            .filter(|e| e.notification.live && e.banner_until.is_some())
            .max_by_key(|e| (e.notification.urgent, e.notification.revision))
            .map(|e| (e.notification.id, e.notification.revision));
        let mut items: Vec<_> = self
            .entries
            .iter()
            .map(|e| e.notification.clone())
            .collect();
        items.sort_by_key(|n| std::cmp::Reverse(n.revision));
        Snapshot {
            generation: self.revision,
            items,
            banner,
            dnd: self.dnd,
            hide_previews: self.hide_previews,
            available: true,
            error: None,
            settings_error: None,
            settings_saved: false,
            settings_available: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Incoming {
        Incoming {
            app_name: "Fixture".into(),
            summary: "Hello".into(),
            body: "Synthetic".into(),
            actions: vec![Action {
                key: "open".into(),
                label: "Open".into(),
            }],
            urgency: 1,
            expire_ms: -1,
            resident: false,
            transient: false,
        }
    }

    #[test]
    fn dismissal_preserves_live_entries_under_backpressure_and_versions_history_removal() {
        let mut store = Store::default();
        let mut resident = input();
        resident.resident = true;
        let first = store.notify(":1.1", 0, resident, Duration::ZERO).unwrap();
        let first_revision = store.snapshot().items[0].revision;
        let second = store.notify(":1.2", 0, input(), Duration::ZERO).unwrap();
        let second_revision = store.snapshot().items[0].revision;
        for _ in 0..MAX_ENTRIES * 4 - 1 {
            store.ui(UiAction::Invoke {
                id: first,
                revision: first_revision,
                key: "open".into(),
            });
        }
        store.close(":1.1", first).unwrap();
        store.ui(UiAction::Dismiss {
            id: second,
            revision: second_revision,
        });
        assert!(
            store
                .snapshot()
                .items
                .iter()
                .any(|n| n.id == second && n.live)
        );
        assert_eq!(store.signals().len(), MAX_ENTRIES * 4);
        store.ui(UiAction::Dismiss {
            id: second,
            revision: second_revision,
        });
        assert!(!store.snapshot().items.iter().any(|n| n.id == second));
        assert_eq!(
            store.signals(),
            vec![Signal::Closed {
                owner: ":1.2".into(),
                id: second,
                reason: 2
            }]
        );
        let snapshot = store.snapshot();
        store.ui(UiAction::Dismiss {
            id: first,
            revision: snapshot.items[0].revision,
        });
        assert!(store.snapshot().items.is_empty());
        assert!(store.snapshot().generation > snapshot.generation);
    }

    #[test]
    fn bus_replacement_cannot_reuse_ownership_actions_or_expiry() {
        let mut store = Store::default();
        let old_id = store.notify(":1.1", 0, input(), Duration::ZERO).unwrap();
        let old_revision = store.snapshot().items[0].revision;
        store.ui(UiAction::Invoke {
            id: old_id,
            revision: old_revision,
            key: "open".into(),
        });
        let mut transient = input();
        transient.transient = true;
        store.notify(":1.2", 0, transient, Duration::ZERO).unwrap();
        store.disconnected();
        assert_eq!(store.deadline(), None);
        assert!(store.signals().is_empty());
        let snapshot = store.snapshot();
        assert_eq!(snapshot.items.len(), 1);
        assert!(!snapshot.items[0].live && snapshot.items[0].orphaned);
        assert_eq!(store.close(":1.1", old_id), Err(Error::Unknown));
        store.ui(UiAction::Invoke {
            id: old_id,
            revision: old_revision,
            key: "open".into(),
        });
        assert!(store.signals().is_empty());
        let new_id = store
            .notify(":1.1", old_id, input(), Duration::from_secs(20))
            .unwrap();
        assert!(new_id > old_id);
        assert_ne!(store.snapshot().items[0].revision, old_revision);
    }

    #[test]
    fn replacement_preserves_id_but_stale_actions_and_deadlines_do_not_apply() {
        let mut store = Store::default();
        let mut first = input();
        first.expire_ms = 100;
        let id = store.notify(":1.1", 0, first, Duration::ZERO).unwrap();
        let revision = store.snapshot().items[0].revision;
        assert_eq!(
            store.notify(":1.1", id, input(), Duration::from_millis(50)),
            Ok(id)
        );
        store.tick(Duration::from_millis(150));
        store.ui(UiAction::Invoke {
            id,
            revision,
            key: "open".into(),
        });
        assert!(store.signals().is_empty());
        assert!(store.snapshot().items[0].live);
        assert_eq!(
            store.notify(":1.2", id, input(), Duration::ZERO),
            Err(Error::NotOwner)
        );
        assert_eq!(store.close(":1.2", id), Err(Error::NotOwner));
    }

    #[test]
    fn banner_timeout_keeps_dashboard_actions_and_resident_actions_do_not_close() {
        let mut store = Store::default();
        let mut n = input();
        n.resident = true;
        let id = store.notify(":1.1", 0, n, Duration::ZERO).unwrap();
        store.tick(BANNER_TIME);
        let snapshot = store.snapshot();
        assert_eq!(snapshot.banner, None);
        assert!(snapshot.items[0].live);
        store.ui(UiAction::Invoke {
            id,
            revision: snapshot.items[0].revision,
            key: "open".into(),
        });
        assert_eq!(
            store.signals(),
            vec![Signal::Invoked {
                owner: ":1.1".into(),
                id,
                key: "open".into()
            }]
        );
        assert!(store.snapshot().items[0].live);
    }

    #[test]
    fn close_reasons_expiry_transience_and_sender_loss_have_distinct_behavior() {
        let mut store = Store::default();
        let mut n = input();
        n.expire_ms = 100;
        n.transient = true;
        let id = store.notify(":1.1", 0, n, Duration::ZERO).unwrap();
        store.tick(Duration::from_millis(100));
        assert!(store.snapshot().items.is_empty());
        assert_eq!(
            store.signals(),
            vec![Signal::Closed {
                owner: ":1.1".into(),
                id,
                reason: 1
            }]
        );
        let id = store.notify(":1.1", 0, input(), Duration::ZERO).unwrap();
        let old = store.snapshot().items[0].revision;
        store.owner_gone(":1.1");
        store.ui(UiAction::Invoke {
            id,
            revision: old,
            key: "open".into(),
        });
        assert!(store.signals().is_empty());
        assert!(store.snapshot().items[0].orphaned);
        assert!(
            store.snapshot().banner.is_some(),
            "one-shot notify clients still get a banner"
        );
        store.close(":1.1", id).unwrap();
        assert_eq!(
            store.signals(),
            vec![Signal::Closed {
                owner: ":1.1".into(),
                id,
                reason: 3
            }]
        );
        assert!(!store.snapshot().items[0].live);
    }

    #[test]
    fn dnd_retains_items_critical_does_not_expire_and_limits_are_bounded() {
        let mut store = Store::default();
        store.ui(UiAction::Settings {
            dnd: true,
            hide_previews: true,
        });
        let mut n = input();
        n.urgency = 2;
        n.expire_ms = 1;
        store.notify(":1.1", 0, n, Duration::ZERO).unwrap();
        store.tick(Duration::from_secs(30));
        assert!(store.snapshot().items[0].live);
        assert_eq!(store.snapshot().banner, None);
        for _ in 1..MAX_ENTRIES {
            store.notify(":1.1", 0, input(), Duration::ZERO).unwrap();
        }
        assert_eq!(
            store.notify(":1.1", 0, input(), Duration::ZERO),
            Err(Error::Full)
        );
        store.ui(UiAction::Clear);
        assert!(store.snapshot().items.is_empty());
        assert_eq!(store.signals().len(), MAX_ENTRIES);
    }
}
