//! Non-modal notification well, retained source rail and live actions.
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use tarot_core::{
    Command, Layout, ShellModel,
    notifications::{Notification, Surface, UiAction},
};

const MATERIAL: Color32 = Color32::from_rgb(24, 32, 36);
const ROW: Color32 = Color32::from_rgb(35, 40, 44);

pub(crate) fn entry(
    ui: &mut egui::Ui,
    model: &ShellModel,
    rect: Rect,
    commands: &mut Vec<Command>,
) {
    let count = model.notifications.items.len();
    let response = ui
        .put(
            rect,
            egui::Button::new(if count == 0 {
                String::new()
            } else {
                count.to_string()
            })
            .frame(false),
        )
        .on_hover_text("Open notification dashboard");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            response.enabled(),
            format!("Notifications, {count} retained"),
        )
    });
    let center = Pos2::new(
        if count == 0 {
            rect.center().x
        } else {
            rect.left() + 18.0
        },
        rect.center().y,
    );
    crate::menu::glyph::paint(
        ui.painter(),
        center,
        crate::menu::glyph::Glyph::Notifications,
        crate::theme::TEXT,
    );
    if response.clicked() {
        commands.push(Command::NotificationsOpen(!model.notifications_open));
    }
}

fn action(commands: &mut Vec<Command>, value: UiAction) {
    commands.push(Command::Notification(value));
}

fn button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add_sized([ui.available_width(), 48.0], egui::Button::new(text))
}

fn neutral_icon(ui: &egui::Ui, rect: Rect, count: usize) {
    let icon = Rect::from_center_size(rect.center(), Vec2::splat(26.0));
    ui.painter().rect(
        icon,
        6.0,
        Color32::from_rgb(66, 88, 91),
        Stroke::new(1.0_f32, Color32::from_rgb(143, 167, 164)),
        StrokeKind::Inside,
    );
    for x in [-6.0, 0.0, 6.0] {
        ui.painter()
            .circle_filled(icon.center() + Vec2::new(x, 0.0), 1.6, Color32::WHITE);
    }
    if count > 1 {
        let center = Pos2::new(icon.right(), icon.top());
        ui.painter()
            .circle_filled(center, 9.0, Color32::from_rgb(94, 131, 146));
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            count.to_string(),
            crate::theme::bold(ui.ctx(), 10.0),
            Color32::WHITE,
        );
    }
}

fn rail(ui: &mut egui::Ui, model: &ShellModel, commands: &mut Vec<Command>) {
    let mut groups: Vec<(&Notification, usize)> = Vec::new();
    for item in &model.notifications.items {
        if let Some(group) = groups.iter_mut().find(|(n, _)| n.source == item.source) {
            group.1 += 1;
        } else {
            groups.push((item, 1));
        }
    }
    let capacity = (ui.available_width() / 48.0).floor().max(1.0) as usize;
    let overflow = groups.len() > capacity;
    let shown = groups
        .len()
        .min(capacity.saturating_sub(usize::from(overflow)));
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if overflow {
            let label = format!("+{}", groups.len() - shown);
            if ui
                .add_sized([48.0, 48.0], egui::Button::new(label).frame(false))
                .on_hover_text("More notification sources")
                .clicked()
            {
                commands.push(Command::NotificationsOpen(true));
            }
        }
        for (item, count) in groups[..shown].iter().rev() {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
            neutral_icon(ui, rect, *count);
            if ui
                .interact(
                    rect,
                    ui.id().with(("notification-source", item.source)),
                    Sense::click(),
                )
                .on_hover_text(format!("{}: {} retained", item.app_name, count))
                .clicked()
            {
                ui.ctx().data_mut(|d| {
                    d.insert_temp(egui::Id::new("notification_rail_selection"), item.source)
                });
                commands.push(Command::NotificationsOpen(true));
            }
        }
    });
}

fn settings(ui: &mut egui::Ui, model: &ShellModel, commands: &mut Vec<Command>, compact: bool) {
    let mut controls = |ui: &mut egui::Ui| {
        let mut dnd = model.notifications.dnd;
        let mut privacy = model.notifications.hide_previews;
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
        if ui
            .add_enabled(
                !model.notifications.items.is_empty(),
                egui::Button::new("Clear all"),
            )
            .clicked()
        {
            action(commands, UiAction::Clear);
        }
    };
    ui.spacing_mut().interact_size.y = 48.0;
    if compact {
        ui.horizontal(&mut controls);
    } else {
        controls(ui);
    }
    for error in [
        &model.notifications.error,
        &model.notifications.settings_error,
    ]
    .into_iter()
    .flatten()
    {
        ui.colored_label(Color32::LIGHT_RED, error);
    }
    if !model.notifications.available {
        ui.label("Notifications unavailable");
    }
}

