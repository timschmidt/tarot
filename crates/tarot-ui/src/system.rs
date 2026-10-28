//! The status menu shows observed state, with separate pending/error feedback.
use egui::{Color32, Pos2, Rect, Vec2};
use tarot_core::{Command, Layout, ShellModel, SystemPage, power::UiAction};

pub fn status_label(model: &ShellModel) -> String {
    let battery = &model.power.battery;
    if battery.available && battery.present {
        format!(
            "{}{}  v",
            battery.level_label(),
            if battery.state == tarot_core::power::ChargeState::Charging {
                "+"
            } else {
                ""
            }
        )
    } else {
        "System  v".into()
    }
}

pub fn show(ctx: &egui::Context, model: &ShellModel, layout: Layout, commands: &mut Vec<Command>) {
    let popup_page = egui::Id::new("system_menu_popup_page");
    if !model.system_menu_open {
        ctx.data_mut(|data| data.remove_temp::<SystemPage>(popup_page));
        return;
    }
    let previous_page = ctx.data_mut(|data| data.get_temp::<SystemPage>(popup_page));
    if previous_page != Some(model.system_page) {
        egui::Popup::close_all(ctx);
        ctx.data_mut(|data| data.insert_temp(popup_page, model.system_page));
    }
    // An outside click promotes its Area. Keep the interactive panel directly
    // above that scrim, including when the same panel is reopened later.
    ctx.set_sublayer(
        egui::LayerId::new(egui::Order::Foreground, "system_scrim".into()),
        egui::LayerId::new(egui::Order::Foreground, "system_menu".into()),
    );
    let content = layout.content();
    let full = Rect::from_min_size(
        Pos2::new(content.x, content.y),
        Vec2::new(content.width, content.height),
    );
    egui::Area::new("system_scrim".into())
        .order(egui::Order::Foreground)
        .fixed_pos(full.min)
        .movable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.set_min_size(full.size());
            ui.painter()
                .rect_filled(full, 0.0, Color32::from_black_alpha(120));
            if ui
                .interact(full, ui.id().with("close"), egui::Sense::click())
                .clicked()
            {
                commands.push(Command::SystemMenuOpen(false));
            }
        });
    egui::Area::new("system_menu".into())
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::new(8.0, 36.0))
        .movable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.set_width(layout.width - 16.0);
            ui.set_max_height((layout.gesture().y - 44.0).max(160.0));
            egui::Frame::new()
                .fill(Color32::from_rgb(22, 29, 33))
                .corner_radius(12.0)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_width(layout.width - 40.0);
                    ui.set_max_height((layout.gesture().y - 68.0).max(136.0));
                    ui.spacing_mut().interact_size.y = 48.0;
                    ui.horizontal(|ui| {
                        ui.heading(match model.system_page {
                            SystemPage::Wifi => "Wi-Fi",
                            SystemPage::Power => "Power & display",
                            SystemPage::Audio => "Audio",
                        });
                        if ui.add_sized([48.0, 48.0], egui::Button::new("×")).clicked() {
                            commands.push(Command::SystemMenuOpen(false));
                        }
                    });
                    if model.system_page == SystemPage::Audio {
                        super::audio::show(ui, model, layout, commands);
                        return;
                    }
                    if model.system_page == SystemPage::Wifi {
                        if model.network_input.is_some() {
                            super::credentials::show(ui, model, commands);
                            return;
                        }
                        super::wifi::show(ui, model, layout, commands);
                        return;
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("power_controls")
                        .max_height((layout.gesture().y - 124.0).max(80.0))
                        .show(ui, |ui| {
                            if !model.power.enabled {
                                ui.label("System services are disabled for this session.");
                            }
                            let battery = &model.power.battery;
                            ui.label(egui::RichText::new(battery.level_label()).strong());
                            if battery.available && battery.present {
                                ui.label(battery.state.label());
                            }
                            if let Some(external) = battery.external_power {
                                ui.label(if external {
                                    "External power connected"
                                } else {
                                    "On battery power"
                                });
                            }
                            if let Some(error) = &battery.error {
                                ui.colored_label(Color32::LIGHT_RED, error);
                            }
                            ui.separator();
                            if model.sensors.enabled {
                                let mut locked = model.rotation_locked;
                                if ui.checkbox(&mut locked, "Rotation lock").changed() {
                                    commands.push(Command::RotationLock(locked));
                                }
                                if !model.sensors.available {
                                    ui.label("Rotation sensor unavailable");
                                } else if !model.sensors.has_accelerometer {
                                    ui.label("No orientation sensor");
                                }
                                ui.separator();
                            }
                            let brightness = &model.power.brightness;
                            ui.label(format!(
                                "Brightness: {}",
                                brightness
                                    .percent()
                                    .map_or_else(|| "unknown".into(), |p| format!("{p}%"))
                            ));
                            if let Some(percent) = brightness.pending_percent {
                                ui.label(format!("Changing to {percent}%…"));
                            }
                            let enabled =
                                brightness.can_set && brightness.pending_percent.is_none();
                            ui.horizontal(|ui| {
                                let percent = brightness.percent().unwrap_or(50);
                                for (label, target) in [
                                    ("−", percent.saturating_sub(10).max(5)),
                                    ("40%", 40),
                                    ("60%", 60),
                                    ("+", percent.saturating_add(10).min(100)),
                                ] {
                                    if ui
                                        .add_enabled(
                                            enabled,
                                            egui::Button::new(label)
                                                .min_size(Vec2::new(64.0, 48.0)),
                                        )
                                        .clicked()
                                    {
                                        commands.push(Command::Power(UiAction::Brightness {
                                            percent: target,
                                        }));
                                    }
                                }
                            });
                            if let Some(error) = &brightness.error {
                                ui.colored_label(Color32::LIGHT_RED, error);
                            }
                            if let Some(error) = &brightness.action_error {
                                ui.colored_label(Color32::LIGHT_RED, error);
                            }
                            ui.separator();
                            if ui
                                .add_sized(
                                    [ui.available_width(), 48.0],
                                    egui::Button::new(format!(
                                        "Wi-Fi: {}",
                                        model.network.state.label()
                                    )),
                                )
                                .clicked()
                            {
                                commands.push(Command::SystemPage(SystemPage::Wifi));
                            }
                            if ui
                                .add_sized([ui.available_width(), 48.0], egui::Button::new("Audio"))
                                .clicked()
                            {
                                commands.push(Command::SystemPage(SystemPage::Audio));
                            }
                            if model.lock_available
                                && ui
                                    .add_sized(
                                        [ui.available_width(), 48.0],
                                        egui::Button::new("Lock"),
                                    )
                                    .clicked()
                            {
                                commands.push(Command::Lock);
                            }
                            ui.separator();
                            idle_controls(ui, model, commands);
                            if model.display.settings_available {
                                if model.display.settings_error.is_some() {
                                    ui.colored_label(
                                        Color32::LIGHT_RED,
                                        "Settings could not be saved. Changes apply until logout.",
                                    );
                                } else if !model.display.settings_saved {
                                    ui.label("Saving settings…");
                                }
                            }
                        });
                });
        });
}

