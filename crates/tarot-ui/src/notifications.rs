//! Bottom banners and the retained notification dashboard.
use egui::{Color32, Pos2, Rect, Vec2};
use tarot_core::{
    Command, Layout, ShellModel,
    notifications::{Notification, UiAction, banner_rect},
};

fn action(commands: &mut Vec<Command>, action: UiAction) {
    commands.push(Command::Notification(action));
}
fn button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add_sized([ui.available_width(), 48.0], egui::Button::new(label))
}

fn item(ui: &mut egui::Ui, item: &Notification, commands: &mut Vec<Command>) {
    ui.push_id((item.id, item.revision), |ui| {
        egui::Frame::new()
            .fill(Color32::from_rgb(35, 40, 44))
            .corner_radius(8.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_max_width((ui.available_width() - 52.0).max(80.0));
                        ui.label(
                            egui::RichText::new(if item.app_name.is_empty() {
                                "Notification"
                            } else {
                                &item.app_name
                            })
                            .small()
                            .color(Color32::LIGHT_GRAY),
                        );
                        ui.label(egui::RichText::new(&item.summary).strong());
                    });
                    if ui
                        .add_sized([48.0, 48.0], egui::Button::new("×"))
                        .on_hover_text("Dismiss notification")
                        .clicked()
                    {
                        action(
                            commands,
                            UiAction::Dismiss {
                                id: item.id,
                                revision: item.revision,
                            },
                        );
                    }
                });
                if !item.body.is_empty() {
                    ui.label(item.body.chars().take(512).collect::<String>());
                }
                if !item.live {
                    ui.label(
                        egui::RichText::new("Earlier notification")
                            .small()
                            .color(Color32::GRAY),
                    );
                } else if item.orphaned {
                    ui.label(
                        egui::RichText::new("Application disconnected")
                            .small()
                            .color(Color32::GRAY),
                    );
                }
                if item.live && !item.orphaned {
                    for choice in &item.actions {
                        let label = if choice.label.is_empty() {
                            "Open"
                        } else {
                            &choice.label
                        };
                        if button(ui, label).clicked() {
                            action(
                                commands,
                                UiAction::Invoke {
                                    id: item.id,
                                    revision: item.revision,
                                    key: choice.key.clone(),
                                },
                            );
                        }
                    }
                }
            });
    });
}