fn item(
    ui: &mut egui::Ui,
    item: &Notification,
    commands: &mut Vec<Command>,
    force_expand: bool,
) -> egui::Response {
    ui.push_id((item.id, item.revision), |ui| {
        let key = egui::Id::new("notification_expanded_entries");
        let expanded = force_expand
            || ui.ctx().data_mut(|d| {
                d.get_temp::<Vec<(u32, u64)>>(key)
                    .unwrap_or_default()
                    .contains(&(item.id, item.revision))
            });
        egui::Frame::new()
            .fill(ROW)
            .corner_radius(10.0)
            .inner_margin(4.0)
            .show(ui, |ui| {
                let width = ui.available_width();
                let (row, _) = ui.allocate_exact_size(Vec2::new(width, 48.0), Sense::hover());
                let dismiss = Rect::from_min_max(Pos2::new(row.right() - 48.0, row.top()), row.max);
                let content = Rect::from_min_max(row.min, Pos2::new(dismiss.left(), row.bottom()));
                let icon = Rect::from_min_size(content.min, Vec2::splat(44.0));
                neutral_icon(ui, icon, 1);
                let label =
                    Rect::from_min_max(Pos2::new(icon.right() + 4.0, row.top()), content.max);
                let painter = ui.painter().with_clip_rect(label);
                let summary = if item.summary.is_empty() {
                    "Notification"
                } else {
                    &item.summary
                };
                painter.text(
                    Pos2::new(label.left(), label.top() + 3.0),
                    egui::Align2::LEFT_TOP,
                    summary,
                    crate::theme::bold(ui.ctx(), 16.0),
                    Color32::WHITE,
                );
                let detail = if !item.live {
                    "Earlier notification"
                } else if item.orphaned {
                    "Application disconnected"
                } else if item.body.is_empty() {
                    &item.app_name
                } else {
                    &item.body
                };
                painter.text(
                    Pos2::new(label.left(), label.top() + 26.0),
                    egui::Align2::LEFT_TOP,
                    detail.lines().next().unwrap_or_default(),
                    egui::FontId::proportional(13.0),
                    Color32::from_rgb(193, 204, 198),
                );
                if ui
                    .interact(content, ui.id().with("open"), Sense::click())
                    .clicked()
                {
                    ui.ctx().data_mut(|d| {
                        let mut entries = d.get_temp::<Vec<(u32, u64)>>(key).unwrap_or_default();
                        entries.retain(|identity| *identity != (item.id, item.revision));
                        if !expanded && entries.len() < tarot_core::notifications::MAX_ENTRIES {
                            entries.push((item.id, item.revision));
                        }
                        d.insert_temp(key, entries);
                    });
                }
                if ui
                    .put(dismiss, egui::Button::new("×").frame(false))
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
                if expanded {
                    if !item.body.is_empty() {
                        ui.label(&item.body);
                    }
                    ui.label(
                        egui::RichText::new(&item.app_name)
                            .small()
                            .color(Color32::LIGHT_GRAY),
                    );
                    if item.live && !item.orphaned {
                        for choice in &item.actions {
                            if button(
                                ui,
                                if choice.label.is_empty() {
                                    "Open"
                                } else {
                                    &choice.label
                                },
                            )
                            .clicked()
                            {
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
                }
            })
            .response
    })
    .inner
}

fn compact_item(ui: &mut egui::Ui, entry: &Notification, commands: &mut Vec<Command>) {
    ui.push_id(("compact-notification", entry.id, entry.revision), |ui| {
        let live = entry.live && !entry.orphaned;
        let default = live && entry.actions.iter().any(|a| a.key == "default");
        if ui
            .add_enabled(
                default,
                egui::Button::new(&entry.summary).min_size(Vec2::new(180.0, 48.0)),
            )
            .clicked()
        {
            action(
                commands,
                UiAction::Invoke {
                    id: entry.id,
                    revision: entry.revision,
                    key: "default".into(),
                },
            );
        }
        if live {
            for choice in entry.actions.iter().filter(|a| a.key != "default") {
                if ui
                    .add_sized([100.0, 48.0], egui::Button::new(&choice.label))
                    .clicked()
                {
                    action(
                        commands,
                        UiAction::Invoke {
                            id: entry.id,
                            revision: entry.revision,
                            key: choice.key.clone(),
                        },
                    );
                }
            }
        }
        if ui
            .add_sized([48.0, 48.0], egui::Button::new("×"))
            .on_hover_text("Dismiss notification")
            .clicked()
        {
            action(
                commands,
                UiAction::Dismiss {
                    id: entry.id,
                    revision: entry.revision,
                },
            );
        }
    });
}

fn dashboard(ui: &mut egui::Ui, model: &ShellModel, layout: Layout, commands: &mut Vec<Command>) {
    let key = egui::Id::new("notification_settings");
    let mut show_settings = ui
        .ctx()
        .data_mut(|d| d.get_temp::<bool>(key).unwrap_or(false));
    let compact = layout.notification_well().height < 112.0;
    if compact {
        // Landscape with a keyboard retains a full-width row, without covering
        // either the app or keyboard. The strip entry can always close it.
        if show_settings {
            ui.horizontal(|ui| {
                if ui.add_sized([48.0, 48.0], egui::Button::new("‹")).clicked() {
                    show_settings = false;
                }
                egui::ScrollArea::horizontal()
                    .id_salt("notification_compact_settings")
                    .show(ui, |ui| settings(ui, model, commands, true));
            });
        } else {
            ui.horizontal(|ui| {
                if ui
                    .add_sized([80.0, 48.0], egui::Button::new("Settings"))
                    .clicked()
                {
                    show_settings = true;
                }
                egui::ScrollArea::horizontal()
                    .id_salt("notification_compact_entries")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if model.notifications.items.is_empty() {
                                ui.label("No notifications");
                            }
                            for entry in &model.notifications.items {
                                compact_item(ui, entry, commands);
                            }
                        });
                    });
            });
        }
    } else {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "Notifications · {}",
                    model.notifications.items.len()
                ))
                .strong(),
            );
            if ui
                .add_sized(
                    [80.0, 48.0],
                    egui::Button::new(if show_settings { "Back" } else { "Settings" }),
                )
                .clicked()
            {
                show_settings = !show_settings;
            }
            if ui
                .add_sized([48.0, 48.0], egui::Button::new("×"))
                .on_hover_text("Close dashboard")
                .clicked()
            {
                commands.push(Command::NotificationsOpen(false));
            }
        });
        egui::ScrollArea::vertical()
            .id_salt(if show_settings {
                "notification_settings_scroll"
            } else {
                "notification_entries"
            })
            .max_height(ui.available_height())
            .show(ui, |ui| {
                if show_settings {
                    settings(ui, model, commands, false);
                } else {
                    if model.notifications.items.is_empty() {
                        ui.label("No notifications");
                    }
                    let rail_source = ui.ctx().data_mut(|d| {
                        d.get_temp::<u32>(egui::Id::new("notification_rail_selection"))
                    });
                    let mut scrolled_source = false;
                    for entry in &model.notifications.items {
                        let selected = model.just_type_active()
                            && model.search.selected.as_ref()
                                == Some(&tarot_core::search::Target::Notification {
                                    id: entry.id,
                                    revision: entry.revision,
                                });
                        let response = item(ui, entry, commands, selected);
                        if selected {
                            let key = egui::Id::new("just_type_notification_scroll");
                            let selection = (
                                model.search.revision,
                                model.search.selection_revision,
                                entry.id,
                                entry.revision,
                            );
                            if ui
                                .ctx()
                                .data_mut(|d| d.get_temp::<(u64, u64, u32, u64)>(key))
                                != Some(selection)
                            {
                                ui.scroll_to_rect(response.rect, Some(egui::Align::Min));
                                ui.ctx().data_mut(|d| d.insert_temp(key, selection));
                            }
                        } else if !scrolled_source && rail_source == Some(entry.source) {
                            ui.scroll_to_rect(response.rect, Some(egui::Align::Min));
                            scrolled_source = true;
                        }
                        ui.add_space(4.0);
                    }
                    if scrolled_source {
                        ui.ctx().data_mut(|d| {
                            d.remove::<u32>(egui::Id::new("notification_rail_selection"))
                        });
                    }
                }
            });
    }
    ui.ctx().data_mut(|d| d.insert_temp(key, show_settings));
}

