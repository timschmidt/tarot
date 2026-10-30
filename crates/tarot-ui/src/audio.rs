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
                device_controls(
                    ui,
                    audio,
                    output,
                    false,
                    model.feedback.generation,
                    commands,
                );
            }
            ui.separator();
            ui.label(egui::RichText::new("Recording inputs").strong());
            ui.label("Default input applies to new recordings. Apps may keep their current input.");
            if audio.available && audio.inputs.is_empty() {
                ui.label("No recording inputs available");
            }
            for input in &audio.inputs {
                device_controls(ui, audio, input, true, model.feedback.generation, commands);
            }
            ui.label(
                "Input mute controls the sound server. It does not disable microphone hardware.",
            );
            if !audio.cards.is_empty() {
                egui::CollapsingHeader::new("Audio device modes")
                    .id_salt("audio_device_modes")
                    .show(ui, |ui| {
                        ui.label("Changing mode may replace playback and recording devices.");
                        for card in &audio.cards {
                            ui.push_id(
                                (card.id, audio.generation, model.feedback.generation),
                                |ui| {
                                    ui.group(|ui| {
                                        ui.set_width(ui.available_width().max(1.0));
                                        ui.label(egui::RichText::new(&card.label).strong());
                                        if card.profiles.is_empty() {
                                            ui.label("Modes unavailable");
                                        }
                                        for profile in &card.profiles {
                                            let label = if profile.available {
                                                profile.label.clone()
                                            } else {
                                                format!("{} · Unavailable", profile.label)
                                            };
                                            if ui
                                                .add_enabled(
                                                    audio.available
                                                        && audio.pending.is_none()
                                                        && profile.available,
                                                    egui::Button::new(label)
                                                        .selected(profile.active)
                                                        .min_size(Vec2::new(
                                                            ui.available_width(),
                                                            48.0,
                                                        )),
                                                )
                                                .clicked()
                                            {
                                                commands.push(Command::Audio(
                                                    UiAction::CardProfile {
                                                        card: card.id,
                                                        profile: profile.id,
                                                    },
                                                ));
                                            }
                                            ui.small(format!(
                                                "{} outputs · {} inputs",
                                                profile.outputs, profile.inputs
                                            ));
                                        }
                                    });
                                },
                            );
                        }
                    });
            }
            if model.feedback.enabled {
                ui.push_id(
                    (
                        "feedback_controls",
                        audio.generation,
                        model.feedback.generation,
                    ),
                    |ui| {
                        feedback(ui, model, commands);
                    },
                );
            }
        });
}

