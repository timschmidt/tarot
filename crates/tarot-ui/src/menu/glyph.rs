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
}

pub(crate) fn paint(p: &Painter, center: Pos2, glyph: Glyph, color: Color32) {
    let stroke = Stroke::new(1.8_f32, color);
    let at = |x, y| center + Vec2::new(x, y);
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
                    origin + Vec2::new(angle.cos(), angle.sin()) * radius
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
            p.circle_filled(at(0.0, 7.0), 1.7, color);
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
            p.circle_filled(at(0.0, 4.5), 1.4, color);
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
        Glyph::Sun => {
            p.circle_stroke(center, 5.0, stroke);
            for i in 0..8 {
                let angle = i as f32 * FRAC_PI_4;
                let direction = Vec2::new(angle.cos(), angle.sin());
                p.line_segment(
                    [center + direction * 8.0, center + direction * 12.0],
                    stroke,
                );
            }
        }
    }
}