fn banner(ui: &mut egui::Ui, model: &ShellModel, commands: &mut Vec<Command>) {
    let Some((id, revision)) = model.notifications.banner else {
        return;
    };
    let Some(entry) = model
        .notifications
        .items
        .iter()
        .find(|n| n.id == id && n.revision == revision)
    else {
        return;
    };
    // Replacement changes the actual widget identity even if pixels stay put.
    ui.push_id((id, revision), |ui| {
        let compact = ui.available_height() < 72.0;
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 56.0).max(1.0);
            ui.vertical(|ui| {
                ui.set_max_width(width);
                if !compact {
                    ui.label(egui::RichText::new(&entry.app_name).small());
                }
                let label = if model.notifications.hide_previews {
                    "New notification"
                } else {
                    &entry.summary
                };
                if ui
                    .add_sized([width, 48.0], egui::Button::new(label).wrap().frame(false))
                    .clicked()
                {
                    if entry.actions.iter().any(|a| a.key == "default")
                        && !entry.orphaned
                        && entry.live
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
                .add_sized([48.0, 48.0], egui::Button::new("×").frame(false))
                .on_hover_text("Hide banner; keep in dashboard")
                .clicked()
            {
                action(commands, UiAction::HideBanner { id, revision });
            }
        });
    });
}

