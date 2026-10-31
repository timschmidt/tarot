//! The packet's shared controls (components/control-states): the ON/OFF switch.
use egui::{Align2, Color32, CornerRadius, Response, Sense, Stroke, Ui, Vec2, Widget, WidgetText};

const TRACK_OFF: Color32 = Color32::from_rgb(170, 179, 159);
const TRACK_ON: Color32 = Color32::from_rgb(31, 122, 179);
const KNOB: Color32 = Color32::from_rgb(242, 243, 237);
const MARK: Color32 = Color32::from_rgb(243, 245, 233);
const SIZE: Vec2 = Vec2::new(54.0, 28.0);

/// A labelled ON/OFF switch; a drop-in for `egui::Checkbox`.
pub struct Toggle<'a> {
    value: &'a mut bool,
    text: WidgetText,
}

impl<'a> Toggle<'a> {
    pub fn new(value: &'a mut bool, text: impl Into<WidgetText>) -> Self {
        Self {
            value,
            text: text.into(),
        }
    }
}

impl Widget for Toggle<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let height = ui.spacing().interact_size.y.max(48.0);
        let width = ui.available_width().max(SIZE.x + 8.0);
        let label_width = (width - SIZE.x - 16.0).max(1.0);
        let galley = self.text.into_galley(
            ui,
            Some(egui::TextWrapMode::Wrap),
            label_width,
            egui::TextStyle::Body,
        );
        let (rect, mut response) = ui.allocate_exact_size(
            Vec2::new(width, height.max(galley.size().y + 12.0)),
            Sense::click(),
        );
        if response.clicked() {
            *self.value = !*self.value;
            response.mark_changed();
        }
        let on = *self.value;
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                ui.is_enabled(),
                on,
                galley.text(),
            )
        });
        if ui.is_rect_visible(rect) {
            let enabled = ui.is_enabled();
            let painter = ui.painter();
            let text_color = ui.visuals().widgets.noninteractive.fg_stroke.color;
            painter.galley(
                egui::pos2(rect.left() + 4.0, rect.center().y - galley.size().y / 2.0),
                galley,
                text_color,
            );
            let track = egui::Rect::from_center_size(
                egui::pos2(rect.right() - SIZE.x / 2.0 - 4.0, rect.center().y),
                SIZE,
            );
            let fill = if on && enabled { TRACK_ON } else { TRACK_OFF };
            let alpha = if enabled { 1.0 } else { 0.55 };
            painter.rect(
                track,
                CornerRadius::same(14),
                fill.gamma_multiply(alpha),
                Stroke::new(1.0_f32, Color32::from_black_alpha(40)),
                egui::StrokeKind::Inside,
            );
            let knob_x = if on {
                track.right() - SIZE.y / 2.0
            } else {
                track.left() + SIZE.y / 2.0
            };
            painter.circle(
                egui::pos2(knob_x, track.center().y),
                SIZE.y / 2.0 - 2.0,
                KNOB.gamma_multiply(alpha),
                Stroke::new(1.0_f32, Color32::from_black_alpha(50)),
            );
            let mark_x = if on {
                track.left() + 14.0
            } else {
                track.right() - 15.0
            };
            painter.text(
                egui::pos2(mark_x, track.center().y),
                Align2::CENTER_CENTER,
                if on { "ON" } else { "OFF" },
                crate::theme::bold(ui.ctx(), 10.0),
                MARK.gamma_multiply(alpha),
            );
            if response.has_focus() {
                painter.rect_stroke(
                    track.expand(2.0),
                    CornerRadius::same(16),
                    Stroke::new(2.0_f32, crate::theme::FOCUS),
                    egui::StrokeKind::Outside,
                );
            }
        }
        response
    }
}

/// `ui.switch(&mut value, text)` in place of `ui.checkbox(...)`.
pub trait Switch {
    fn switch(&mut self, value: &mut bool, text: impl Into<WidgetText>) -> Response;
}

impl Switch for Ui {
    fn switch(&mut self, value: &mut bool, text: impl Into<WidgetText>) -> Response {
        self.add(Toggle::new(value, text))
    }
}
