//! Origin and axis ownership are fixed for a sequence. App scrolling stays app-owned.

use crate::{CardDrop, Command, Layout, ShellModel, StackId, View, WindowId, motion::Presentation};
use std::{collections::VecDeque, time::Duration};

const SLOP: f32 = crate::design::AXIS_SLOP;
const VELOCITY_WINDOW_MS: u64 = 120;
pub const HOLD_MS: u64 = crate::design::REORDER_HOLD_MS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Owner {
    Shell,
    Application,
}

#[derive(Clone, Copy, Debug)]
enum Origin {
    Strip,
    Cards(Option<WindowId>),
    OrdinaryUi,
    Application,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Preview {
    QuickLaunch(crate::quick_launch::Wave),
    Overview {
        progress: f32,
    },
    Browse {
        offset: f32,
    },
    Dismiss {
        id: WindowId,
        offset: f32,
    },
    Arrange {
        id: WindowId,
        delta: (f32, f32),
        deck_offset: f32,
        target: Option<CardDrop>,
    },
}

#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub consumed: bool,
    pub command: Option<Command>,
}

#[derive(Clone, Copy, Debug)]
pub struct Start {
    pub id: i32,
    pub position: (f32, f32),
    pub time_ms: u64,
}

#[derive(Clone, Copy, Debug)]
enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug)]
struct Contact {
    start: Start,
    position: (f32, f32),
    origin: Origin,
    view: View,
    selected: Option<WindowId>,
    selected_index: usize,
    card_count: usize,
    positions: Vec<f32>,
    deck: Vec<(WindowId, StackId)>,
    original: Presentation,
    layout: Layout,
    arranging: bool,
    quick_launch: bool,
    wave_candidate_at: Option<u64>,
    favorites: crate::quick_launch::Favorites,
    deck_index: usize,
    deck_offset: f32,
    edge: Option<(i32, u64)>,
    time_ms: u64,
    height: f32,
    axis: Option<Axis>,
    samples: VecDeque<(u64, (f32, f32))>,
}

impl Contact {
    fn tick(&mut self, time_ms: u64) {
        self.time_ms = self.time_ms.max(time_ms);
        if !self.quick_launch
            && matches!(self.origin, Origin::Strip)
            && matches!(self.axis, Some(Axis::Vertical))
            && self.delta().1 < -crate::design::WAVE_DRAG
        {
            let since = *self.wave_candidate_at.get_or_insert(self.time_ms);
            if self.time_ms.saturating_sub(since) >= crate::design::WAVE_DWELL_MS {
                self.quick_launch = true;
            }
        } else if !self.quick_launch {
            self.wave_candidate_at = None;
        }
        if !self.arranging
            && self.axis.is_none()
            && matches!(self.origin, Origin::Cards(Some(_)))
            && self.delta().0.abs().max(self.delta().1.abs()) <= crate::design::TAP_SLOP
            && self.time_ms.saturating_sub(self.start.time_ms) >= HOLD_MS
        {
            self.arranging = true;
        }
        if !self.arranging {
            return;
        }
        let direction = if self.position.0 < 28.0 && self.deck_index > 0 {
            -1
        } else if self.position.0 > self.layout.width - 28.0
            && self.deck_index + 1 < self.card_count
        {
            1
        } else {
            0
        };
        if direction == 0 {
            self.edge = None;
            return;
        }
        if !self.edge.is_some_and(|(old, _)| old == direction) {
            self.edge = Some((direction, self.time_ms + 350));
        }
        if let Some((_, deadline)) = self.edge
            && self.time_ms >= deadline
        {
            self.deck_index = (self.deck_index as i64 + i64::from(direction))
                .clamp(0, self.card_count.saturating_sub(1) as i64)
                as usize;
            self.deck_offset =
                self.positions[self.selected_index] - self.positions[self.deck_index];
            self.edge = Some((direction, self.time_ms + 500));
        }
    }

