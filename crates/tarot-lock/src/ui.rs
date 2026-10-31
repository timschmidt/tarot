//! Lock presentation never hands the credential to egui or the clipboard.
use egui::{Align2, Color32, FontId, Rect, Sense, Stroke, Vec2};
use tarot_ui::keyboard::{Action, KeyboardUi};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ready,
    Checking,
    Rejected,
    Unavailable,
}
#[derive(Default)]
pub struct View {
    pub editing: bool,
    pub keyboard: bool,
    keys: KeyboardUi,
    /// Upward travel of the current swipe. egui reports drag movement only
    /// per frame, and not at all on the release frame, so it is kept here.
    swipe: Option<f32>,
}

/// Upward travel that opens password entry, like a tap on the lock.
const SWIPE_TRAVEL: f32 = 48.0;
pub struct Frame<'a> {
    pub count: usize,
    pub status: Status,
    pub retry_seconds: u64,
    pub acknowledged: bool,
    pub clock: &'a str,
    pub date: &'a str,
}
impl View {
    pub fn reset(&mut self) {
        self.editing = false;
        self.keyboard = false;
        self.swipe = None;
        self.keys.reset();
    }
    pub fn show(&mut self, ctx: &egui::Context, frame: Frame<'_>) -> Vec<Action> {
        tarot_ui::theme::install(ctx);
        let mut actions = Vec::new();
        let ready =
            frame.acknowledged && frame.status != Status::Checking && frame.retry_seconds == 0;
        if self.editing && self.keyboard {
            egui::TopBottomPanel::bottom("private_lock_keyboard")
                .exact_height(240.0_f32.min(ctx.content_rect().height() * 0.5))
                .frame(
                    egui::Frame::new()
                        .fill(tarot_ui::keyboard::PANEL)
                        .inner_margin(5.0),
                )
                .show(ctx, |ui| {
                    ui.add_enabled_ui(ready, |ui| {
                        actions = self.keys.show_in_ui(ui, false, true);
                    });
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(Color32::from_rgb(20, 29, 34)))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter().clone();
                let center = rect.center().x;
                let compact = self.editing && rect.height() < 430.0;
                let compact_clock = rect.height() >= 156.0;
                if !compact || compact_clock {
                    painter.text(
                        egui::pos2(center, rect.top() + if compact { 20.0 } else { 100.0 }),
                        Align2::CENTER_CENTER,
                        frame.clock,
                        FontId::proportional(if compact { 22.0 } else { 58.0 }),
                        Color32::WHITE,
                    );
                }
                if !compact {
                    painter.text(
                        egui::pos2(center, rect.top() + 148.0),
                        Align2::CENTER_CENTER,
                        frame.date,
                        FontId::proportional(17.0),
                        Color32::from_gray(195),
                    );
                }
                if self.editing {
                    let field = Rect::from_center_size(
                        egui::pos2(
                            center,
                            rect.top()
                                + if compact {
                                    if compact_clock { 62.0 } else { 22.0 }
                                } else {
                                    222.0
                                },
                        ),
                        Vec2::new(
                            (rect.width() - 40.0).min(360.0),
                            if compact { 38.0 } else { 46.0 },
                        ),
                    );
                    let response = ui.allocate_rect(field, Sense::click());
                    painter.rect_filled(field, 12.0, Color32::from_rgb(45, 56, 62));
                    painter.rect_stroke(
                        field,
                        12.0,
                        Stroke::new(1.0_f32, Color32::from_rgb(132, 166, 177)),
                        egui::StrokeKind::Inside,
                    );
                    let bullets = "•".repeat(frame.count.min(24));
                    painter.text(
                        field.center(),
                        Align2::CENTER_CENTER,
                        if bullets.is_empty() {
                            "Device password"
                        } else {
                            &bullets
                        },
                        FontId::proportional(20.0),
                        Color32::WHITE,
                    );
                    if response.clicked() && ready {
                        self.keyboard = true;
                    }
                    let message = if !frame.acknowledged {
                        "Securing display…".to_owned()
                    } else if frame.retry_seconds > 0 {
                        format!("Try again in {}s", frame.retry_seconds)
                    } else {
                        match frame.status {
                            Status::Ready => "Enter your device password".into(),
                            Status::Checking => "Checking…".into(),
                            Status::Rejected => "Password not accepted. Try again.".into(),
                            Status::Unavailable => "Authentication unavailable. Try again.".into(),
                        }
                    };
                    painter.text(
                        egui::pos2(
                            center,
                            rect.top()
                                + if compact {
                                    if compact_clock { 99.0 } else { 52.0 }
                                } else {
                                    268.0
                                },
                        ),
                        Align2::CENTER_CENTER,
                        message,
                        FontId::proportional(if compact { 12.0 } else { 14.0 }),
                        Color32::from_gray(210),
                    );
                    let unlock = Rect::from_center_size(
                        if compact {
                            egui::pos2(center - 88.0, rect.bottom() - 24.0)
                        } else {
                            egui::pos2(center, rect.top() + 322.0)
                        },
                        if compact {
                            Vec2::new(150.0, 38.0)
                        } else {
                            Vec2::new(180.0, 44.0)
                        },
                    );
                    if ui.put(unlock, egui::Button::new("Unlock")).clicked()
                        && ready
                        && frame.count > 0
                    {
                        actions.push(Action::Enter);
                    }
                    let cancel = Rect::from_center_size(
                        if compact {
                            egui::pos2(center + 88.0, rect.bottom() - 24.0)
                        } else {
                            egui::pos2(center, rect.top() + 377.0)
                        },
                        Vec2::new(if compact { 120.0 } else { 140.0 }, 38.0),
                    );
                    if ui.put(cancel, egui::Button::new("Cancel")).clicked()
                        && frame.status != Status::Checking
                    {
                        actions.push(Action::Hide);
                    }
                } else {
                    // A swipe up may start anywhere on the lock screen; the
                    // lock itself also takes a tap. Both only open entry.
                    let screen = ui.interact(rect, ui.id().with("swipe"), Sense::drag());
                    let puck = Rect::from_center_size(
                        egui::pos2(center, rect.bottom() - 112.0),
                        Vec2::splat(78.0),
                    );
                    let response = ui.allocate_rect(puck, Sense::click_and_drag());
                    let mut swiped = false;
                    for drag in [&screen, &response] {
                        if drag.dragged()
                            && let Some(delta) = drag.total_drag_delta()
                        {
                            self.swipe = Some(-delta.y);
                        }
                        if drag.drag_stopped() {
                            swiped |= self.swipe.take().is_some_and(|up| up >= SWIPE_TRAVEL);
                        }
                    }
                    painter.circle_filled(puck.center(), 39.0, Color32::from_rgb(63, 76, 82));
                    let body = Rect::from_center_size(
                        puck.center() + egui::vec2(0.0, 7.0),
                        egui::vec2(24.0, 20.0),
                    );
                    painter.rect_filled(body, 3.0, Color32::from_gray(225));
                    painter.rect_stroke(
                        Rect::from_center_size(
                            puck.center() - egui::vec2(0.0, 8.0),
                            egui::vec2(18.0, 24.0),
                        ),
                        8.0,
                        Stroke::new(3.0_f32, Color32::from_gray(225)),
                        egui::StrokeKind::Inside,
                    );
                    painter.text(
                        egui::pos2(center, rect.bottom() - 46.0),
                        Align2::CENTER_CENTER,
                        "Swipe up or tap to unlock",
                        FontId::proportional(15.0),
                        Color32::from_gray(220),
                    );
                    if ready && (response.clicked() || swiped) {
                        self.editing = true;
                        self.keyboard = true;
                    }
                }
            });
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_real_unlock_click_requires_presentation_and_no_pending_or_retry() {
        for (width, height, keyboard, position) in [
            (360.0, 720.0, false, egui::pos2(180.0, 330.0)),
            (720.0, 360.0, true, egui::pos2(272.0, 148.0)),
            (360.0, 240.0, true, egui::pos2(92.0, 88.0)),
        ] {
            for (status, retry, ack, expected) in [
                (Status::Ready, 0, true, true),
                (Status::Checking, 0, true, false),
                (Status::Ready, 2, true, false),
                (Status::Ready, 0, false, false),
            ] {
                let ctx = egui::Context::default();
                let mut view = View {
                    editing: true,
                    keyboard,
                    ..Default::default()
                };
                let mut actions = Vec::new();
                for events in [
                    vec![],
                    vec![
                        egui::Event::PointerMoved(position),
                        egui::Event::PointerButton {
                            pos: position,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                    vec![egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                ] {
                    let input = egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, height),
                        )),
                        events,
                        ..Default::default()
                    };
                    let _ = ctx.run(input, |ctx| {
                        actions = view.show(
                            ctx,
                            Frame {
                                count: 6,
                                status,
                                retry_seconds: retry,
                                acknowledged: ack,
                                clock: "12:34",
                                date: "Tuesday",
                            },
                        );
                    });
                }
                assert_eq!(
                    actions.iter().any(|a| matches!(a, Action::Enter)),
                    expected,
                    "{width}x{height}"
                );
            }
        }
    }