pub fn show(ctx: &egui::Context, model: &ShellModel, layout: Layout, commands: &mut Vec<Command>) {
    ctx.data_mut(|d| {
        let key = egui::Id::new("notification_expanded_entries");
        let mut entries = d.get_temp::<Vec<(u32, u64)>>(key).unwrap_or_default();
        entries.retain(|&(id, revision)| {
            model
                .notifications
                .items
                .iter()
                .any(|n| n.id == id && n.revision == revision)
        });
        d.insert_temp(key, entries);
    });
    let surface = tarot_core::notifications::surface(model);
    if surface != Surface::Dashboard {
        ctx.data_mut(|d| d.remove::<bool>(egui::Id::new("notification_settings")));
    }
    let well = layout.notification_well();
    if well.height <= 0.0 {
        return;
    }
    let rect = Rect::from_min_size(
        Pos2::new(well.x, well.y),
        Vec2::new(well.width, well.height),
    );
    egui::Area::new("notification_well".into())
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .constrain(false)
        .movable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.set_min_size(rect.size());
            ui.set_max_size(rect.size());
            ui.set_clip_rect(rect);
            ui.painter().rect_filled(rect, 0.0, MATERIAL);
            ui.painter().line_segment(
                [rect.left_top(), rect.right_top()],
                Stroke::new(1.0_f32, Color32::from_rgb(65, 83, 84)),
            );
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
            match surface {
                Surface::Hidden => {}
                Surface::Rail => rail(ui, model, commands),
                Surface::Banner => banner(ui, model, commands),
                Surface::Dashboard => dashboard(ui, model, layout, commands),
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tarot_core::notifications::{Action, Incoming, Store};

    fn incoming() -> Incoming {
        Incoming {
            app_name: "Fixture".into(),
            summary: "Synthetic message".into(),
            body: "Synthetic body".into(),
            actions: vec![Action {
                key: "default".into(),
                label: "Open".into(),
            }],
            urgency: 1,
            expire_ms: 0,
            resident: true,
            transient: false,
        }
    }
    fn layout(model: &ShellModel) -> Layout {
        Layout {
            width: 360.0,
            height: 720.0,
            keyboard_height: 0.0,
            notification_height: 0.0,
        }
        .with_notifications(model)
    }
    fn frame(ctx: &egui::Context, model: &ShellModel, events: Vec<egui::Event>) -> Vec<Command> {
        crate::prepare_pointer_input(ctx, &events);
        let mut commands = vec![];
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 720.0))),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                commands = crate::show(ctx, model, layout(model));
            },
        );
        commands
    }
    fn pointer(pos: Pos2, pressed: bool) -> Vec<egui::Event> {
        let mut events = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        if !pressed {
            events.push(egui::Event::PointerGone);
        }
        events
    }
    #[test]
    fn a_held_banner_cannot_invoke_a_replacement_but_a_fresh_tap_can() {
        let ctx = egui::Context::default();
        let mut store = Store::default();
        let id = store.notify(":1.1", 0, incoming(), Duration::ZERO).unwrap();
        let mut model = ShellModel::default();
        model.add_window(tarot_core::WindowId(7), "Fixture".into(), "fixture".into());
        model.notifications = store.snapshot();
        for _ in 0..3 {
            assert!(frame(&ctx, &model, vec![]).is_empty());
        }
        let well = layout(&model).notification_well();
        let pos = Pos2::new(120.0, well.y + well.height * 0.5);
        assert!(frame(&ctx, &model, pointer(pos, true)).is_empty());
        store
            .notify(":1.1", id, incoming(), Duration::from_millis(50))
            .unwrap();
        model.notifications = store.snapshot();
        assert!(frame(&ctx, &model, pointer(pos, false)).is_empty());
        assert!(frame(&ctx, &model, vec![]).is_empty());
        assert!(frame(&ctx, &model, pointer(pos, true)).is_empty());
        assert_eq!(
            frame(&ctx, &model, pointer(pos, false)),
            vec![Command::Notification(UiAction::Invoke {
                id,
                revision: model.notifications.items[0].revision,
                key: "default".into()
            })]
        );
    }
}