    fn drop_target(&self, source: WindowId) -> Option<CardDrop> {
        let (x, y) = self.position;
        if x < 0.0 || x >= self.layout.width || y < Layout::STATUS || y >= self.layout.gesture().y {
            return None;
        }
        let candidates: Vec<_> = self
            .original
            .windows
            .iter()
            .rev()
            .filter(|w| w.id != source && w.opacity > 0.01)
            .collect();
        if y >= self.layout.height - Layout::GESTURE - Layout::DOCK - 48.0 {
            let anchor = candidates.iter().min_by(|a, b| {
                let center =
                    |rect: crate::Rect| (rect.x + rect.width / 2.0 + self.deck_offset - x).abs();
                center(a.rect).total_cmp(&center(b.rect))
            })?;
            Some(CardDrop::Separate {
                anchor: anchor.id,
                before: x < anchor.rect.x + anchor.rect.width / 2.0 + self.deck_offset,
            })
        } else {
            let anchor = candidates
                .into_iter()
                .find(|w| w.contains(x - self.deck_offset, y))?;
            Some(CardDrop::Into {
                anchor: anchor.id,
                before: x < anchor.rect.x + anchor.rect.width / 2.0 + self.deck_offset,
            })
        }
    }

    fn delta(&self) -> (f32, f32) {
        (
            self.position.0 - self.start.position.0,
            self.position.1 - self.start.position.1,
        )
    }

    fn sample(&mut self, time: u64) {
        let time = time.max(self.samples.back().map_or(time, |(time, _)| *time));
        self.samples.push_back((time, self.position));
        while self.samples.len() > 1
            && self
                .samples
                .front()
                .is_some_and(|(old, _)| time.saturating_sub(*old) > VELOCITY_WINDOW_MS)
        {
            self.samples.pop_front();
        }
        while self.samples.len() > 32 {
            self.samples.pop_front();
        }
    }

    fn velocity(&self) -> (f32, f32) {
        let Some(&(first, origin)) = self.samples.front() else {
            return (0.0, 0.0);
        };
        let Some(&(last, position)) = self.samples.back() else {
            return (0.0, 0.0);
        };
        let elapsed = last.saturating_sub(first) as f32;
        if elapsed < 8.0 {
            return (0.0, 0.0);
        }
        (
            (position.0 - origin.0) / elapsed,
            (position.1 - origin.1) / elapsed,
        )
    }

    fn owner(&self) -> Owner {
        if matches!(self.origin, Origin::Application) {
            Owner::Application
        } else {
            Owner::Shell
        }
    }
}

#[derive(Debug, Default)]
pub struct GestureRecognizer {
    contact: Option<Contact>,
    cancelled: bool,
}

impl GestureRecognizer {
    pub fn begin(
        &mut self,
        start: Start,
        layout: Layout,
        model: &ShellModel,
        presentation: &Presentation,
    ) -> Owner {
        if let Some(contact) = &self.contact {
            self.cancelled = true;
            return contact.owner();
        }
        self.cancelled = false;
        let (x, y) = start.position;
        let origin = if model.card_menu_open
            || crate::notifications::overlay_contains(model, layout, x, y)
        {
            Origin::OrdinaryUi
        } else if layout.gesture().contains(x, y) {
            Origin::Strip
        } else if model.view == View::Overview
            && !model.cards.is_empty()
            && y >= Layout::STATUS
            && y < layout.height - Layout::GESTURE - Layout::DOCK
        {
            Origin::Cards(presentation.card_at(x, y, model.selected).filter(|id| {
                model
                    .cards
                    .iter()
                    .any(|card| card.id == *id && !card.closing)
            }))
        } else if matches!(model.view, View::App(_)) && layout.content().contains(x, y) {
            Origin::Application
        } else {
            Origin::OrdinaryUi
        };
        let selected_index = model
            .cards
            .iter()
            .position(|card| Some(card.id) == model.selected)
            .unwrap_or(0);
        let contact = Contact {
            start,
            position: start.position,
            origin,
            view: model.view,
            selected: model.selected,
            selected_index,
            card_count: model.cards.len(),
            positions: model.card_positions(layout),
            deck: model
                .cards
                .iter()
                .map(|card| (card.id, card.stack))
                .collect(),
            original: presentation.clone(),
            layout,
            arranging: false,
            quick_launch: false,
            wave_candidate_at: None,
            favorites: model.favorites.clone(),
            deck_index: selected_index,
            deck_offset: 0.0,
            edge: None,
            time_ms: start.time_ms,
            height: layout.height,
            axis: None,
            samples: VecDeque::from([(start.time_ms, start.position)]),
        };
        let owner = contact.owner();
        self.contact = Some(contact);
        owner
    }

