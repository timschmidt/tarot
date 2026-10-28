//! Compositor-owned protected recovery. Takes no shell model or private content.
use egui::{Align2, Color32, FontId, Rect, Sense, Vec2};

pub fn retry_rect(width: f32, height: f32) -> Rect {
    Rect::from_center_size(
        egui::pos2(width * 0.5, height * 0.5 + 28.0),
        Vec2::new((width - 40.0).clamp(120.0, 260.0), 52.0),
    )
}

/// Only requests another trusted client. This view cannot authenticate/unlock.
pub fn show(ctx: &egui::Context, width: f32, height: f32) -> bool {
    let mut retry = false;
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(Color32::from_rgb(20, 29, 34)))
        .show(ctx, |ui| {
            let center = egui::pos2(width * 0.5, height * 0.5);
            let painter = ui.painter().clone();
            painter.text(
                center - egui::vec2(0.0, 104.0),
                Align2::CENTER_CENTER,
                "Your session is locked",
                FontId::proportional(23.0),
                Color32::WHITE,
            );
            painter.text(
                center - egui::vec2(0.0, 62.0),
                Align2::CENTER_CENTER,
                "Unlock screen unavailable",
                FontId::proportional(16.0),
                Color32::from_gray(205),
            );
            let response = ui.put(
                retry_rect(width, height),
                egui::Button::new("Try again").sense(Sense::click()),
            );
            retry = response.clicked();
            painter.text(
                center + egui::vec2(0.0, 92.0),
                Align2::CENTER_CENTER,
                "Or press Enter to try again",
                FontId::proportional(14.0),
                Color32::from_gray(205),
            );
        });
    retry
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canceled_contact_cannot_retry_and_a_fresh_click_can() {
        let ctx = egui::Context::default();
        let pos = retry_rect(360.0, 720.0).center();
        let frame = |events: Vec<egui::Event>| {
            crate::prepare_pointer_input(&ctx, &events);
            let mut clicked = false;
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(360.0, 720.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    clicked = show(ctx, 360.0, 720.0);
                },
            );
            clicked
        };
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![]);
        frame(vec![]);
        assert!(!frame(vec![egui::Event::PointerMoved(pos), button(true)]));
        let mut cancel = Vec::new();
        crate::cancel_pointer_input(&mut cancel);
        assert!(!frame(cancel));
        assert!(!frame(vec![button(false)]));
        assert!(!frame(vec![egui::Event::PointerMoved(pos), button(true)]));
        assert!(frame(vec![button(false)]));
    }
}
