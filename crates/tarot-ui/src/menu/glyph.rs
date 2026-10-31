//! Menu roles use vector strokes so their meaning does not depend on font coverage.
use egui::{Color32, Painter, Pos2, Stroke, Vec2};

#[derive(Clone, Copy)]
pub(crate) enum Glyph {
    Wifi,
    Sound,
    Power,
    Notifications,
    Lock,
    Sun,
    Check,
    Play,
    Pause,
    Previous,
    Next,
    // Packet glyphs (glyphs/*.svg), drawn on their 24-unit grid.
    Back,
    Close,
    Chevron,
    Bluetooth,
    Search,
}

pub(crate) fn paint(p: &Painter, center: Pos2, glyph: Glyph, color: Color32) {
    paint_scaled(p, center, glyph, color, 1.0);
}

/// Paint at `scale` times the menu size (the status bar uses about 0.7).
pub(crate) fn paint_scaled(p: &Painter, center: Pos2, glyph: Glyph, color: Color32, scale: f32) {
    let stroke = Stroke::new(1.8_f32 * scale.max(0.6), color);
    let at = |x: f32, y: f32| center + Vec2::new(x, y) * scale;
    // Packet glyph coordinates are 0..24 with the centre at 12.
    let grid = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points
                .iter()
                .map(|&(x, y)| at((x - 12.0) * 0.85, (y - 12.0) * 0.85))
                .collect(),
            stroke,
        ));
    };
    let line = |points: &[(f32, f32)]| {
        p.add(egui::Shape::line(
            points.iter().map(|&(x, y)| at(x, y)).collect(),
            stroke,
        ));
    };
    let arc = |origin: Pos2, radius: f32, start: f32, end: f32| {
        p.add(egui::Shape::line(
            (0..=16)
                .map(|i| {
                    let angle = start + (end - start) * i as f32 / 16.0;
                    origin + Vec2::new(angle.cos(), angle.sin()) * radius * scale
                })
                .collect(),
            stroke,
        ));
    };
    use std::f32::consts::{FRAC_PI_4, PI};
    match glyph {
        Glyph::Check => line(&[(-5.0, 0.0), (-1.0, 4.0), (6.0, -5.0)]),
        Glyph::Wifi => {
            for radius in [5.0, 10.0, 15.0] {
                arc(at(0.0, 7.0), radius, -3.0 * FRAC_PI_4, -FRAC_PI_4);
            }
            p.circle_filled(at(0.0, 7.0), 1.7 * scale, color);
        }
        Glyph::Sound => {
            line(&[
                (-9.0, -3.0),
                (-5.0, -3.0),
                (0.0, -8.0),
                (0.0, 8.0),
                (-5.0, 3.0),
                (-9.0, 3.0),
                (-9.0, -3.0),
            ]);
            for radius in [6.0, 11.0] {
                arc(at(0.0, 0.0), radius, -FRAC_PI_4, FRAC_PI_4);
            }
        }
        Glyph::Power => {
            arc(center, 9.0, -PI / 3.0, 4.0 * PI / 3.0);
            line(&[(0.0, -11.0), (0.0, 0.0)]);
        }
        Glyph::Notifications => {
            line(&[
                (-9.0, 6.0),
                (-6.0, 2.0),
                (-6.0, -4.0),
                (-4.0, -8.0),
                (0.0, -10.0),
                (4.0, -8.0),
                (6.0, -4.0),
                (6.0, 2.0),
                (9.0, 6.0),
                (-9.0, 6.0),
            ]);
            arc(at(0.0, 7.0), 3.0, 0.0, PI);
        }
        Glyph::Lock => {
            arc(at(0.0, -3.0), 6.0, PI, 2.0 * PI);
            line(&[(-6.0, -3.0), (-6.0, 0.0)]);
            line(&[(6.0, -3.0), (6.0, 0.0)]);
            p.rect_stroke(
                egui::Rect::from_min_max(at(-8.0, 0.0), at(8.0, 10.0)),
                2.0,
                stroke,
                egui::StrokeKind::Inside,
            );
            p.circle_filled(at(0.0, 4.5), 1.4 * scale, color);
        }
        Glyph::Play => line(&[(-5.0, -8.0), (7.0, 0.0), (-5.0, 8.0), (-5.0, -8.0)]),
        Glyph::Pause => {
            line(&[(-4.0, -8.0), (-4.0, 8.0)]);
            line(&[(4.0, -8.0), (4.0, 8.0)]);
        }
        Glyph::Previous => {
            line(&[(-7.0, -8.0), (-7.0, 8.0)]);
            line(&[(7.0, -8.0), (-4.0, 0.0), (7.0, 8.0), (7.0, -8.0)]);
        }
        Glyph::Next => {
            line(&[(7.0, -8.0), (7.0, 8.0)]);
            line(&[(-7.0, -8.0), (4.0, 0.0), (-7.0, 8.0), (-7.0, -8.0)]);
        }
        Glyph::Back => grid(&[(15.0, 5.0), (8.0, 12.0), (15.0, 19.0)]),
        Glyph::Chevron => grid(&[(9.0, 5.0), (16.0, 12.0), (9.0, 19.0)]),
        Glyph::Close => {
            grid(&[(5.0, 5.0), (19.0, 19.0)]);
            grid(&[(19.0, 5.0), (5.0, 19.0)]);
        }
        Glyph::Bluetooth => grid(&[
            (7.0, 6.0),
            (19.0, 18.0),
            (12.0, 23.0),
            (12.0, 1.0),
            (19.0, 6.0),
            (7.0, 18.0),
        ]),
        Glyph::Search => {
            p.circle_stroke(at(-2.0 * 0.85, -2.0 * 0.85), 6.6 * 0.85 * scale, stroke);
            grid(&[(15.0, 15.0), (22.0, 22.0)]);
        }
        Glyph::Sun => {
            p.circle_stroke(center, 5.0 * scale, stroke);
            for i in 0..8 {
                let angle = i as f32 * FRAC_PI_4;
                let direction = Vec2::new(angle.cos(), angle.sin()) * scale;
                p.line_segment(
                    [center + direction * 8.0, center + direction * 12.0],
                    stroke,
                );
            }
        }
    }
}

