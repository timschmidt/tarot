//! Original vector Quick Launch artwork; geometry is shared with touch routing.
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
use tarot_core::{
    Command, Layout, ShellModel,
    quick_launch::{self, Edit, Wave},
};

pub fn show(
    ui: &mut egui::Ui,
    model: &ShellModel,
    layout: Layout,
    wave: Option<Wave>,
    drag: Option<&tarot_core::favorite_drag::Preview>,
    commands: &mut Vec<Command>,
) {
    let painter = ui.painter().clone();
    let mut material = egui::Mesh::with_texture(crate::theme::tray(ui.ctx()));
    // Shared edges keep the original tray material continuous beneath the wave.
    let first = quick_launch::slot(layout, 0, wave);
    for edge in 0..=5 {
        let top = match edge {
            0 => first.y,
            5 => quick_launch::slot(layout, 4, wave).y,
            _ => {
                (quick_launch::slot(layout, edge - 1, wave).y
                    + quick_launch::slot(layout, edge, wave).y)
                    * 0.5
            }
        };
        let x = first.x + edge as f32 * first.width;
        let u = edge as f32 / 5.0;
        for (y, v) in [(top, 0.0), (top + tarot_core::design::DOCK, 1.0)] {
            material.vertices.push(egui::epaint::Vertex {
                pos: Pos2::new(x, y),
                uv: Pos2::new(u, v),
                color: Color32::WHITE,
            });
        }
        if edge < 5 {
            let n = edge as u32 * 2;
            material
                .indices
                .extend_from_slice(&[n, n + 2, n + 1, n + 1, n + 2, n + 3]);
        }
    }
    painter.add(egui::Shape::mesh(material));
    for index in 0..5 {
        let geometry = quick_launch::slot(layout, index, wave);
        let rect = Rect::from_min_size(
            Pos2::new(geometry.x, geometry.y),
            Vec2::new(geometry.width, geometry.height),
        );
        let favorites = drag.map_or(&model.favorites, |drag| &drag.favorites);
        let id = favorites.applications.get(index);
        let app = id.and_then(|id| model.applications.iter().find(|a| a.id == *id));
        let launcher = index == quick_launch::CAPACITY;
        let label = if launcher {
            "Apps"
        } else if id.is_none() {
            "Add"
        } else {
            app.map_or("Unavailable", |a| a.name.as_str())
        };
        let available = launcher || id.is_none() || app.is_some_and(|a| a.unavailable.is_none());
        let selected = wave.is_some_and(|w| w.selected == Some(index))
            || drag.is_some_and(|d| matches!(d.target,Some(tarot_core::favorite_drag::Target::Place(i) | tarot_core::favorite_drag::Target::Full(i) | tarot_core::favorite_drag::Target::Unavailable(i)) if i==index));
        if selected {
            painter.rect_filled(
                rect.expand(2.0),
                12.0,
                crate::theme::FOCUS.gamma_multiply(0.3),
            );
        }
        let response = ui.interact(
            rect,
            ui.id().with(("quick_launch", index)),
            if wave.is_none() {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                available,
                format!("Quick Launch: {label}"),
            )
        });
        let center = Pos2::new(rect.center().x, rect.top() + 32.0);
        if wave.is_none()
            && drag.is_none()
            && let Some(id) = id
        {
            crate::favorite_drag::register(
                ui,
                tarot_core::favorite_drag::Source::Dock {
                    id: id.clone(),
                    index,
                },
                rect,
            );
        }
        let color = if available {
            Color32::from_gray(242)
        } else {
            Color32::from_gray(115)
        };
        let tint = if available { Color32::WHITE } else { color };
        let held = drag.is_some_and(|d| id.is_some_and(|id| *id == d.source.id()));
        if held {
            painter.rect_stroke(
                rect.shrink(6.0),
                8.0,
                Stroke::new(1.0_f32, crate::theme::FOCUS),
                egui::StrokeKind::Inside,
            );
        } else if !((id.is_some() || launcher)
            && crate::theme::paint_icon(
                ui.ctx(),
                &painter,
                app.map_or("", |a| a.id.as_str()),
                launcher,
                Rect::from_center_size(center, Vec2::splat(tarot_core::design::ICON)),
                tint,
                model.calendar_day,
            ))
        {
            icon(
                &painter,
                center,
                app.map(|a| a.id.as_str()),
                launcher,
                id.is_none(),
                tint,
            );
        }
        if selected {
            let text = painter.layout_no_wrap(label.to_owned(), FontId::proportional(15.0), color);
            painter.galley(
                Pos2::new(
                    (rect.center().x - text.size().x / 2.0)
                        .clamp(8.0, (layout.width - text.size().x - 8.0).max(8.0)),
                    rect.top() - 22.0,
                ),
                text,
                color,
            );
        }
        if wave.is_none() && response.clicked() && available {
            commands.push(if launcher {
                Command::Launcher
            } else {
                Command::QuickLaunch(index)
            });
        }
        response.on_hover_text(if available {
            label.to_owned()
        } else {
            format!("{} is not installed or available", id.unwrap())
        });
    }
}

