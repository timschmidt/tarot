//! Card order and contiguous stack membership. No process or focus side effects.
use crate::{Layout, ShellModel, View, WindowId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CardDrop {
    /// Join/reorder within the anchor's stack, next to that individual card.
    Into { anchor: WindowId, before: bool },
    /// Create an independent card before/after the anchor's entire stack.
    Separate { anchor: WindowId, before: bool },
}

impl CardDrop {
    pub fn anchor(self) -> WindowId {
        match self {
            Self::Into { anchor, .. } | Self::Separate { anchor, .. } => anchor,
        }
    }
}

impl ShellModel {
    /// Absolute deck positions; members expose a 48-unit strip, groups have a gap.
    pub fn card_positions(&self, layout: Layout) -> Vec<f32> {
        let mut position = 0.0;
        self.cards
            .iter()
            .enumerate()
            .map(|(index, card)| {
                if index > 0 {
                    position += if self.cards[index - 1].stack == card.stack {
                        48.0
                    } else {
                        layout.card(0, 0).width + 18.0
                    };
                }
                position
            })
            .collect()
    }

    pub fn stack_members(&self, id: WindowId) -> Vec<WindowId> {
        let Some(stack) = self
            .cards
            .iter()
            .find(|card| card.id == id)
            .map(|card| card.stack)
        else {
            return Vec::new();
        };
        self.cards
            .iter()
            .filter(|card| card.stack == stack)
            .map(|card| card.id)
            .collect()
    }

    pub(crate) fn place_card(&mut self, id: WindowId, target: CardDrop) {
        if self.view != View::Overview || id == target.anchor() {
            return;
        }
        let Some(source) = self
            .cards
            .iter()
            .position(|card| card.id == id && !card.closing)
        else {
            return;
        };
        let Some(anchor) = self
            .cards
            .iter()
            .find(|card| card.id == target.anchor() && !card.closing)
        else {
            return;
        };
        let stack = anchor.stack;
        let mut card = self.cards.remove(source);
        let index = match target {
            CardDrop::Into { anchor, before } => {
                card.stack = stack;
                self.cards
                    .iter()
                    .position(|card| card.id == anchor)
                    .unwrap()
                    + usize::from(!before)
            }
            CardDrop::Separate { before, .. } => {
                card.stack = self.new_stack();
                if before {
                    self.cards
                        .iter()
                        .position(|card| card.stack == stack)
                        .unwrap()
                } else {
                    self.cards
                        .iter()
                        .rposition(|card| card.stack == stack)
                        .unwrap()
                        + 1
                }
            }
        };
        self.cards.insert(index, card);
        self.selected = Some(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;

    fn fixture() -> ShellModel {
        let mut model = ShellModel::default();
        for id in 1..=4 {
            model.add_window(WindowId(id), id.to_string(), "same.app".into());
        }
        model.apply(Command::Overview);
        model
    }

    #[test]
    fn grouping_reordering_and_extracting_preserve_windows_and_contiguous_stacks() {
        let mut model = fixture();
        for id in [1, 4] {
            assert!(
                model
                    .apply(Command::Stack {
                        card: WindowId(id),
                        with: WindowId(3)
                    })
                    .is_empty()
            );
        }
        assert_eq!(
            model.stack_members(WindowId(3)),
            vec![WindowId(3), WindowId(4), WindowId(1)]
        );
        model.apply(Command::PlaceCard {
            card: WindowId(1),
            target: CardDrop::Into {
                anchor: WindowId(3),
                before: true,
            },
        });
        assert_eq!(
            model.stack_members(WindowId(3)),
            vec![WindowId(1), WindowId(3), WindowId(4)]
        );
        model.apply(Command::Unstack(WindowId(3)));
        assert_eq!(
            model.stack_members(WindowId(1)),
            vec![WindowId(1), WindowId(4)]
        );
        assert_eq!(model.stack_members(WindowId(3)), vec![WindowId(3)]);
        let order: Vec<_> = model.cards.iter().map(|c| c.id).collect();
        assert_eq!(
            order,
            vec![WindowId(2), WindowId(1), WindowId(4), WindowId(3)]
        );
        assert_eq!(model.selected, Some(WindowId(3)));
        assert_eq!(model.view, View::Overview);
        let stack = model.cards[1].stack;
        model.remove_window(WindowId(1));
        assert_eq!(model.cards[1].id, WindowId(4));
        assert_eq!(model.cards[1].stack, stack);
    }

    #[test]
    fn closing_a_focused_stack_member_returns_to_its_sibling() {
        let mut model = fixture();
        model.apply(Command::Stack {
            card: WindowId(1),
            with: WindowId(2),
        });
        model.apply(Command::Activate(WindowId(1)));
        assert_eq!(
            model.remove_window(WindowId(1)),
            vec![crate::Effect::Focus(Some(WindowId(2)))]
        );
        assert_eq!(model.view, View::App(WindowId(2)));
        assert_eq!(model.stack_members(WindowId(2)), vec![WindowId(2)]);
    }

    #[test]
    fn stale_self_and_closing_targets_cannot_reorder_a_deck() {
        let mut model = fixture();
        model.apply(Command::Close(WindowId(3)));
        let before = model.cards.clone();
        for anchor in [WindowId(1), WindowId(3), WindowId(999)] {
            model.apply(Command::Stack {
                card: WindowId(1),
                with: anchor,
            });
        }
        assert_eq!(model.cards, before);
        model.apply(Command::Activate(WindowId(2)));
        model.apply(Command::Stack {
            card: WindowId(1),
            with: WindowId(4),
        });
        assert_eq!(model.cards, before);
    }
}