fn device_controls(
    ui: &mut egui::Ui,
    audio: &tarot_core::audio::Snapshot,
    device: &tarot_core::audio::Output,
    input: bool,
    feedback_generation: u64,
    commands: &mut Vec<Command>,
) {
    ui.push_id((device.id, audio.generation, feedback_generation), |ui| {
        ui.group(|ui| {
            ui.set_width(ui.available_width().max(1.0));
            ui.label(egui::RichText::new(&device.label).strong());
            ui.label(format!(
                "{}: {}%{}",
                if input { "Input level" } else { "Volume" },
                device.percent,
                if device.muted { " · Muted" } else { "" }
            ));
            let enabled = audio.available && audio.pending.is_none();
            ui.horizontal(|ui| {
                for (label, percent) in [
                    ("−", device.percent.saturating_sub(10).min(100) as u8),
                    ("+", device.percent.saturating_add(10).min(100) as u8),
                ] {
                    if ui
                        .add_enabled(
                            enabled,
                            egui::Button::new(label).min_size(Vec2::new(64.0, 48.0)),
                        )
                        .clicked()
                    {
                        commands.push(Command::Audio(if input {
                            UiAction::InputVolume {
                                input: device.id,
                                percent,
                            }
                        } else {
                            UiAction::Volume {
                                output: device.id,
                                percent,
                            }
                        }));
                    }
                }
                if ui
                    .add_enabled(
                        enabled,
                        egui::Button::new(if device.muted { "Unmute" } else { "Mute" })
                            .min_size(Vec2::new(100.0, 48.0)),
                    )
                    .clicked()
                {
                    commands.push(Command::Audio(if input {
                        UiAction::InputMute {
                            input: device.id,
                            muted: !device.muted,
                        }
                    } else {
                        UiAction::Mute {
                            output: device.id,
                            muted: !device.muted,
                        }
                    }));
                }
            });
            if device.default {
                ui.label(if input {
                    "Default input"
                } else {
                    "Default output"
                });
            } else if ui
                .add_enabled(
                    enabled,
                    egui::Button::new(if input {
                        "Use for new recordings"
                    } else {
                        "Use for new playback"
                    })
                    .min_size(Vec2::new(ui.available_width(), 48.0)),
                )
                .clicked()
            {
                commands.push(Command::Audio(if input {
                    UiAction::DefaultInput { input: device.id }
                } else {
                    UiAction::Default { output: device.id }
                }));
            }
            for port in &device.ports {
                let label = match port.available {
                    tarot_core::audio::Availability::No => format!("{} · Unavailable", port.label),
                    _ => port.label.clone(),
                };
                if ui
                    .add_enabled(
                        enabled && port.available != tarot_core::audio::Availability::No,
                        egui::Button::new(label)
                            .selected(port.active)
                            .min_size(Vec2::new(ui.available_width(), 48.0)),
                    )
                    .clicked()
                {
                    commands.push(Command::Audio(if input {
                        UiAction::InputPort {
                            input: device.id,
                            port: port.id,
                        }
                    } else {
                        UiAction::OutputPort {
                            output: device.id,
                            port: port.id,
                        }
                    }));
                }
            }
        });
    });
    ui.add_space(4.0);
}

fn feedback(ui: &mut egui::Ui, model: &ShellModel, commands: &mut Vec<Command>) {
    use tarot_core::feedback::{Profile, UiAction as Feedback};
    let state = &model.feedback;
    ui.separator();
    ui.label(egui::RichText::new("Notification feedback").strong());
    ui.label(format!(
        "Profile: {}",
        state.profile.map_or("Unknown", Profile::label)
    ));
    ui.label("The device theme controls sound, vibration and lights. Silent may still use lights.");
    for profile in Profile::ALL {
        if ui
            .add_enabled(
                state.available && state.pending.is_none(),
                egui::Button::new(profile.label())
                    .selected(state.profile == Some(profile))
                    .min_size(Vec2::new(ui.available_width(), 48.0)),
            )
            .clicked()
        {
            commands.push(Command::Feedback(Feedback::Profile {
                epoch: state.epoch,
                profile,
            }));
        }
    }
    if state.pending.is_some() {
        ui.label("Changing feedback profile…");
    }
    let mut notifications = state.notifications_enabled;
    if ui
        .add_enabled(
            state.settings_available,
            egui::Checkbox::new(&mut notifications, "Feedback for new notifications"),
        )
        .changed()
    {
        commands.push(Command::Feedback(Feedback::Notifications {
            enabled: notifications,
        }));
    }
    ui.label("Do Not Disturb suppresses new notification feedback.");
    if ui
        .add_enabled(
            state.available && state.pending.is_none() && !state.test_active,
            egui::Button::new("Test vibration").min_size(Vec2::new(ui.available_width(), 48.0)),
        )
        .clicked()
    {
        commands.push(Command::Feedback(Feedback::Test { epoch: state.epoch }));
    }
    ui.label("The current profile can suppress vibration.");
    if let Some(result) = state.test_result {
        ui.label(match result {
            tarot_core::feedback::TestResult::Ended => "Test ended.",
            tarot_core::feedback::TestResult::Unavailable => {
                "Vibration is unavailable for this profile or device."
            }
        });
    }
    if state.test_active {
        ui.label("Feedback test pending…");
        if ui.button("Stop test").clicked() {
            commands.push(Command::Feedback(Feedback::StopTest));
        }
    }
    for error in [&state.error, &state.action_error, &state.settings_error]
        .into_iter()
        .flatten()
    {
        ui.colored_label(Color32::LIGHT_RED, error);
    }
}
