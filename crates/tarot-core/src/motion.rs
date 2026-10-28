//! One deterministic presentation shared by chrome, native surfaces, and hit testing.
//! Time is supplied by the host; settling never depends on the number of frames.

use serde::Serialize;
use std::time::Duration;

use crate::{CardDrop, Layout, Rect, ShellModel, View, WindowId, gesture::Preview};

pub const SETTLE: Duration = Duration::from_millis(crate::design::MINIMIZE_MS);

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct WindowVisual {
    pub id: WindowId,
    pub rect: Rect,
    pub opacity: f32,
    /// Logical radius, interpolated with the native surface geometry.
    pub corner_radius: f32,
}

impl WindowVisual {
    pub fn contains(self, x: f32, y: f32) -> bool {
        if !self.rect.contains(x, y) {
            return false;
        }
        let radius = self
            .corner_radius
            .max(0.0)
            .min(self.rect.width.min(self.rect.height) * 0.5);
        let nearest_x = x.clamp(self.rect.x + radius, self.rect.x + self.rect.width - radius);
        let nearest_y = y.clamp(
            self.rect.y + radius,
            self.rect.y + self.rect.height - radius,
        );
        (x - nearest_x).hypot(y - nearest_y) <= radius
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Presentation {
    pub windows: Vec<WindowVisual>,
    pub overview: f32,
    pub dock: f32,
    pub arrange: Option<ArrangeVisual>,
    pub quick_launch: Option<crate::quick_launch::Wave>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct ArrangeVisual {
    pub id: WindowId,
    pub target: Option<CardDrop>,
}

impl Default for Presentation {
    fn default() -> Self {
        Self {
            windows: Vec::new(),
            overview: 0.0,
            dock: 1.0,
            arrange: None,
            quick_launch: None,
        }
    }
}

impl Presentation {
    pub fn settled(model: &ShellModel, layout: Layout) -> Self {
        Self::for_view(model, layout, model.view)
    }

    fn for_view(model: &ShellModel, layout: Layout, view: View) -> Self {
        let selected = model
            .cards
            .iter()
            .position(|card| Some(card.id) == model.selected)
            .unwrap_or(0);
        let positions = model.card_positions(layout);
        let selected_position = positions.get(selected).copied().unwrap_or(0.0);
        let selected_stack = model.cards.get(selected).map(|card| card.stack);
        let mut frame = Self {
            windows: model
                .cards
                .iter()
                .enumerate()
                .map(|(index, card)| WindowVisual {
                    id: card.id,
                    rect: if view == View::App(card.id) {
                        layout.content()
                    } else {
                        let mut rect = layout
                            .card(0, 0)
                            .translated(positions[index] - selected_position, 0.0);
                        if Some(card.stack) == selected_stack {
                            rect.y += (index as f32 - selected as f32).clamp(-3.0, 3.0) * 12.0;
                        }
                        rect
                    },
                    opacity: if view == View::Overview || view == View::App(card.id) {
                        1.0
                    } else {
                        0.0
                    },
                    corner_radius: if view == View::App(card.id) {
                        0.0
                    } else {
                        let card = layout.card(0, 0);
                        (crate::design::CARD_RADIUS * card.width / layout.content().width.max(1.0))
                            .min(card.width.min(card.height) * 0.5)
                    },
                })
                .collect(),
            overview: if view == View::Overview { 1.0 } else { 0.0 },
            dock: if matches!(view, View::App(_)) {
                0.0
            } else {
                1.0
            },
            arrange: None,
            quick_launch: None,
        };
        // Back-to-front order is shared by native drawing, chrome and hit tests.
        frame
            .windows
            .sort_by_key(|window| Some(window.id) == model.selected);
        frame
    }

    pub fn window(&self, id: WindowId) -> Option<&WindowVisual> {
        self.windows.iter().find(|window| window.id == id)
    }

    pub fn card_at(&self, x: f32, y: f32, _selected: Option<WindowId>) -> Option<WindowId> {
        let hit = |window: &&WindowVisual| window.opacity > 0.01 && window.contains(x, y);
        self.windows.iter().rev().find(hit).map(|window| window.id)
    }

    pub fn visible(&self, id: WindowId, layout: Layout) -> bool {
        self.window(id)
            .is_some_and(|window| window.opacity > 0.01 && window.rect.intersects(layout.content()))
    }

    fn blend(&self, target: &Self, progress: f32) -> Self {
        let mix = |a: f32, b: f32| a + (b - a) * progress;
        Self {
            windows: target
                .windows
                .iter()
                .map(|end| {
                    let start = self.window(end.id).copied().unwrap_or(WindowVisual {
                        opacity: 0.0,
                        ..*end
                    });
                    WindowVisual {
                        id: end.id,
                        rect: Rect {
                            x: mix(start.rect.x, end.rect.x),
                            y: mix(start.rect.y, end.rect.y),
                            width: mix(start.rect.width, end.rect.width),
                            height: mix(start.rect.height, end.rect.height),
                        },
                        opacity: mix(start.opacity, end.opacity),
                        corner_radius: mix(start.corner_radius, end.corner_radius),
                    }
                })
                .collect(),
            overview: mix(self.overview, target.overview),
            dock: mix(self.dock, target.dock),
            arrange: target.arrange,
            quick_launch: target.quick_launch,
        }
    }
}

#[derive(Debug, Default)]
pub struct Animator {
    from: Presentation,
    to: Presentation,
    started: Duration,
    duration: Duration,
    drag_origin: Option<Presentation>,
    reduced_motion: bool,
}

impl Animator {
    /// Geometry changes invalidate the old animation's coordinate system.
    pub fn reconfigure(&mut self, model: &ShellModel, layout: Layout, now: Duration) {
        let frame = Presentation::settled(model, layout);
        self.from = frame.clone();
        self.to = frame;
        self.started = now.saturating_sub(SETTLE);
        self.duration = SETTLE;
        self.drag_origin = None;
    }

    pub fn set_reduced_motion(&mut self, enabled: bool) {
        self.reduced_motion = enabled;
    }

    pub fn begin_drag(&mut self, now: Duration) {
        self.drag_origin = Some(self.sample(now));
    }

    pub fn end_drag(&mut self) {
        self.drag_origin = None;
    }

    pub fn sample(&self, now: Duration) -> Presentation {
        let progress = if self.reduced_motion {
            1.0
        } else {
            (now.saturating_sub(self.started).as_secs_f32()
                / self.duration.max(Duration::from_millis(1)).as_secs_f32())
            .min(1.0)
        };
        if progress >= 1.0 {
            return self.to.clone();
        }
        // Cubic ease-out, evaluated from elapsed time rather than per-frame increments.
        self.from.blend(&self.to, 1.0 - (1.0 - progress).powi(3))
    }

    pub fn advance(
        &mut self,
        model: &ShellModel,
        layout: Layout,
        preview: Option<Preview>,
        now: Duration,
    ) -> (Presentation, bool) {
        if let Some(preview) = preview {
            let origin = self.drag_origin.clone().unwrap_or_else(|| self.sample(now));
            let mut frame = origin.clone();
            match preview {
                Preview::QuickLaunch(wave) => {
                    frame.quick_launch = Some(wave);
                    frame.dock = 0.0;
                }
                Preview::Overview { progress } => {
                    frame = origin.blend(
                        &Presentation::for_view(model, layout, View::Overview),
                        progress,
                    );
                }
                Preview::Browse { offset } => {
                    for window in &mut frame.windows {
                        window.rect = window.rect.translated(offset, 0.0);
                    }
                }
                Preview::Dismiss { id, offset } => {
                    if let Some(window) = frame.windows.iter_mut().find(|window| window.id == id) {
                        window.rect = window.rect.translated(0.0, offset);
                    }
                }
                Preview::Arrange {
                    id,
                    delta,
                    deck_offset,
                    target,
                } => {
                    for window in &mut frame.windows {
                        if window.id == id {
                            window.rect = window.rect.translated(delta.0, delta.1);
                            window.opacity = 0.72;
                        } else {
                            window.rect = window.rect.translated(deck_offset, 0.0);
                        }
                    }
                    frame.windows.sort_by_key(|window| window.id == id);
                    frame.arrange = Some(ArrangeVisual { id, target });
                    frame.dock = 0.0;
                }
            }
            self.from = frame.clone();
            self.to = frame.clone();
            self.started = now;
            return (frame, false);
        }
        let target = Presentation::settled(model, layout);
        if self.to != target {
            self.from = self.sample(now);
            self.duration =
                Duration::from_millis(if target.windows.len() > self.to.windows.len() {
                    crate::design::LAUNCH_MS
                } else if target.windows.len() < self.to.windows.len() {
                    crate::design::DISMISS_MS
                } else if matches!(model.view, View::App(_)) {
                    crate::design::MAXIMIZE_MS
                } else if model.view == View::Overview {
                    crate::design::MINIMIZE_MS
                } else {
                    crate::design::LAUNCHER_MS
                });
            self.to = target;
            self.started = now;
        }
        let moving = !self.reduced_motion
            && self.from != self.to
            && now.saturating_sub(self.started) < self.duration;
        (self.sample(now), moving)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Command;

    #[test]
    fn a_rounded_corner_selects_the_visible_card_behind_it() {
        let back = WindowVisual {
            id: WindowId(1),
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 200.0,
                height: 200.0,
            },
            opacity: 1.0,
            corner_radius: 0.0,
        };
        let front = WindowVisual {
            id: WindowId(2),
            rect: Rect {
                x: 10.0,
                y: 10.0,
                width: 100.0,
                height: 100.0,
            },
            opacity: 1.0,
            corner_radius: 11.0,
        };
        let frame = Presentation {
            windows: vec![back, front],
            ..Default::default()
        };
        assert_eq!(frame.card_at(11.0, 11.0, None), Some(back.id));
        assert_eq!(frame.card_at(21.0, 11.0, None), Some(front.id));
        assert_eq!(frame.card_at(60.0, 60.0, None), Some(front.id));
        assert_eq!(frame.card_at(210.0, 210.0, None), None);
    }

    #[test]
    fn quick_launch_preview_preserves_native_geometry_and_cancels_without_navigation() {
        let (model, layout, mut animation) = fixture();
        let (initial, _) = animation.advance(&model, layout, None, SETTLE);
        animation.begin_drag(SETTLE);
        let wave = crate::quick_launch::wave(layout, (42.0, 610.0));
        let (shown, moving) =
            animation.advance(&model, layout, Some(Preview::QuickLaunch(wave)), SETTLE);
        assert_eq!(shown.windows, initial.windows);
        assert!(!moving && shown.quick_launch == Some(wave));
        animation.end_drag();
        let (returned, _) = animation.advance(&model, layout, None, SETTLE * 2);
        assert!(returned.quick_launch.is_none());
        assert_eq!(returned.windows, initial.windows);
    }

    fn fixture() -> (ShellModel, Layout, Animator) {
        let mut model = ShellModel::default();
        model.add_window(WindowId(1), "One".into(), "one".into());
        model.add_window(WindowId(2), "Two".into(), "two".into());
        let layout = Layout {
            width: 360.0,
            height: 720.0,
            keyboard_height: 0.0,
        };
        let frame = Presentation::settled(&model, layout);
        let animation = Animator {
            from: frame.clone(),
            to: frame,
            ..Default::default()
        };
        (model, layout, animation)
    }

    #[test]
    fn arranged_card_follows_the_finger_above_its_stack_and_cancel_settles_back() {
        let (mut model, layout, mut animation) = fixture();
        model.apply(Command::Overview);
        model.apply(Command::Stack {
            card: WindowId(1),
            with: WindowId(2),
        });
        animation.advance(&model, layout, None, Duration::ZERO);
        animation.advance(&model, layout, None, SETTLE);
        let original = Presentation::settled(&model, layout);
        animation.begin_drag(SETTLE);
        let (dragged, _) = animation.advance(
            &model,
            layout,
            Some(Preview::Arrange {
                id: WindowId(2),
                delta: (60.0, 40.0),
                deck_offset: -48.0,
                target: None,
            }),
            SETTLE,
        );
        assert_eq!(dragged.windows.last().unwrap().id, WindowId(2));
        let rect = original
            .window(WindowId(2))
            .unwrap()
            .rect
            .translated(60.0, 40.0);
        assert_eq!(dragged.window(WindowId(2)).unwrap().rect, rect);
        assert_eq!(
            dragged.card_at(rect.x + 10.0, rect.y + 10.0, model.selected),
            Some(WindowId(2))
        );
        animation.end_drag();
        assert_eq!(
            animation
                .advance(&model, layout, None, SETTLE)
                .0
                .window(WindowId(2))
                .unwrap()
                .rect,
            rect
        );
        assert_eq!(
            animation.advance(&model, layout, None, SETTLE + SETTLE).0,
            original
        );
    }

    #[test]
    fn interrupted_transition_starts_at_the_displayed_position() {
        let (mut model, layout, mut animation) = fixture();
        model.apply(Command::Overview);
        let (first, running) = animation.advance(&model, layout, None, Duration::ZERO);
        assert!(running);
        assert_eq!(first.window(WindowId(2)).unwrap().rect, layout.content());
        let time = Duration::from_millis(90);
        let midway = animation.sample(time);
        assert_ne!(midway.window(WindowId(2)).unwrap().rect, layout.content());
        model.apply(Command::Activate(WindowId(2)));
        let (reversed, _) = animation.advance(&model, layout, None, time);
        assert_eq!(reversed, midway);
        let (end, running) = animation.advance(&model, layout, None, time + SETTLE);
        assert_eq!(end, Presentation::settled(&model, layout));
        assert!(!running);
    }

    #[test]
    fn drag_cancel_and_selection_settle_from_the_finger_position() {
        let (mut model, layout, mut animation) = fixture();
        model.apply(Command::Overview);
        animation.advance(&model, layout, None, Duration::ZERO);
        let time = SETTLE;
        animation.advance(&model, layout, None, time);
        animation.begin_drag(time);
        let (dragged, _) = animation.advance(
            &model,
            layout,
            Some(Preview::Browse { offset: 120.0 }),
            time,
        );
        animation.end_drag();
        let (released, moving) = animation.advance(&model, layout, None, time);
        assert_eq!(dragged, released);
        assert!(moving);
        let (end, moving) = animation.advance(&model, layout, None, time + SETTLE);
        assert_eq!(end, Presentation::settled(&model, layout));
        assert!(!moving);
    }

    #[test]
    fn frame_rate_does_not_change_motion_and_reduced_motion_finishes_immediately() {
        let (mut model, layout, mut dense) = fixture();
        let (_, _, mut sparse) = fixture();
        model.apply(Command::Overview);
        dense.advance(&model, layout, None, Duration::ZERO);
        sparse.advance(&model, layout, None, Duration::ZERO);
        for time in 1..110 {
            dense.advance(&model, layout, None, Duration::from_millis(time));
        }
        assert_eq!(
            dense.sample(Duration::from_millis(110)),
            sparse.sample(Duration::from_millis(110))
        );
        dense.set_reduced_motion(true);
        let (end, moving) = dense.advance(&model, layout, None, Duration::from_millis(110));
        assert_eq!(end, Presentation::settled(&model, layout));
        assert!(!moving);
    }

    #[test]
    fn hit_testing_uses_the_displayed_card_during_a_drag() {
        let (mut model, layout, mut animation) = fixture();
        model.apply(Command::Overview);
        animation.advance(&model, layout, None, Duration::ZERO);
        animation.advance(&model, layout, None, SETTLE);
        animation.begin_drag(SETTLE);
        let (frame, _) = animation.advance(
            &model,
            layout,
            Some(Preview::Dismiss {
                id: WindowId(2),
                offset: -100.0,
            }),
            SETTLE,
        );
        let rect = frame.window(WindowId(2)).unwrap().rect;
        assert_eq!(
            frame.card_at(rect.x + 20.0, rect.y + 20.0, model.selected),
            Some(WindowId(2))
        );
        assert_eq!(
            frame.card_at(rect.x + 20.0, rect.y + rect.height + 10.0, model.selected),
            None
        );
    }
}