/// A button showing a packet glyph on the shared control material. `label`
/// is its accessible name; `text`, when given, follows the glyph.
pub(crate) fn button(
    ui: &mut egui::Ui,
    glyph: Glyph,
    label: &str,
    text: Option<&str>,
    size: Vec2,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        ui.painter().rect(
            rect,
            visuals.corner_radius,
            visuals.weak_bg_fill,
            visuals.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let color = if ui.is_enabled() {
            visuals.fg_stroke.color
        } else {
            visuals.fg_stroke.color.gamma_multiply(0.45)
        };
        match text {
            Some(text) => {
                let galley = ui.painter().layout_no_wrap(
                    text.to_owned(),
                    egui::FontId::proportional(17.0),
                    color,
                );
                let total = 16.0 + 4.0 + galley.size().x;
                let left = rect.center().x - total / 2.0;
                paint_scaled(
                    ui.painter(),
                    egui::pos2(left + 8.0, rect.center().y),
                    glyph,
                    color,
                    0.75,
                );
                ui.painter().galley(
                    egui::pos2(left + 20.0, rect.center().y - galley.size().y / 2.0),
                    galley,
                    color,
                );
            }
            None => paint_scaled(ui.painter(), rect.center(), glyph, color, 0.8),
        }
    }
    response
}

/// A glyph-only button for `ui.put`/`ui.add_sized`; it fills the given rect.
pub(crate) struct GlyphButton<'a> {
    glyph: Glyph,
    label: &'a str,
    framed: bool,
    color: Option<Color32>,
}

impl<'a> GlyphButton<'a> {
    pub(crate) fn new(glyph: Glyph, label: &'a str) -> Self {
        Self {
            glyph,
            label,
            framed: true,
            color: None,
        }
    }
    pub(crate) fn frame(mut self, framed: bool) -> Self {
        self.framed = framed;
        self
    }
    pub(crate) fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }
}

impl egui::Widget for GlyphButton<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let size = ui.available_size().max(Vec2::splat(24.0));
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let label = self.label;
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
        });
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            if self.framed {
                ui.painter().rect(
                    rect,
                    visuals.corner_radius,
                    visuals.weak_bg_fill,
                    visuals.bg_stroke,
                    egui::StrokeKind::Inside,
                );
            } else if response.is_pointer_button_down_on() || response.has_focus() {
                ui.painter().rect_filled(
                    rect.shrink(4.0),
                    visuals.corner_radius,
                    Color32::from_white_alpha(28),
                );
            }
            let mut color = self.color.unwrap_or(visuals.fg_stroke.color);
            if !ui.is_enabled() {
                color = color.gamma_multiply(0.45);
            }
            let scale = (rect.width().min(rect.height()) / 48.0 * 0.8).clamp(0.5, 1.0);
            paint_scaled(ui.painter(), rect.center(), self.glyph, color, scale);
        }
        response
    }
}