    pub fn update(&mut self, id: i32, x: f32, y: f32, time_ms: u64) {
        if let Some(contact) = &mut self.contact
            && contact.start.id == id
        {
            contact.tick(time_ms);
            contact.position = (x, y);
            contact.sample(time_ms);
            if !contact.arranging
                && contact.axis.is_none()
                && matches!(contact.origin, Origin::Strip | Origin::Cards(_))
            {
                let (dx, dy) = contact.delta();
                if dx.abs().max(dy.abs()) >= SLOP {
                    contact.axis = Some(if dx.abs() > dy.abs() {
                        Axis::Horizontal
                    } else {
                        Axis::Vertical
                    });
                }
            }
            contact.tick(time_ms);
        }
    }

    pub fn tick_elapsed(&mut self, elapsed: Duration) {
        if !self.cancelled
            && let Some(contact) = &mut self.contact
        {
            contact.tick(
                contact
                    .start
                    .time_ms
                    .saturating_add(elapsed.as_millis().min(u128::from(u64::MAX)) as u64),
            );
        }
    }

    pub fn wake_after(&self) -> Option<Duration> {
        if self.cancelled {
            return None;
        }
        let contact = self.contact.as_ref()?;
        let deadline = if contact.arranging {
            contact.edge.map(|(_, time)| time)?
        } else if contact.axis.is_none()
            && matches!(contact.origin, Origin::Cards(Some(_)))
            && contact.delta().0.abs().max(contact.delta().1.abs()) <= crate::design::TAP_SLOP
        {
            contact.start.time_ms + HOLD_MS
        } else if !contact.quick_launch
            && let Some(since) = contact.wave_candidate_at
        {
            since + crate::design::WAVE_DWELL_MS
        } else {
            return None;
        };
        Some(Duration::from_millis(
            deadline.saturating_sub(contact.time_ms).max(1),
        ))
    }

    pub fn is_arranging(&self) -> bool {
        self.contact.as_ref().is_some_and(|c| c.arranging) && !self.cancelled
    }

    pub fn preview(&self) -> Option<Preview> {
        if self.cancelled {
            return None;
        }
        let contact = self.contact.as_ref()?;
        if contact.quick_launch {
            let mut wave = crate::quick_launch::wave(contact.layout, contact.position);
            wave.selected = wave.selected.filter(|i| {
                *i == crate::quick_launch::CAPACITY || *i < contact.favorites.applications.len()
            });
            return Some(Preview::QuickLaunch(wave));
        }
        let (dx, dy) = contact.delta();
        if contact.arranging
            && let Origin::Cards(Some(id)) = contact.origin
        {
            return Some(Preview::Arrange {
                id,
                delta: (dx, dy),
                deck_offset: contact.deck_offset,
                target: contact.drop_target(id),
            });
        }
        match (contact.origin, contact.axis) {
            (Origin::Strip, Some(Axis::Vertical)) if matches!(contact.view, View::App(_)) => {
                Some(Preview::Overview {
                    progress: (-dy / 160.0).clamp(0.0, 1.0),
                })
            }
            (Origin::Cards(_), Some(Axis::Horizontal)) => {
                let min = contact.positions[contact.selected_index]
                    - contact.positions.last().copied().unwrap_or(0.0);
                let max = contact.positions[contact.selected_index];
                let clamped = dx.clamp(min, max);
                Some(Preview::Browse {
                    offset: clamped + (dx - clamped) * 0.25,
                })
            }
            (Origin::Cards(Some(id)), Some(Axis::Vertical)) => Some(Preview::Dismiss {
                id,
                offset: if dy <= 0.0 { dy } else { dy * 0.25 },
            }),
            _ => None,
        }
    }