pub(crate) fn paint_drag_icon(
    ctx: &egui::Context,
    painter: &egui::Painter,
    id: &str,
    rect: Rect,
    day: Option<u8>,
) {
    if !crate::theme::paint_icon(ctx, painter, id, false, rect, Color32::WHITE, day) {
        icon(
            painter,
            rect.center(),
            Some(id),
            false,
            false,
            Color32::WHITE,
        );
    }
}

fn icon(
    p: &egui::Painter,
    at: Pos2,
    id: Option<&str>,
    launcher: bool,
    empty: bool,
    color: Color32,
) {
    let stroke = Stroke::new(2.3_f32, color);
    let point = |x, y| at + Vec2::new(x, y);
    let id = id.unwrap_or("").to_ascii_lowercase();
    if launcher {
        for x in [-10.0, 0.0, 10.0] {
            for y in [-10.0, 0.0, 10.0] {
                p.rect_filled(
                    Rect::from_center_size(point(x, y), Vec2::splat(5.0)),
                    1.0,
                    color,
                );
            }
        }
    } else if empty {
        p.circle_stroke(at, 16.0, Stroke::new(1.0_f32, Color32::from_gray(130)));
        p.line_segment([point(-7.0, 0.0), point(7.0, 0.0)], stroke);
        p.line_segment([point(0.0, -7.0), point(0.0, 7.0)], stroke);
    } else if id.contains("calls") || id.contains("dialer") {
        p.line(
            vec![
                point(-12.0, -14.0),
                point(-15.0, -8.0),
                point(-10.0, 3.0),
                point(1.0, 13.0),
                point(10.0, 15.0),
                point(15.0, 9.0),
                point(8.0, 3.0),
                point(3.0, 8.0),
                point(-5.0, 0.0),
                point(-9.0, -7.0),
                point(-5.0, -12.0),
                point(-12.0, -14.0),
            ],
            stroke,
        );
    } else if id.contains("contacts") || id.contains("phonebook") {
        p.rect_stroke(
            Rect::from_center_size(at, Vec2::new(30.0, 34.0)),
            3.0,
            stroke,
            egui::StrokeKind::Inside,
        );
        p.circle_stroke(point(0.0, -6.0), 5.0, stroke);
        p.line(
            vec![
                point(-8.0, 11.0),
                point(-8.0, 6.0),
                point(-4.0, 2.0),
                point(4.0, 2.0),
                point(8.0, 6.0),
                point(8.0, 11.0),
            ],
            stroke,
        );
    } else if id.contains("geary") || id.contains("mail") {
        p.rect_stroke(
            Rect::from_center_size(at, Vec2::new(34.0, 25.0)),
            3.0,
            stroke,
            egui::StrokeKind::Inside,
        );
        p.line(
            vec![point(-16.0, -11.0), point(0.0, 1.0), point(16.0, -11.0)],
            stroke,
        );
    } else if id.contains("calendar") || id.contains("kalendar") {
        p.rect_stroke(
            Rect::from_center_size(at, Vec2::splat(31.0)),
            3.0,
            stroke,
            egui::StrokeKind::Inside,
        );
        p.line_segment([point(-14.0, -6.0), point(14.0, -6.0)], stroke);
    } else {
        p.rect_stroke(
            Rect::from_center_size(at, Vec2::splat(31.0)),
            7.0,
            stroke,
            egui::StrokeKind::Inside,
        );
        let letter = id
            .rsplit('.')
            .nth(1)
            .unwrap_or("App")
            .chars()
            .next()
            .unwrap_or('A')
            .to_uppercase()
            .to_string();
        p.text(
            at,
            Align2::CENTER_CENTER,
            letter,
            FontId::proportional(20.0),
            color,
        );
    }
}