fn timeout_label(seconds: u32) -> String {
    if seconds.is_multiple_of(60) {
        format!("{} min", seconds / 60)
    } else {
        format!("{seconds} sec")
    }
}

fn idle_controls(ui: &mut egui::Ui, model: &ShellModel, commands: &mut Vec<Command>) {
    if !model.lock_available {
        return;
    }
    ui.label(egui::RichText::new("Automatic locking").strong());
    if !model.display.idle_configurable {
        ui.label("Idle settings are managed for this session.");
        return;
    }
    let previous = model.display.idle;
    let mut settings = previous;
    const TIMES: [u32; 9] = [30, 60, 120, 180, 300, 600, 900, 1800, 3600];
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = 44.0;
        ui.label("Lock after inactivity");
        egui::ComboBox::from_id_salt("idle_lock_timeout")
            .selected_text(timeout_label(settings.lock_after_seconds))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for seconds in TIMES {
                    ui.selectable_value(
                        &mut settings.lock_after_seconds,
                        seconds,
                        timeout_label(seconds),
                    );
                }
            });
        if model.display.panel_power_available {
            ui.checkbox(
                &mut settings.screen_off_enabled,
                "Turn screen off automatically",
            );
            ui.add_enabled_ui(settings.screen_off_enabled, |ui| {
                ui.label("Screen off after inactivity");
                egui::ComboBox::from_id_salt("idle_off_timeout")
                    .selected_text(timeout_label(settings.off_after_seconds))
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        for seconds in TIMES
                            .into_iter()
                            .filter(|s| *s >= settings.lock_after_seconds)
                        {
                            ui.selectable_value(
                                &mut settings.off_after_seconds,
                                seconds,
                                timeout_label(seconds),
                            );
                        }
                    });
            });
        } else {
            ui.label("Screen power control is unavailable in this session.");
        }
    });
    if settings != previous {
        if settings.screen_off_enabled {
            settings.off_after_seconds =
                settings.off_after_seconds.max(settings.lock_after_seconds);
        }
        commands.push(Command::IdleTimeouts(settings));
    }
}
