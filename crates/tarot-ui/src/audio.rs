//! Playback controls show the server's observed volume and mute state.
use egui::{Color32, Vec2};
use tarot_core::{Command, Layout, ShellModel, SystemPage, audio::UiAction};

pub fn show_key_feedback(
    ctx: &egui::Context,
    feedback: &tarot_core::audio::KeyFeedback,
    layout: Layout,
) {
    let bounds = layout.volume_feedback();
    egui::Area::new("hardware_volume_feedback".into())
        .order(egui::Order::Tooltip)
        .fixed_pos(egui::pos2(bounds.x, bounds.y))
        .movable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(
                egui::vec2(bounds.width, bounds.height),
                egui::Sense::click(),
            );
            let p = ui.painter().with_clip_rect(rect);
            p.rect_filled(rect, 12.0, Color32::from_rgb(30, 39, 44));
            let title = match feedback.percent {
                None => "Audio unavailable".to_owned(),
                Some(_) if feedback.failed => "Volume change failed".to_owned(),
                Some(_) if feedback.muted => "Muted".to_owned(),
                Some(percent) => format!("Volume {percent}%"),
            };
            let title = if feedback.pending {
                format!("{title} …")
            } else {
                title
            };
            p.text(
                rect.center_top() + egui::vec2(0.0, 20.0),
                egui::Align2::CENTER_CENTER,
                title,
                egui::FontId::proportional(16.0),
                Color32::WHITE,
            );
            if let Some(percent) = feedback.percent {
                let bar = egui::Rect::from_min_size(
                    rect.left_bottom() + egui::vec2(14.0, -18.0),
                    egui::vec2((rect.width() - 28.0).max(1.0), 5.0),
                );
                p.rect_filled(bar, 3.0, Color32::from_gray(75));
                let mut filled = bar;
                filled.max.x = filled.min.x + filled.width() * (percent.min(100) as f32 / 100.0);
                p.rect_filled(
                    filled,
                    3.0,
                    if feedback.muted {
                        Color32::GRAY
                    } else {
                        Color32::from_rgb(115, 195, 207)
                    },
                );
            }
        });
}

pub fn show(ui: &mut egui::Ui, model: &ShellModel, _layout: Layout, commands: &mut Vec<Command>) {
    if ui
        .add_sized(
            [ui.available_width(), 48.0],
            egui::Button::new("Back to system controls"),
        )
        .clicked()
    {
        commands.push(Command::SystemPage(SystemPage::Power));
    }
    let audio = &model.audio;
    if !audio.enabled {
        ui.label("System services are disabled for this session.");
    }
    if let Some(error) = &audio.error {
        ui.colored_label(Color32::LIGHT_RED, error);
    }
    if let Some(error) = &audio.action_error {
        ui.colored_label(Color32::LIGHT_RED, error);
    }
    if audio.pending.is_some() {
        ui.label("Changing audio…");
    }
    if audio.available && audio.outputs.is_empty() {
        ui.label("No playback outputs available");
    }
    ui.label("Default output applies to new playback. Apps may keep their own route.");
    egui::ScrollArea::vertical()
        .id_salt("audio_outputs")
        .max_height(ui.available_height().max(1.0))
        .show(ui, |ui| {
            for output in &audio.outputs {
                ui.push_id(output.id, |ui| {
                    ui.group(|ui| {
                        ui.set_width(ui.available_width().max(1.0));
                        ui.label(egui::RichText::new(&output.label).strong());
                        ui.label(format!(
                            "Volume: {}%{}",
                            output.percent,
                            if output.muted { " · Muted" } else { "" }
                        ));
                        let enabled = audio.available && audio.pending.is_none();
                        ui.horizontal(|ui| {
                            for (label, percent) in [
                                ("−", output.percent.saturating_sub(10).min(100) as u8),
                                ("+", output.percent.saturating_add(10).min(100) as u8),
                            ] {
                                if ui
                                    .add_enabled(
                                        enabled,
                                        egui::Button::new(label).min_size(Vec2::new(64.0, 48.0)),
                                    )
                                    .clicked()
                                {
                                    commands.push(Command::Audio(UiAction::Volume {
                                        output: output.id,
                                        percent,
                                    }));
                                }
                            }
                            if ui
                                .add_enabled(
                                    enabled,
                                    egui::Button::new(if output.muted { "Unmute" } else { "Mute" })
                                        .min_size(Vec2::new(100.0, 48.0)),
                                )
                                .clicked()
                            {
                                commands.push(Command::Audio(UiAction::Mute {
                                    output: output.id,
                                    muted: !output.muted,
                                }));
                            }
                        });
                        if output.default {
                            ui.label("Default output");
                        } else if ui
                            .add_enabled(
                                enabled,
                                egui::Button::new("Use for new playback")
                                    .min_size(Vec2::new(ui.available_width(), 48.0)),
                            )
                            .clicked()
                        {
                            commands.push(Command::Audio(UiAction::Default { output: output.id }));
                        }
                    });
                });
                ui.add_space(4.0);
            }
        });
}