    fn pointer(ctx: &egui::Context, view: &mut View, events: Vec<egui::Event>, ready: bool) {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(360.0, 720.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            view.show(
                ctx,
                Frame {
                    count: 0,
                    status: Status::Ready,
                    retry_seconds: 0,
                    acknowledged: ready,
                    clock: "12:34",
                    date: "Tuesday",
                },
            );
        });
    }

    fn swipe(from: egui::Pos2, dy: f32, ready: bool) -> bool {
        let ctx = egui::Context::default();
        let mut view = View::default();
        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        pointer(&ctx, &mut view, vec![], ready);
        pointer(
            &ctx,
            &mut view,
            vec![egui::Event::PointerMoved(from), button(from, true)],
            ready,
        );
        let steps = 6;
        for step in 1..=steps {
            let at = from + egui::vec2(0.0, dy * step as f32 / steps as f32);
            pointer(&ctx, &mut view, vec![egui::Event::PointerMoved(at)], ready);
        }
        let end = from + egui::vec2(0.0, dy);
        pointer(&ctx, &mut view, vec![button(end, false)], ready);
        view.editing
    }

    #[test]
    fn swiping_up_opens_password_entry_like_a_tap() {
        let lock = egui::pos2(180.0, 720.0 - 112.0);
        assert!(swipe(lock, -120.0, true), "swipe up from the lock");
        assert!(
            swipe(egui::pos2(90.0, 500.0), -120.0, true),
            "swipe up elsewhere"
        );
        assert!(!swipe(lock, -20.0, true), "a short drag is not a swipe");
        assert!(!swipe(egui::pos2(90.0, 300.0), 150.0, true), "swiping down");
        assert!(
            !swipe(lock, -120.0, false),
            "not before presentation is acknowledged"
        );
    }
}
