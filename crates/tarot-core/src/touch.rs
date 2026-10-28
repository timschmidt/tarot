//! Contact ownership survives motion and cancellation. Only one contact drives shell pointer UI.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    Shell,
    Application,
    Suppressed,
}

pub struct Begin {
    pub target: Target,
    pub cancel_shell: bool,
}

#[derive(Default, Debug)]
pub struct TouchRouter {
    contacts: BTreeMap<i32, Target>,
    cancelled: bool,
}

impl TouchRouter {
    pub fn begin(&mut self, id: i32, shell: bool) -> Begin {
        if self.contacts.is_empty() {
            self.cancelled = false;
        }
        if self.contacts.contains_key(&id) {
            return Begin {
                target: Target::Suppressed,
                cancel_shell: false,
            };
        }
        let cancel_shell = self
            .contacts
            .values()
            .any(|target| *target == Target::Shell);
        if cancel_shell {
            self.cancelled = true;
            for target in self.contacts.values_mut() {
                if *target == Target::Shell {
                    *target = Target::Suppressed;
                }
            }
        }
        let target = if self.cancelled || (shell && !self.contacts.is_empty()) {
            Target::Suppressed
        } else if shell {
            Target::Shell
        } else {
            Target::Application
        };
        self.contacts.insert(id, target);
        Begin {
            target,
            cancel_shell,
        }
    }

    pub fn target(&self, id: i32) -> Target {
        self.contacts
            .get(&id)
            .copied()
            .unwrap_or(Target::Suppressed)
    }

    pub fn has_shell(&self) -> bool {
        self.contacts
            .values()
            .any(|target| *target == Target::Shell)
    }
    pub fn has_contacts(&self) -> bool {
        !self.contacts.is_empty()
    }
    pub fn has_application(&self) -> bool {
        self.contacts
            .values()
            .any(|target| *target == Target::Application)
    }

    pub fn end(&mut self, id: i32) -> Target {
        self.contacts.remove(&id).unwrap_or(Target::Suppressed)
    }

    /// Retain contact IDs until their up events so held fingers cannot restart after a view change.
    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.contacts
            .values_mut()
            .for_each(|target| *target = Target::Suppressed);
    }

    /// Backend cancellation ends the entire sequence; no up events are required.
    pub fn reset(&mut self) {
        self.contacts.clear();
        self.cancelled = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_finger_cancels_shell_without_a_second_pointer_press() {
        let mut router = TouchRouter::default();
        assert_eq!(router.begin(1, true).target, Target::Shell);
        let second = router.begin(2, true);
        assert!(second.cancel_shell);
        assert_eq!(second.target, Target::Suppressed);
        assert_eq!(router.end(2), Target::Suppressed);
        assert_eq!(router.end(1), Target::Suppressed);
        assert_eq!(router.begin(3, true).target, Target::Shell);
    }

    #[test]
    fn app_multitouch_keeps_ownership_and_shell_cannot_steal_a_contact() {
        let mut router = TouchRouter::default();
        assert_eq!(router.begin(1, false).target, Target::Application);
        assert_eq!(router.begin(2, false).target, Target::Application);
        assert_eq!(router.begin(3, true).target, Target::Suppressed);
        assert_eq!(router.target(1), Target::Application);
    }

    #[test]
    fn second_contact_in_app_content_cannot_start_an_app_sequence_during_shell_cancellation() {
        let mut router = TouchRouter::default();
        router.begin(1, true);
        let second = router.begin(2, false);
        assert!(second.cancel_shell);
        assert_eq!(second.target, Target::Suppressed);
        assert_eq!(router.target(1), Target::Suppressed);
        assert_eq!(router.end(2), Target::Suppressed);
        assert_eq!(router.end(1), Target::Suppressed);
        assert_eq!(router.begin(3, false).target, Target::Application);
    }

    #[test]
    fn cancellation_suppresses_held_contacts_until_a_fresh_sequence() {
        let mut router = TouchRouter::default();
        router.begin(1, false);
        router.cancel();
        assert_eq!(router.begin(2, false).target, Target::Suppressed);
        assert_eq!(router.end(1), Target::Suppressed);
        assert_eq!(router.end(2), Target::Suppressed);
        assert_eq!(router.begin(3, false).target, Target::Application);
        router.reset();
        assert_eq!(router.begin(4, true).target, Target::Shell);
    }
}