    pub fn finish(&mut self, id: i32, time_ms: u64, model: &ShellModel) -> Outcome {
        if self
            .contact
            .as_ref()
            .is_some_and(|contact| contact.start.id != id)
        {
            return Outcome::default();
        }
        let Some(mut contact) = self.contact.take() else {
            return Outcome::default();
        };
        contact.tick(time_ms);
        let consumed = contact.axis.is_some() || contact.arranging;
        if self.cancelled
            || (matches!(contact.origin, Origin::Strip | Origin::Cards(_))
                && (model.system_menu_open || model.notifications_open || model.card_menu_open))
            || contact.favorites != model.favorites
            || contact.view != model.view
            || contact.selected != model.selected
            || contact.deck
                != model
                    .cards
                    .iter()
                    .map(|card| (card.id, card.stack))
                    .collect::<Vec<_>>()
        {
            return Outcome {
                consumed: true,
                command: None,
            };
        }
        if contact.quick_launch {
            let selected = crate::quick_launch::wave(contact.layout, contact.position).selected;
            return Outcome {
                consumed: true,
                command: selected
                    .filter(|i| {
                        *i == crate::quick_launch::CAPACITY
                            || model.favorites.applications.get(*i).is_some_and(|id| {
                                model
                                    .applications
                                    .iter()
                                    .any(|a| a.id == *id && a.unavailable.is_none())
                            })
                    })
                    .map(Command::QuickLaunch),
            };
        }
        if contact.arranging
            && let Origin::Cards(Some(id)) = contact.origin
        {
            return Outcome {
                consumed: true,
                command: contact
                    .drop_target(id)
                    .map(|target| Command::PlaceCard { card: id, target }),
            };
        }
        contact.sample(time_ms); // A pause before release removes stale flick velocity.
        let (dx, dy) = contact.delta();
        let (vx, vy) = contact.velocity();
        let command = match (contact.origin, contact.axis) {
            (Origin::Cards(Some(id)), None)
                if dx.abs().max(dy.abs()) <= crate::design::TAP_SLOP =>
            {
                Some(Command::Activate(id))
            }
            (Origin::Strip, Some(Axis::Vertical)) if dy < -48.0 || (dy < -16.0 && vy < -0.5) => {
                Some(match contact.view {
                    View::App(_) | View::Launcher => Command::Overview,
                    _ => Command::Launcher,
                })
            }
            (Origin::Cards(_), Some(Axis::Horizontal)) => {
                let projected = contact.positions[contact.selected_index] - dx - vx * 120.0;
                let nearest = contact
                    .positions
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        (**a - projected).abs().total_cmp(&(**b - projected).abs())
                    })
                    .map_or(contact.selected_index, |(index, _)| index);
                let steps = nearest as i32 - contact.selected_index as i32;
                if steps != 0 {
                    Some(Command::BrowseCards(steps))
                } else {
                    None
                }
            }
            (Origin::Cards(Some(id)), Some(Axis::Vertical))
                if dy < -(contact.height * crate::design::DISMISS_DISTANCE).max(80.0)
                    || (dy < -24.0 && vy <= -crate::design::DISMISS_VELOCITY / 1000.0) =>
            {
                Some(Command::Close(id))
            }
            _ => None,
        };
        Outcome {
            consumed: consumed || matches!(command.as_ref(), Some(Command::Activate(_))),
            command,
        }
    }

    pub fn cancel(&mut self) {
        self.contact = None;
        self.cancelled = true;
    }
    pub fn owner(&self) -> Option<Owner> {
        self.contact.as_ref().map(Contact::owner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upward_hold_selects_one_favorite_without_an_overview_command() {
        let (mut model, layout, mut recognizer) = fixture(View::App(WindowId(2)));
        model.applications.push(crate::Application {
            id: "probe.desktop".into(),
            name: "Probe".into(),
            description: String::new(),
            icon: None,
            keywords: vec![],
            unavailable: None,
        });
        model.favorites.applications.push("probe.desktop".into());
        begin(&mut recognizer, &model, layout, (180.0, 710.0));
        recognizer.update(1, 180.0, 610.0, 100);
        recognizer.update(1, 40.0, 610.0, 150);
        recognizer.tick_elapsed(Duration::from_millis(450));
        assert!(matches!(
            recognizer.preview(),
            Some(Preview::QuickLaunch(crate::quick_launch::Wave {
                selected: Some(0),
                ..
            }))
        ));
        assert!(recognizer.wake_after().is_none());
        assert_eq!(
            recognizer.finish(1, 460, &model),
            Outcome {
                consumed: true,
                command: Some(Command::QuickLaunch(0))
            }
        );
        assert_eq!(recognizer.finish(1, 470, &model), Outcome::default());
    }

    #[test]
    fn short_swipe_keeps_navigation_and_wave_release_outside_cancels() {
        for (release, position, expected) in [
            (200, (180.0, 610.0), Some(Command::Overview)),
            (460, (320.0, 610.0), Some(Command::QuickLaunch(4))),
            (460, (180.0, 610.0), None), // An empty favorite position.
            (460, (-10.0, 610.0), None),
            (460, (320.0, 10.0), None),
        ] {
            let (model, layout, mut recognizer) = fixture(View::App(WindowId(2)));
            begin(&mut recognizer, &model, layout, (180.0, 710.0));
            // Fix vertical ownership before sliding to a target horizontally.
            recognizer.update(1, 180.0, 610.0, 100);
            recognizer.tick_elapsed(Duration::from_millis(release));
            recognizer.update(1, position.0, position.1, release);
            assert_eq!(recognizer.finish(1, release, &model).command, expected);
        }
    }

    #[test]
    fn wave_cannot_survive_modal_favorite_change_second_contact_or_cancel() {
        for interruption in 0..4 {
            let (mut model, layout, mut recognizer) = fixture(View::App(WindowId(2)));
            begin(&mut recognizer, &model, layout, (180.0, 710.0));
            recognizer.update(1, 180.0, 610.0, 100);
            recognizer.tick_elapsed(Duration::from_millis(450));
            recognizer.update(1, 320.0, 610.0, 460);
            match interruption {
                0 => model.system_menu_open = true,
                1 => model.favorites.applications.push("changed.desktop".into()),
                2 => {
                    recognizer.begin(
                        Start {
                            id: 2,
                            position: (200.0, 600.0),
                            time_ms: 461,
                        },
                        layout,
                        &model,
                        &Presentation::settled(&model, layout),
                    );
                }
                _ => recognizer.cancel(),
            }
            assert_eq!(recognizer.finish(1, 470, &model).command, None);
        }
    }

    fn fixture(view: View) -> (ShellModel, Layout, GestureRecognizer) {
        let mut model = ShellModel::default();
        for id in 1..=3 {
            model.add_window(WindowId(id), id.to_string(), "probe".into());
        }
        model.selected = Some(WindowId(2));
        model.view = view;
        (
            model,
            Layout {
                width: 360.0,
                height: 720.0,
                keyboard_height: 0.0,
            },
            GestureRecognizer::default(),
        )
    }

    fn begin(
        recognizer: &mut GestureRecognizer,
        model: &ShellModel,
        layout: Layout,
        position: (f32, f32),
    ) -> Owner {
        recognizer.begin(
            Start {
                id: 1,
                position,
                time_ms: 0,
            },
            layout,
            model,
            &Presentation::settled(model, layout),
        )
    }

    #[test]
    fn notification_banner_owns_a_hold_over_a_card() {
        let (mut model, layout, mut recognizer) = fixture(View::Overview);
        model.notifications.banner = Some((1, 1));
        let banner = crate::notifications::banner_rect(layout);
        begin(&mut recognizer, &model, layout, (180.0, banner.y + 20.0));
        recognizer.tick_elapsed(Duration::from_secs(1));
        assert_eq!(recognizer.preview(), None);
        assert_eq!(recognizer.finish(1, 1_100, &model), Outcome::default());
    }

    #[test]
    fn empty_overview_has_no_card_hold_or_browse_sequence() {
        let (mut model, layout, mut recognizer) = fixture(View::Overview);
        model.cards.clear();
        model.selected = None;
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.update(1, 20.0, 300.0, 100);
        recognizer.tick_elapsed(Duration::from_secs(1));
        assert_eq!(recognizer.preview(), None);
        assert_eq!(recognizer.finish(1, 1_100, &model), Outcome::default());
    }

    #[test]
    fn a_card_menu_owns_its_input_even_over_an_underlying_card() {
        let (mut model, layout, mut recognizer) = fixture(View::Overview);
        model.apply(Command::CardMenuOpen(true));
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.tick_elapsed(Duration::from_millis(1_000));
        recognizer.update(1, 20.0, 100.0, 1_050);
        assert_eq!(recognizer.preview(), None);
        assert_eq!(recognizer.wake_after(), None);
        assert_eq!(recognizer.finish(1, 1_100, &model), Outcome::default());
    }

    #[test]
    fn hold_pages_the_deck_and_drop_groups_only_on_release() {
        let (mut model, layout, mut recognizer) = fixture(View::Overview);
        let original = model.cards.clone();
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        assert_eq!(
            recognizer.wake_after(),
            Some(Duration::from_millis(HOLD_MS))
        );
        recognizer.tick_elapsed(Duration::from_millis(HOLD_MS));
        assert!(recognizer.is_arranging());
        assert_eq!(recognizer.wake_after(), None);
        recognizer.update(1, 345.0, 300.0, 500);
        recognizer.tick_elapsed(Duration::from_millis(850));
        recognizer.update(1, 210.0, 300.0, 900);
        let target = CardDrop::Into {
            anchor: WindowId(3),
            before: false,
        };
        assert!(
            matches!(recognizer.preview(),Some(Preview::Arrange { id:WindowId(2), target:Some(found), deck_offset,.. }) if found == target && deck_offset < -(layout.width*crate::design::CARD_SCALE))
        );
        assert_eq!(model.cards, original, "preview cannot mutate the deck");
        let outcome = recognizer.finish(1, 920, &model);
        assert!(outcome.consumed);
        assert_eq!(
            outcome.command,
            Some(Command::PlaceCard {
                card: WindowId(2),
                target
            })
        );
        model.apply(outcome.command.unwrap());
        assert_eq!(
            model.stack_members(WindowId(3)),
            vec![WindowId(3), WindowId(2)]
        );
        assert_eq!(model.view, View::Overview);
    }

    #[test]
    fn a_stack_member_can_be_reordered_then_extracted_via_the_separate_rail() {
        let (mut model, layout, mut recognizer) = fixture(View::Overview);
        model.apply(Command::Stack {
            card: WindowId(2),
            with: WindowId(3),
        });
        let frame = Presentation::settled(&model, layout);
        assert_eq!(
            frame.card_at(60.0, 300.0, model.selected),
            Some(WindowId(3))
        );
        begin(&mut recognizer, &model, layout, (60.0, 300.0)); // Exposed fan of card 3.
        recognizer.tick_elapsed(Duration::from_millis(HOLD_MS));
        recognizer.update(1, 210.0, 300.0, 500);
        model.apply(recognizer.finish(1, 550, &model).command.unwrap());
        assert_eq!(
            model.stack_members(WindowId(3)),
            vec![WindowId(2), WindowId(3)]
        );
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.tick_elapsed(Duration::from_millis(HOLD_MS));
        recognizer.update(1, 180.0, 590.0, 500);
        let target = CardDrop::Separate {
            anchor: WindowId(2),
            before: false,
        };
        assert_eq!(
            recognizer.finish(1, 550, &model).command,
            Some(Command::PlaceCard {
                card: WindowId(3),
                target
            })
        );
        model.apply(Command::PlaceCard {
            card: WindowId(3),
            target,
        });
        assert_eq!(model.stack_members(WindowId(3)), vec![WindowId(3)]);
        assert_eq!(model.stack_members(WindowId(2)), vec![WindowId(2)]);
    }

    #[test]
    fn moving_before_the_hold_and_interruption_cannot_accidentally_arrange() {
        let (mut model, layout, mut recognizer) = fixture(View::Overview);
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.update(1, 140.0, 300.0, 100);
        recognizer.tick_elapsed(Duration::from_millis(800));
        assert!(!recognizer.is_arranging());
        recognizer.cancel();
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.tick_elapsed(Duration::from_millis(HOLD_MS));
        recognizer.begin(
            Start {
                id: 2,
                position: (100.0, 300.0),
                time_ms: 500,
            },
            layout,
            &model,
            &Presentation::settled(&model, layout),
        );
        assert_eq!(recognizer.preview(), None);
        assert_eq!(recognizer.finish(1, 550, &model).command, None);
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.tick_elapsed(Duration::from_millis(HOLD_MS));
        model.remove_window(WindowId(3));
        assert_eq!(recognizer.finish(1, 550, &model).command, None);
    }

    #[test]
    fn ordinary_app_and_launcher_scrolls_never_navigate() {
        for view in [View::App(WindowId(2)), View::Launcher, View::Home] {
            let (model, layout, mut recognizer) = fixture(view);
            let owner = begin(&mut recognizer, &model, layout, (180.0, 600.0));
            assert_eq!(
                owner,
                if matches!(view, View::App(_)) {
                    Owner::Application
                } else {
                    Owner::Shell
                }
            );
            recognizer.update(1, 180.0, 200.0, 100);
            assert_eq!(recognizer.preview(), None);
            assert_eq!(recognizer.finish(1, 120, &model), Outcome::default());
        }
    }

    #[test]
    fn strip_swipe_follows_finger_commits_once_and_cancel_cannot_commit() {
        let (model, layout, mut recognizer) = fixture(View::App(WindowId(2)));
        begin(&mut recognizer, &model, layout, (180.0, 710.0));
        recognizer.update(1, 180.0, 630.0, 100);
        assert_eq!(
            recognizer.preview(),
            Some(Preview::Overview { progress: 0.5 })
        );
        assert_eq!(
            recognizer.finish(1, 110, &model),
            Outcome {
                consumed: true,
                command: Some(Command::Overview)
            }
        );
        assert_eq!(recognizer.finish(1, 120, &model), Outcome::default());
        begin(&mut recognizer, &model, layout, (180.0, 710.0));
        recognizer.update(1, 180.0, 600.0, 100);
        recognizer.cancel();
        assert_eq!(recognizer.finish(1, 120, &model), Outcome::default());
    }

    #[test]
    fn launcher_scroll_and_strip_toggle_have_distinct_origins() {
        let (model, layout, mut recognizer) = fixture(View::Launcher);
        begin(&mut recognizer, &model, layout, (180.0, 710.0));
        recognizer.update(1, 180.0, 600.0, 100);
        assert_eq!(
            recognizer.finish(1, 110, &model).command,
            Some(Command::Overview)
        );
    }

    #[test]
    fn horizontal_axis_stays_owned_when_the_path_turns_upward() {
        let (mut model, layout, mut recognizer) = fixture(View::Overview);
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.update(1, 140.0, 298.0, 30);
        recognizer.update(1, -20.0, 100.0, 160);
        assert!(matches!(recognizer.preview(), Some(Preview::Browse { .. })));
        let outcome = recognizer.finish(1, 170, &model);
        assert!(matches!(outcome.command, Some(Command::BrowseCards(_))));
        model.apply(outcome.command.unwrap());
        assert_eq!(model.selected, Some(WindowId(3)));
        assert_eq!(model.view, View::Overview);
    }

    #[test]
    fn short_drag_consumes_the_pointer_but_does_not_activate_or_dismiss() {
        let (model, layout, mut recognizer) = fixture(View::Overview);
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.update(1, 180.0, 270.0, 300);
        assert_eq!(
            recognizer.finish(1, 400, &model),
            Outcome {
                consumed: true,
                command: None
            }
        );
    }

    #[test]
    fn recent_velocity_commits_a_flick_but_a_pause_removes_that_velocity() {
        for (release, expected) in [(50, Some(Command::Close(WindowId(2)))), (500, None)] {
            let (model, layout, mut recognizer) = fixture(View::Overview);
            begin(&mut recognizer, &model, layout, (180.0, 300.0));
            recognizer.update(1, 180.0, 255.0, 40);
            assert_eq!(recognizer.finish(1, release, &model).command, expected);
        }
    }

    #[test]
    fn second_contact_or_changed_view_cannot_finish_an_old_gesture() {
        let (mut model, layout, mut recognizer) = fixture(View::Overview);
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.update(1, 180.0, 100.0, 150);
        recognizer.begin(
            Start {
                id: 2,
                position: (160.0, 300.0),
                time_ms: 160,
            },
            layout,
            &model,
            &Presentation::settled(&model, layout),
        );
        assert_eq!(recognizer.finish(1, 170, &model).command, None);
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.update(1, 180.0, 100.0, 150);
        model.view = View::Home;
        assert_eq!(recognizer.finish(1, 170, &model).command, None);
    }

    #[test]
    fn the_edge_of_the_deck_resists_dragging_past_the_last_card() {
        let (mut model, layout, mut recognizer) = fixture(View::Overview);
        model.selected = Some(WindowId(3));
        begin(&mut recognizer, &model, layout, (180.0, 300.0));
        recognizer.update(1, 60.0, 300.0, 100);
        assert_eq!(
            recognizer.preview(),
            Some(Preview::Browse { offset: -30.0 })
        );
    }
}
