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
    /// Clockwise degrees about the card center in output coordinates.
    pub rotation: f32,
}

impl WindowVisual {
    pub fn to_output(self, x: f32, y: f32) -> (f32, f32) {
        self.rotate(x, y, self.rotation)
    }
    pub fn from_output(self, x: f32, y: f32) -> (f32, f32) {
        self.rotate(x, y, -self.rotation)
    }
    fn rotate(self, x: f32, y: f32, degrees: f32) -> (f32, f32) {
        let cx = self.rect.x + self.rect.width * 0.5;
        let cy = self.rect.y + self.rect.height * 0.5;
        let (sin, cos) = degrees.to_radians().sin_cos();
        (
            cx + cos * (x - cx) - sin * (y - cy),
            cy + sin * (x - cx) + cos * (y - cy),
        )
    }
    pub fn bounds(self) -> Rect {
        let cx = self.rect.x + self.rect.width * 0.5;
        let cy = self.rect.y + self.rect.height * 0.5;
        let (sin, cos) = self.rotation.to_radians().sin_cos();
        let width = cos.abs() * self.rect.width + sin.abs() * self.rect.height;
        let height = sin.abs() * self.rect.width + cos.abs() * self.rect.height;
        Rect {
            x: cx - width * 0.5,
            y: cy - height * 0.5,
            width,
            height,
        }
    }
    pub fn contains(self, x: f32, y: f32) -> bool {
        let (x, y) = self.from_output(x, y);
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

// Preserve the packet's relative fan poses while fitting the entire group
// above the tray in landscape/small outputs. All members share one fit.
fn preview_pose(
    layout: Layout,
    offset: f32,
    pose: Option<(f32, f32, f32, f32)>,
    inactive: bool,
) -> (Rect, f32) {
    let base = layout.card(0, 0);
    let raw = |pose: Option<(f32, f32, f32, f32)>| {
        let mut rect = base.translated(offset, 0.0);
        let mut rotation = 0.0;
        if let Some((x, y, scale, degrees)) = pose {
            let factor = base.width / (360.0 * crate::design::CARD_SCALE);
            rect.x += (x - 61.2) * factor;
            rect.y += (y - crate::design::CARD_Y) * factor;
            rect.width *= scale / crate::design::CARD_SCALE;
            rect.height *= scale / crate::design::CARD_SCALE;
            rotation = degrees;
        }
        if inactive {
            let factor = crate::design::INACTIVE_CARD_SCALE / crate::design::CARD_SCALE;
            rect.x += rect.width * (1.0 - factor) * 0.5;
            rect.width *= factor;
            rect.height *= factor;
            rect.y += 21.0 * base.width / (360.0 * crate::design::CARD_SCALE);
        }
        WindowVisual {
            id: WindowId(0),
            rect,
            rotation,
            opacity: 1.0,
            corner_radius: 0.0,
        }
    };
    let previews = if pose.is_some() {
        crate::design::STACK_POSES
            .map(|p| raw(Some(p)).bounds())
            .to_vec()
    } else {
        vec![raw(None).bounds()]
    };
    let top = previews.iter().map(|r| r.y).fold(f32::INFINITY, f32::min);
    let bottom = previews
        .iter()
        .map(|r| r.y + r.height)
        .fold(f32::NEG_INFINITY, f32::max);
    let minimum = Layout::STATUS + 52.0;
    let maximum = (layout.tray_bottom() - Layout::DOCK - 36.0).max(minimum + 1.0);
    let factor = ((maximum - minimum) / (bottom - top)).min(1.0);
    let placed_top = top.clamp(minimum, (maximum - (bottom - top) * factor).max(minimum));
    let mut visual = raw(pose);
    let center = base.x + base.width * 0.5 + offset;
    visual.rect.x = center + (visual.rect.x - center) * factor;
    visual.rect.y = placed_top + (visual.rect.y - top) * factor;
    visual.rect.width *= factor;
    visual.rect.height *= factor;
    (visual.rect, visual.rotation)
}

impl Presentation {
    pub fn settled(model: &ShellModel, layout: Layout) -> Self {
        Self::for_view(model, layout, model.view)
    }

    fn for_view(model: &ShellModel, layout: Layout, view: View) -> Self {
        let selected_stack = model
            .cards
            .iter()
            .find(|c| Some(c.id) == model.selected)
            .map(|c| c.stack);
        let mut groups: Vec<_> = model.cards.iter().map(|c| c.stack).collect();
        groups.dedup();
        let selected_group = groups
            .iter()
            .position(|stack| Some(*stack) == selected_stack)
            .unwrap_or(0);
        let mut frame = Self {
            windows: model
                .cards
                .iter()
                .map(|card| {
                    let group = groups
                        .iter()
                        .position(|stack| *stack == card.stack)
                        .unwrap();
                    // A folded group occupies one deck position regardless of
                    // its member count. Member browse distances are separate.
                    let group_offset =
                        (group as f32 - selected_group as f32) * (layout.card(0, 0).width + 18.0);
                    let mut pose = None;
                    let mut preview_visible = true;
                    let members = model.stack_members(card.id);
                    let active_group = Some(card.stack) == selected_stack;
                    if members.len() > 1 {
                        let mut order = members;
                        if let Some(selected) = model.selected.filter(|id| order.contains(id)) {
                            order.retain(|id| *id != selected);
                            order.push(selected);
                        }
                        let rank = order.iter().position(|id| *id == card.id).unwrap();
                        preview_visible = rank >= order.len().saturating_sub(3);
                        let slot = if order.len() == 2 {
                            rank * 2
                        } else {
                            rank.saturating_sub(order.len().saturating_sub(3))
                        };
                        pose = Some(crate::design::STACK_POSES[slot]);
                    }
                    let (mut rect, mut rotation) =
                        preview_pose(layout, group_offset, pose, !active_group);
                    if view == View::App(card.id) {
                        rect = layout.content();
                        rotation = 0.0;
                    }
                    WindowVisual {
                        id: card.id,
                        rect,
                        rotation,
                        opacity: if (view == View::Overview && preview_visible)
                            || view == View::App(card.id)
                        {
                            1.0
                        } else {
                            0.0
                        },
                        corner_radius: if view == View::App(card.id) {
                            0.0
                        } else {
                            (crate::design::CARD_RADIUS * rect.width
                                / layout.content().width.max(1.0))
                            .min(rect.width.min(rect.height) * 0.5)
                        },
                    }
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
        self.window(id).is_some_and(|window| {
            window.opacity > 0.01 && window.bounds().intersects(layout.content())
        })
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
                        rotation: mix(start.rotation, end.rotation),
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
        self.advance_with_duration(model, layout, preview, now, None)
    }

    pub fn advance_with_duration(
        &mut self,
        model: &ShellModel,
        layout: Layout,
        preview: Option<Preview>,
        now: Duration,
        duration: Option<Duration>,
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
            self.duration = duration.unwrap_or_else(|| {
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
                })
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
    fn adjacent_group_previews_remain_visible_after_a_three_or_five_member_stack() {
        for count in [3, 5] {
            let (_, layout, _) = fixture();
            let mut model = ShellModel::default();
            for id in 1..=count {
                model.add_window(WindowId(id), id.to_string(), "same.app".into());
            }
            model.apply(Command::Overview);
            for id in 1..count {
                model.apply(Command::Stack {
                    card: WindowId(id),
                    with: WindowId(count),
                });
            }
            let sibling = model.selected.unwrap();
            model.add_window(WindowId(count + 1), "Neighbor".into(), "other.app".into());
            model.apply(Command::Overview);
            let frame = Presentation::settled(&model, layout);
            assert!(
                model
                    .cards
                    .iter()
                    .any(|c| c.id != WindowId(count + 1) && frame.visible(c.id, layout)),
                "Previous group must remain visible beside the selected standalone card"
            );
            model.apply(Command::Activate(sibling));
            model.apply(Command::Overview);
            let frame = Presentation::settled(&model, layout);
            assert!(
                frame.visible(WindowId(count + 1), layout),
                "Next group must remain visible beside the selected fan"
            );
        }
    }

    #[test]
    fn three_member_fan_uses_packet_poses_and_active_app_returns_to_zero_rotation() {
        let (mut model, layout, _) = fixture();
        model.add_window(WindowId(3), "Three".into(), "three".into());
        model.apply(Command::Overview);
        model.apply(Command::Stack {
            card: WindowId(1),
            with: WindowId(3),
        });
        model.apply(Command::Stack {
            card: WindowId(2),
            with: WindowId(3),
        });
        let frame = Presentation::settled(&model, layout);
        assert_eq!(frame.windows.len(), 3);
        for (visual, pose) in frame.windows.iter().zip(crate::design::STACK_POSES) {
            assert!((visual.rect.x - pose.0).abs() < 0.001);
            assert!((visual.rect.y - pose.1).abs() < 0.001);
            assert!((visual.rect.width - 360.0 * pose.2).abs() < 0.001);
            assert_eq!(visual.rotation, pose.3);
        }
        let selected = model.selected.unwrap();
        model.apply(Command::Activate(selected));
        let active = Presentation::settled(&model, layout);
        assert_eq!(active.window(selected).unwrap().rotation, 0.0);
        assert_eq!(active.window(selected).unwrap().rect, layout.content());
    }

    #[test]
    fn large_stacks_keep_every_card_but_only_three_live_previews_and_browse_brings_each_forward() {
        let (mut model, layout, _) = fixture();
        for id in 3..=5 {
            model.add_window(WindowId(id), id.to_string(), "same.app".into());
        }
        model.apply(Command::Overview);
        for id in 1..5 {
            model.apply(Command::Stack {
                card: WindowId(id),
                with: WindowId(5),
            });
        }
        model.apply(Command::BrowseCards(i32::MAX));
        let mut visited = std::collections::HashSet::new();
        for _ in 0..10 {
            visited.insert(model.selected.unwrap());
            let frame = Presentation::settled(&model, layout);
            assert_eq!(frame.windows.len(), 5);
            assert_eq!(frame.windows.iter().filter(|w| w.opacity > 0.0).count(), 3);
            assert_eq!(frame.windows.last().unwrap().id, model.selected.unwrap());
            assert_eq!(frame.windows.last().unwrap().opacity, 1.0);
            model.apply(Command::BrowseCards(-1));
        }
        assert_eq!(visited.len(), 5);
    }

    #[test]
    fn landscape_fans_reserve_controls_caption_and_tray_without_distorting_members() {
        for (width, height) in [(360.0, 720.0), (720.0, 360.0), (320.0, 240.0)] {
            let layout = Layout {
                width,
                height,
                notification_height: 0.0,
                keyboard_height: 0.0,
            };
            let ratios = crate::design::STACK_POSES.map(|pose| {
                let (rect, rotation) = preview_pose(layout, 0.0, Some(pose), false);
                let bounds = WindowVisual {
                    id: WindowId(0),
                    rect,
                    rotation,
                    opacity: 1.0,
                    corner_radius: 0.0,
                }
                .bounds();
                assert!(bounds.y >= Layout::STATUS + 52.0 - 0.001);
                assert!(
                    bounds.y + bounds.height <= layout.gesture().y - Layout::DOCK - 36.0 + 0.001
                );
                rect.width / (width * pose.2)
            });
            assert!((ratios[0] - ratios[1]).abs() < 0.001 && (ratios[1] - ratios[2]).abs() < 0.001);
        }
    }

    #[test]
    fn rotated_shape_uses_inverse_pose_and_expanded_visibility_bounds() {
        let card = WindowVisual {
            id: WindowId(1),
            rect: Rect {
                x: 60.0,
                y: 139.0,
                width: 230.4,
                height: 417.28,
            },
            opacity: 1.0,
            corner_radius: 7.04,
            rotation: 5.0,
        };
        let inside = card.to_output(card.rect.x + 10.0, card.rect.y + 10.0);
        assert!(card.contains(inside.0, inside.1));
        let corner = card.to_output(card.rect.x + 1.0, card.rect.y + 1.0);
        assert!(!card.contains(corner.0, corner.1));
        let edge = card.to_output(
            card.rect.x + card.rect.width - 1.0,
            card.rect.y + card.rect.height * 0.5,
        );
        let roundtrip = card.from_output(edge.0, edge.1);
        assert!((roundtrip.0 - card.rect.x - card.rect.width + 1.0).abs() < 0.001);
        assert!(card.bounds().width > card.rect.width && card.bounds().height > card.rect.height);
        assert!(card.bounds().contains(edge.0, edge.1));
    }

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
            rotation: 0.0,
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
            rotation: 0.0,
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
            notification_height: 0.0,
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