pub fn launch_feedback(
    ctx: &egui::Context,
    model: &ShellModel,
    layout: Layout,
    commands: &mut Vec<Command>,
) {
    let Some(feedback) = &model.launch_feedback else {
        return;
    };
    let bounds = layout.launch_feedback();
    egui::Area::new("quick_launch_feedback".into())
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::new(bounds.x, bounds.y))
        .fade_in(false)
        .movable(false)
        .show(ctx, |ui| {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(bounds.width, bounds.height), Sense::hover());
            ui.painter()
                .rect_filled(rect, 10.0, Color32::from_rgba_unmultiplied(24, 31, 35, 240));
            ui.put(
                Rect::from_min_max(rect.min + Vec2::splat(8.0), rect.max - Vec2::new(56.0, 8.0)),
                egui::Label::new(egui::RichText::new(&feedback.message).color(Color32::WHITE))
                    .wrap(),
            );
            let close = ui.put(
                Rect::from_min_size(
                    Pos2::new(rect.right() - 52.0, rect.top() + 8.0),
                    Vec2::splat(48.0),
                ),
                egui::Button::new("×"),
            );
            close.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Dismiss launch message")
            });
            if close.clicked() {
                commands.push(Command::DismissLaunchFeedback);
            }
        });
}

pub fn editor(ui: &mut egui::Ui, model: &ShellModel, layout: Layout, commands: &mut Vec<Command>) {
    let bounds = Rect::from_min_max(
        Pos2::new(16.0, 100.0),
        Pos2::new(layout.width - 16.0, layout.gesture().y - Layout::DOCK - 8.0),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
    child.visuals_mut().override_text_color = Some(Color32::from_gray(235));
    egui::ScrollArea::vertical()
        .id_salt("quick_launch_editor")
        .max_height(bounds.height().max(1.0))
        .show(&mut child, |ui| {
            ui.label("Four favorites plus the fixed Apps button");
            if let Some(error) = &model.favorites_error {
                ui.colored_label(Color32::LIGHT_RED, error);
            } else if !model.favorites_saved {
                ui.label("Saving changes…");
            }
            for (index, id) in model.favorites.applications.iter().enumerate() {
                ui.push_id(("favorite", id), |ui| {
                    ui.horizontal(|ui| {
                        let name = model
                            .applications
                            .iter()
                            .find(|a| a.id == *id)
                            .map_or(id.as_str(), |a| a.name.as_str());
                        ui.add_sized(
                            Vec2::new((bounds.width() - 168.0).max(24.0), 48.0),
                            egui::Label::new(format!("{}. {name}", index + 1)).truncate(),
                        );
                        if ui
                            .add_enabled(
                                index > 0,
                                egui::Button::new("Left").min_size(Vec2::splat(48.0)),
                            )
                            .on_hover_text("Move favorite left")
                            .clicked()
                        {
                            commands.push(Command::Favorite(Edit::Move {
                                from: index,
                                to: index - 1,
                            }));
                        }
                        if ui
                            .add_enabled(
                                index + 1 < model.favorites.applications.len(),
                                egui::Button::new("Right").min_size(Vec2::splat(48.0)),
                            )
                            .on_hover_text("Move favorite right")
                            .clicked()
                        {
                            commands.push(Command::Favorite(Edit::Move {
                                from: index,
                                to: index + 1,
                            }));
                        }
                        if ui
                            .add_sized(Vec2::splat(48.0), egui::Button::new("×"))
                            .on_hover_text("Remove favorite")
                            .clicked()
                        {
                            commands.push(Command::Favorite(Edit::Remove(index)));
                        }
                    });
                });
            }
            ui.separator();
            ui.label("Add an installed application");
            for app in &model.applications {
                if model.favorites.applications.contains(&app.id) {
                    continue;
                }
                if ui
                    .add_enabled(
                        model.favorites.applications.len() < 4 && app.unavailable.is_none(),
                        egui::Button::new(&app.name)
                            .min_size(Vec2::new(bounds.width().max(1.0), 48.0)),
                    )
                    .clicked()
                {
                    commands.push(Command::Favorite(Edit::Add(app.id.clone())));
                }
            }
        });
}