pub fn show(ctx: &egui::Context, model: &ShellModel, layout: Layout, commands: &mut Vec<Command>) {
    if model.notifications_open {
        ctx.set_sublayer(
            egui::LayerId::new(egui::Order::Foreground, "notification_scrim".into()),
            egui::LayerId::new(egui::Order::Foreground, "notification_dashboard".into()),
        );
        egui::Popup::close_all(ctx);
        let content = layout.content();
        let full = Rect::from_min_size(
            Pos2::new(content.x, content.y),
            Vec2::new(content.width, content.height),
        );
        egui::Area::new("notification_scrim".into())
            .order(egui::Order::Foreground)
            .fixed_pos(full.min)
            .movable(false)
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_min_size(full.size());
                ui.painter()
                    .rect_filled(full, 0.0, Color32::from_black_alpha(120));
                if ui
                    .interact(
                        full,
                        ui.id().with("dismiss-dashboard"),
                        egui::Sense::click(),
                    )
                    .clicked()
                {
                    commands.push(Command::NotificationsOpen(false));
                }
            });
        let height = (layout.height * 0.66).min(layout.gesture().y - Layout::STATUS - 8.0);
        let panel = Rect::from_min_size(
            Pos2::new(8.0, layout.gesture().y - height),
            Vec2::new(layout.width - 16.0, height - 8.0),
        );
        egui::Area::new("notification_dashboard".into())
            .order(egui::Order::Foreground)
            .fixed_pos(panel.min)
            .movable(false)
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_min_size(panel.size());
                ui.set_max_size(panel.size());
                egui::Frame::new()
                    .fill(Color32::from_rgb(22, 29, 33))
                    .corner_radius(12.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_min_size(panel.size() - Vec2::splat(16.0));
                        ui.set_max_size(panel.size() - Vec2::splat(16.0));
                        ui.horizontal(|ui| {
                            ui.heading("Notifications");
                            if ui
                                .add_sized([48.0, 48.0], egui::Button::new("×"))
                                .on_hover_text("Close dashboard")
                                .clicked()
                            {
                                commands.push(Command::NotificationsOpen(false));
                            }
                        });
                        if let Some(error) = &model.notifications.error {
                            ui.colored_label(Color32::LIGHT_RED, error);
                        }
                        if !model.notifications.available {
                            ui.label("Notifications unavailable");
                        }
                        if let Some(error) = &model.notifications.settings_error {
                            ui.colored_label(Color32::LIGHT_RED, error);
                        }
                        let mut dnd = model.notifications.dnd;
                        let mut privacy = model.notifications.hide_previews;
                        ui.spacing_mut().interact_size.y = 48.0;
                        let dnd_changed = ui
                            .add_enabled(
                                model.notifications.settings_available,
                                egui::Checkbox::new(&mut dnd, "Do not disturb"),
                            )
                            .changed();
                        let privacy_changed = ui
                            .add_enabled(
                                model.notifications.settings_available,
                                egui::Checkbox::new(&mut privacy, "Hide banner previews"),
                            )
                            .changed();
                        if dnd_changed || privacy_changed {
                            action(
                                commands,
                                UiAction::Settings {
                                    dnd,
                                    hide_previews: privacy,
                                },
                            );
                        }
                        ui.horizontal(|ui| {
                            ui.label(format!("{} retained", model.notifications.items.len()));
                            if ui
                                .add_enabled(
                                    !model.notifications.items.is_empty(),
                                    egui::Button::new("Clear all"),
                                )
                                .clicked()
                            {
                                action(commands, UiAction::Clear);
                            }
                        });
                        egui::ScrollArea::vertical()
                            .id_salt("notification_entries")
                            .max_height(ui.available_height())
                            .show(ui, |ui| {
                                if model.notifications.items.is_empty() {
                                    ui.label("No notifications");
                                }
                                for entry in &model.notifications.items {
                                    item(ui, entry, commands);
                                    ui.add_space(6.0);
                                }
                            });
                    });
            });
    } else if let Some((id, revision)) = model.notifications.banner
        && let Some(entry) = model
            .notifications
            .items
            .iter()
            .find(|n| n.id == id && n.revision == revision)
    {
        let rect = banner_rect(layout);
        egui::Area::new("notification_banner".into())
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(rect.x, rect.y))
            .movable(false)
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_min_size(Vec2::new(rect.width, rect.height));
                ui.set_max_size(Vec2::new(rect.width, rect.height));
                egui::Frame::new()
                    .fill(Color32::from_rgb(24, 32, 36))
                    .corner_radius(10.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let width = (rect.width - 72.0).max(80.0);
                            ui.vertical(|ui| {
                                ui.set_max_width(width);
                                ui.label(egui::RichText::new(&entry.app_name).small());
                                let label = if model.notifications.hide_previews {
                                    "New notification"
                                } else {
                                    &entry.summary
                                };
                                if ui
                                    .add_sized(
                                        [width, 48.0],
                                        egui::Button::new(label).wrap().frame(false),
                                    )
                                    .clicked()
                                {
                                    if entry.actions.iter().any(|a| a.key == "default")
                                        && !entry.orphaned
                                    {
                                        action(
                                            commands,
                                            UiAction::Invoke {
                                                id,
                                                revision,
                                                key: "default".into(),
                                            },
                                        );
                                    } else {
                                        commands.push(Command::NotificationsOpen(true));
                                    }
                                }
                            });
                            if ui
                                .add_sized([48.0, 48.0], egui::Button::new("×"))
                                .on_hover_text("Hide banner; keep in dashboard")
                                .clicked()
                            {
                                action(commands, UiAction::HideBanner { id, revision });
                            }
                        });
                    });
            });
    }
}
