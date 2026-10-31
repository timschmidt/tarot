//! The shared egui shell view; all platform effects are returned as commands.
pub mod audio;
pub mod authorization;
pub mod bluetooth;
pub mod controls;
pub mod credentials;
mod favorite_drag;
mod just_type;
pub mod keyboard;
mod launcher;
mod launcher_paging;
pub mod lock_recovery;
mod menu;
mod native_icons;
pub mod native_text;
pub mod notifications;
pub mod quick_launch;
pub mod system;
pub mod theme;
pub mod wifi;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use tarot_core::motion::Presentation;
use tarot_core::{CardDrop, Command, Layout, ShellModel, View};

/// egui 0.33 counts multi-clicks by time alone. A distant new press starts
/// a separate click sequence so tapping a key cannot select text elsewhere.
pub fn prepare_pointer_input(ctx: &egui::Context, events: &[egui::Event]) {
    theme::install(ctx);
    let id = egui::Id::new("shell_pointer_press");
    let mut previous = ctx.data_mut(|data| data.get_temp::<Pos2>(id));
    for event in events {
        if let egui::Event::PointerButton {
            pos, pressed: true, ..
        } = event
        {
            let distance = ctx.options(|options| options.input_options.max_click_dist);
            if previous.is_some_and(|previous| previous.distance(*pos) > distance)
                && !ctx.input(|input| input.pointer.any_down())
            {
                ctx.input_mut(|input| input.pointer = Default::default());
            }
            previous = Some(*pos);
        }
    }
    if let Some(previous) = previous {
        ctx.data_mut(|data| data.insert_temp(id, previous));
    }
}

/// End an interrupted pointer sequence without clicking the pressed control.
pub fn cancel_pointer_input(events: &mut Vec<egui::Event>) {
    // Retire gesture ownership before synthetic button releases clear egui.
    events.push(egui::Event::PointerGone);
    // PointerGone alone leaves egui's button held. Move outside before release.
    let outside = Pos2::new(-10000.0, -10000.0);
    events.push(egui::Event::PointerMoved(outside));
    for button in [
        egui::PointerButton::Primary,
        egui::PointerButton::Secondary,
        egui::PointerButton::Middle,
    ] {
        events.push(egui::Event::PointerButton {
            pos: outside,
            button,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
    }
    events.push(egui::Event::PointerGone);
}

pub fn show(ctx: &egui::Context, model: &ShellModel, layout: Layout) -> Vec<Command> {
    let layout = layout.with_notifications(model);
    theme::install(ctx);
    theme::prepare_art(ctx);
    ctx.layer_painter(egui::LayerId::background())
        .add(wallpaper(ctx, layout));
    show_with_presentation(ctx, model, layout, &Presentation::settled(model, layout))
}

/// Original Nocturne artwork; the compositor caches it below native clients.
pub fn wallpaper(ctx: &egui::Context, layout: Layout) -> egui::Shape {
    theme::wallpaper(ctx, layout)
}

pub fn show_with_presentation(
    ctx: &egui::Context,
    model: &ShellModel,
    layout: Layout,
    presentation: &Presentation,
) -> Vec<Command> {
    show_with_native_fullscreen(ctx, model, layout, presentation, false)
}

/// Native fullscreen hides the status bar while retaining shell navigation.
/// The compositor decides this only from the client's committed state.
pub fn show_with_native_fullscreen(
    ctx: &egui::Context,
    model: &ShellModel,
    layout: Layout,
    presentation: &Presentation,
    fullscreen: bool,
) -> Vec<Command> {
    theme::install(ctx);
    theme::prepare_art(ctx);
    let mut commands = Vec::new();
    let dashboard_epoch = egui::Id::new("notification_dashboard_popup_epoch");
    let was_dashboard = ctx.data_mut(|d| d.get_temp::<bool>(dashboard_epoch).unwrap_or(false));
    if model.notifications_open && !was_dashboard {
        egui::Popup::close_all(ctx);
    }
    ctx.data_mut(|d| d.insert_temp(dashboard_epoch, model.notifications_open));
    let popup_epoch = egui::Id::new("top_menu_popup_epoch");
    if model.system_menu_open {
        if ctx.data_mut(|d| d.get_temp::<u64>(popup_epoch)) != Some(model.menus.revision) {
            egui::Popup::close_all(ctx);
            ctx.data_mut(|d| d.insert_temp(popup_epoch, model.menus.revision));
        }
    } else {
        ctx.data_mut(|d| d.remove::<u64>(popup_epoch));
    }
    if matches!(model.view, View::Home | View::Overview)
        && !model.system_menu_open
        && !model.card_menu_open
    {
        let typed = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| {
                    if let egui::Event::Text(t) = e {
                        Some(t.as_str())
                    } else {
                        None
                    }
                })
                .collect::<String>()
        });
        if !typed.is_empty() {
            commands.push(Command::JustType(tarot_core::search::Action::Open));
            commands.push(Command::SearchApplications(typed));
        }
    }
    native_icons::prune(ctx, model);
    let favorite_frame = favorite_drag::begin(ctx, model, layout);
    let paging_frame = launcher_paging::begin(
        ctx,
        model,
        layout,
        favorite_frame.preview.is_some(),
        presentation.overview <= 0.001 && presentation.windows.iter().all(|w| w.opacity <= 0.001),
    );
    let mut search_focused = false;
    let search_id = launcher::text_id(model);
    let modal = model.system_menu_open || model.launcher.menu_open;
    if modal {
        ctx.memory_mut(|memory| memory.surrender_focus(search_id));
    }
    let keyboard_rect = layout.keyboard();
    let keyboard_pointer_id = egui::Id::new("shell_keyboard_pointer");
    let (pressed_inside, released, down) = ctx.input(|input| {
        (
            input.events.iter().any(|event| {
                matches!(event, egui::Event::PointerButton { pos, pressed: true, .. }
            if keyboard_rect.contains(pos.x, pos.y) || layout.gesture().contains(pos.x,pos.y))
            }),
            input.pointer.any_released(),
            input.pointer.any_down(),
        )
    });
    let pointer_owned = pressed_inside
        || ctx.data_mut(|data| data.get_temp::<bool>(keyboard_pointer_id).unwrap_or(false));
    let keyboard_interaction = model.shell_text_focus
        && model.view == View::Launcher
        && pointer_owned
        && (down || released);
    ctx.data_mut(|data| data.insert_temp(keyboard_pointer_id, pointer_owned && down));
    let full = Rect::from_min_size(Pos2::ZERO, Vec2::new(layout.width, layout.height));

    egui::Area::new("shell_chrome".into())
        .fixed_pos(Pos2::ZERO)
        .movable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.set_min_size(full.size());
            let painter = ui.painter().clone();
            if !fullscreen {
                let status =
                    Rect::from_min_size(Pos2::ZERO, Vec2::new(layout.width, Layout::STATUS));
                painter.rect_filled(status, 0.0, theme::STATUS);
                let title = match model.view {
                    View::App(id) => model
                        .cards
                        .iter()
                        .find(|card| card.id == id)
                        .map(|card| card.title.as_str())
                        .unwrap_or("Tarot"),
                    View::Launcher => {
                        if model.just_type_active() {
                            "Just Type"
                        } else {
                            "Launcher"
                        }
                    }
                    _ => "Tarot",
                };
                let title_limit = (layout.width / 2.0 - 36.0).max(24.0);
                let measured = painter.layout_no_wrap(
                    title.to_owned(),
                    theme::bold(ctx, 15.0),
                    Color32::WHITE,
                );
                let arrow_x = (measured.size().x + 20.0).min(title_limit - 8.0).max(24.0);
                let activity_button =
                    Rect::from_min_size(Pos2::ZERO, Vec2::new(title_limit, Layout::STATUS));
                if model.system_menu_open && model.menus.entry == tarot_core::SystemPage::Activity {
                    painter.rect_filled(
                        Rect::from_min_size(Pos2::ZERO, Vec2::new(arrow_x + 12.0, Layout::STATUS)),
                        4.0,
                        Color32::from_rgb(63, 73, 66),
                    );
                }
                painter
                    .with_clip_rect(Rect::from_min_max(
                        Pos2::ZERO,
                        Pos2::new(arrow_x - 8.0, Layout::STATUS),
                    ))
                    .text(
                        Pos2::new(10.0, Layout::STATUS / 2.0),
                        Align2::LEFT_CENTER,
                        title,
                        theme::bold(ctx, 15.0),
                        Color32::WHITE,
                    );
                painter.text(
                    Pos2::new(layout.width / 2.0, Layout::STATUS / 2.0),
                    Align2::CENTER_CENTER,
                    if model.clock_label.is_empty() {
                        "—:—"
                    } else {
                        &model.clock_label
                    },
                    theme::bold(ctx, 16.0),
                    theme::TEXT,
                );
                system::paint_status(&painter, model, layout.width - 8.0, Layout::STATUS / 2.0);
                let response = ui
                    .interact(
                        activity_button,
                        ui.id().with("activity-menu"),
                        Sense::click(),
                    )
                    .on_hover_text("Activity menu");
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Activity menu")
                });
                if response.clicked() {
                    commands.push(Command::Menu(tarot_core::menu::Action::ToggleActivity));
                }
                {
                    let center = Pos2::new(arrow_x, Layout::STATUS / 2.0);
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            center + Vec2::new(-3.0, -2.0),
                            center + Vec2::new(3.0, -2.0),
                            center + Vec2::new(0.0, 2.0),
                        ],
                        Color32::WHITE,
                        Stroke::NONE,
                    ));
                }
                let status_button = Rect::from_min_size(
                    Pos2::new(layout.width - 104.0, 0.0),
                    Vec2::new(104.0, Layout::STATUS),
                );
                let status =
                    ui.interact(status_button, ui.id().with("system-status"), Sense::click());
                status.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        true,
                        system::status_label(model),
                    )
                });
                if status.clicked() {
                    commands.push(Command::Menu(tarot_core::menu::Action::ToggleDevice));
                }
            }

            match model.view {
                View::Home | View::Overview => {
                    if !modal {
                        just_type::affordance(ui, layout, &mut commands);
                    }
                }
                View::Launcher => {
                    search_focused = ui
                        .scope(|ui| {
                            if modal {
                                ui.disable();
                            }
                            launcher::show(
                                ui,
                                &painter,
                                model,
                                layout,
                                favorite_frame.preview.as_ref(),
                                &paging_frame,
                                &mut commands,
                            )
                        })
                        .inner
                        || (keyboard_interaction && !modal)
                }
                View::App(_) => {}
            }

            if presentation.overview > 0.001 {
                ui.scope(|ui| {
                    if modal {
                        ui.disable();
                    }
                    show_cards(ui, model, layout, presentation, &mut commands);
                });
            }

            if (presentation.dock > 0.001
                && (model.view != View::Launcher || model.editing_favorites))
                || favorite_frame.preview.is_some()
            {
                ui.scope(|ui| {
                    ui.set_opacity(presentation.dock);
                    if matches!(model.view, View::App(_)) || modal {
                        ui.disable();
                    }
                    quick_launch::show(
                        ui,
                        model,
                        layout,
                        None,
                        favorite_frame.preview.as_ref(),
                        &mut commands,
                    );
                });
            }
            let gesture = layout.gesture();
            let strip = Rect::from_min_size(
                Pos2::new(gesture.x, gesture.y),
                Vec2::new(gesture.width, gesture.height),
            );
            painter.rect_filled(strip, 0.0, theme::GESTURE);
            painter.rect_filled(
                Rect::from_center_size(strip.center(), Vec2::new(38.0, 2.0)),
                1.0,
                Color32::from_rgb(83, 108, 112),
            );
            let overview = if matches!(model.view, View::App(_)) {
                let toggle =
                    Rect::from_min_max(Pos2::new(strip.right() - 96.0, strip.top()), strip.max);
                if ui
                    .put(toggle, egui::Button::new("Keyboard").frame(false))
                    .on_hover_text(if model.keyboard_available {
                        "Show or hide the keyboard for the focused field"
                    } else {
                        "No native keyboard is configured"
                    })
                    .clicked()
                    && model.keyboard_available
                {
                    commands.push(Command::ToggleKeyboard);
                }
                Rect::from_min_max(strip.min, Pos2::new(toggle.left(), strip.bottom()))
            } else {
                strip
            };
            let alerts = Rect::from_min_size(strip.min, Vec2::new(80.0, strip.height()));
            notifications::entry(ui, model, alerts, &mut commands);
            let overview = Rect::from_min_max(
                Pos2::new(overview.left() + 80.0, overview.top()),
                overview.max,
            );
            let response = ui
                .interact(overview, ui.id().with("overview"), Sense::click())
                .on_hover_text("Show cards");
            if response.clicked() {
                commands.push(Command::Overview);
            }
        });
    if favorite_frame.pending && model.shell_text_focus {
        ctx.memory_mut(|memory| memory.request_focus(search_id));
        search_focused = true;
    }
    if model.view == View::Launcher
        && model.shell_text_focus
        && favorite_frame.preview.is_none()
        && !modal
    {
        let keyboard_id = egui::Id::new("shell_keyboard_state");
        let mut keyboard = ctx
            .data_mut(|data| data.get_temp::<keyboard::KeyboardUi>(keyboard_id))
            .unwrap_or_default();
        let mut actions = Vec::new();
        egui::Area::new("shell_keyboard".into())
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(keyboard_rect.x, keyboard_rect.y))
            .fade_in(false)
            .movable(false)
            .show(ctx, |ui| {
                ui.set_clip_rect(Rect::from_min_size(
                    Pos2::new(keyboard_rect.x, keyboard_rect.y),
                    Vec2::new(keyboard_rect.width, keyboard_rect.height),
                ));
                ui.set_min_size(Vec2::new(keyboard_rect.width, keyboard_rect.height));
                ui.set_max_size(Vec2::new(keyboard_rect.width, keyboard_rect.height));
                egui::Frame::new()
                    .fill(crate::keyboard::PANEL)
                    .inner_margin(5.0)
                    .show(ui, |ui| {
                        actions = keyboard.show_in_ui(ui, false, false);
                    });
            });
        ctx.data_mut(|data| data.insert_temp(keyboard_id, keyboard));
        let accept_name =
            actions.contains(&keyboard::Action::Enter) && model.launcher.rename.is_some();
        if accept_name {
            commands.push(Command::LauncherAction(
                tarot_core::launcher::Action::AcceptName,
            ));
            search_focused = true;
        }
        let submit_search = actions.contains(&keyboard::Action::Enter) && model.just_type_active();
        if submit_search {
            commands.push(Command::JustType(tarot_core::search::Action::Submit));
        }
        let hide = actions.contains(&keyboard::Action::Hide)
            || (actions.contains(&keyboard::Action::Enter) && !accept_name && !submit_search);
        if hide {
            search_focused = false;
            ctx.memory_mut(|memory| memory.surrender_focus(search_id));
        } else if keyboard_interaction {
            ctx.memory_mut(|memory| memory.request_focus(search_id));
        }
        for action in actions {
            if !matches!(action, keyboard::Action::Hide | keyboard::Action::Enter) {
                commands.push(Command::ShellKey(action));
            }
        }
    } else {
        ctx.data_mut(|data| {
            data.remove::<keyboard::KeyboardUi>(egui::Id::new("shell_keyboard_state"))
        });
    }
    if search_focused != model.shell_text_focus {
        commands.push(Command::ShellTextFocus(search_focused));
    }
    let menu_open =
        model.view == View::Overview && !model.system_menu_open && egui::Popup::is_any_open(ctx);
    if menu_open != model.card_menu_open {
        commands.push(Command::CardMenuOpen(menu_open));
    }
    if !model.system_menu_open {
        notifications::show(ctx, model, layout, &mut commands);
    }
    system::show(ctx, model, layout, &mut commands);
    credentials::keyboard(ctx, model, layout, &mut commands);
    if model.view != View::Launcher {
        quick_launch::launch_feedback(ctx, model, layout, &mut commands);
    }
    if let Some(wave) = presentation.quick_launch
        && !modal
    {
        egui::Area::new("quick_launch_wave".into())
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .movable(false)
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_min_size(full.size());
                quick_launch::show(ui, model, layout, Some(wave), None, &mut commands);
            });
    }
    if let Some(feedback) = &model.volume_feedback {
        audio::show_key_feedback(ctx, feedback, layout);
    }
    favorite_drag::finish(ctx, &favorite_frame, model, layout, &mut commands);
    launcher_paging::finish(&paging_frame, &mut commands);
    commands
}

fn show_cards(
    ui: &mut egui::Ui,
    model: &ShellModel,
    layout: Layout,
    presentation: &Presentation,
    commands: &mut Vec<Command>,
) {
    for visual in &presentation.windows {
        let Some(card) = model.cards.iter().find(|card| card.id == visual.id) else {
            continue;
        };
        if visual.opacity <= 0.001 {
            continue;
        }
        ui.push_id(("card", card.id.0), |ui| {
            let content = layout.content();
            ui.set_clip_rect(ui.clip_rect().intersect(Rect::from_min_size(
                Pos2::new(content.x, content.y),
                Vec2::new(content.width, content.height),
            )));
            ui.set_opacity(presentation.overview * visual.opacity);
            if model.view != View::Overview
                || presentation.arrange.is_some()
                || model.system_menu_open
            {
                ui.disable();
            }
            let painter = ui.painter().clone();
            let geometry = visual.rect;
            let members = model.stack_members(card.id);
            let representative = model
                .selected
                .filter(|id| members.contains(id))
                .or_else(|| members.last().copied());
            let card_rect = Rect::from_min_size(
                Pos2::new(geometry.x, geometry.y),
                Vec2::new(geometry.width, geometry.height),
            );
            if representative == Some(card.id)
                || presentation
                    .arrange
                    .is_some_and(|arrange| arrange.id == card.id)
            {
                let width = (visual.corner_radius / tarot_core::design::CARD_RADIUS).max(0.5);
                let outline = card_rect.expand(width * 0.5);
                let radius = visual.corner_radius + width * 0.5;
                // Float paths retain the native mask's subpixel radius; egui's
                // integer corner-radius rectangle would round it independently.
                let mut points = Vec::with_capacity(36);
                for (center, start) in [
                    (
                        Pos2::new(outline.left() + radius, outline.top() + radius),
                        std::f32::consts::PI,
                    ),
                    (
                        Pos2::new(outline.right() - radius, outline.top() + radius),
                        std::f32::consts::PI * 1.5,
                    ),
                    (
                        Pos2::new(outline.right() - radius, outline.bottom() - radius),
                        0.0,
                    ),
                    (
                        Pos2::new(outline.left() + radius, outline.bottom() - radius),
                        std::f32::consts::FRAC_PI_2,
                    ),
                ] {
                    for step in 0..=8 {
                        let angle = start + step as f32 * std::f32::consts::FRAC_PI_2 / 8.0;
                        let (sin, cos) = angle.sin_cos();
                        points.push(center + Vec2::new(cos * radius, sin * radius));
                    }
                }
                let points = points
                    .into_iter()
                    .map(|p| {
                        let (x, y) = visual.to_output(p.x, p.y);
                        Pos2::new(x, y)
                    })
                    .collect();
                painter.add(egui::Shape::convex_polygon(
                    points,
                    Color32::TRANSPARENT,
                    Stroke::new(width, Color32::from_rgb(218, 227, 215)),
                ));
            }
            let bounds = visual.bounds();
            let hit_rect = Rect::from_min_size(
                Pos2::new(bounds.x, bounds.y),
                Vec2::new(bounds.width, bounds.height),
            );
            let response = ui.interact(hit_rect, ui.id().with("activate"), Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    format!("Open {}", card.title),
                )
            });
            if response.clicked()
                && response
                    .interact_pointer_pos()
                    .is_some_and(|p| visual.contains(p.x, p.y))
            {
                commands.push(Command::Activate(card.id));
            }
            let mut label = if card.closing {
                format!("Closing… {}", card.title)
            } else {
                card.title.clone()
            };
            // Cards carry no buttons: close is the upward flick and the System
            // Menu holds Open, Arrange and Close. A stack's position joins the
            // title below the card.
            if members.len() > 1
                && let Some(index) = members.iter().position(|id| *id == card.id)
            {
                label = format!("{label}  {} / {}", index + 1, members.len());
            }
            if representative == Some(card.id) && presentation.arrange.is_none() {
                painter
                    .with_clip_rect(Rect::from_min_size(
                        Pos2::new(hit_rect.left(), hit_rect.bottom() + 4.0),
                        Vec2::new(hit_rect.width(), 32.0),
                    ))
                    .text(
                        Pos2::new(hit_rect.center().x, hit_rect.bottom() + 20.0),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(14.0),
                        Color32::WHITE,
                    );
            }
        });
    }
    let painter = ui.painter().clone();
    if let Some(arrange) = presentation.arrange {
        let caption = match arrange.target {
            Some(target) => {
                let title = model
                    .cards
                    .iter()
                    .find(|c| c.id == target.anchor())
                    .map_or("card", |c| c.title.as_str());
                match target {
                    CardDrop::Into { .. } => format!("Stack with {title}"),
                    CardDrop::Separate { before, .. } => format!(
                        "Keep separate: {} {title}",
                        if before { "before" } else { "after" }
                    ),
                }
            }
            None => "Drag onto a card to stack; drop below cards to separate".into(),
        };
        ui.put(
            Rect::from_min_size(Pos2::new(12.0, 34.0), Vec2::new(layout.width - 24.0, 48.0)),
            egui::Label::new(
                egui::RichText::new(caption)
                    .color(Color32::WHITE)
                    .size(14.0),
            )
            .wrap(),
        );
        let rail = Rect::from_min_size(
            Pos2::new(8.0, layout.height - Layout::GESTURE - Layout::DOCK - 48.0),
            Vec2::new(layout.width - 16.0, 48.0),
        );
        painter.rect_filled(rail, 8.0, Color32::from_black_alpha(170));
        painter.text(
            rail.center(),
            Align2::CENTER_CENTER,
            "Keep as a separate card",
            FontId::proportional(13.0),
            Color32::LIGHT_GRAY,
        );
        let accent = Color32::from_rgb(135, 210, 245);
        if let Some(target) = arrange.target
            && let Some(window) = presentation.window(target.anchor())
        {
            match target {
                CardDrop::Into { .. } => {
                    painter.rect_stroke(
                        Rect::from_min_size(
                            Pos2::new(window.rect.x, window.rect.y),
                            Vec2::new(window.rect.width, window.rect.height),
                        )
                        .expand(4.0),
                        8.0,
                        Stroke::new(3.0_f32, accent),
                        StrokeKind::Outside,
                    );
                }
                CardDrop::Separate { before, .. } => {
                    let members = model.stack_members(target.anchor());
                    let group: Vec<_> = presentation
                        .windows
                        .iter()
                        .filter(|w| w.id != arrange.id && members.contains(&w.id))
                        .collect();
                    let x = if before {
                        group
                            .iter()
                            .map(|w| w.rect.x)
                            .reduce(f32::min)
                            .unwrap_or(window.rect.x)
                            - 9.0
                    } else {
                        group
                            .iter()
                            .map(|w| w.rect.x + w.rect.width)
                            .reduce(f32::max)
                            .unwrap_or(window.rect.x + window.rect.width)
                            + 9.0
                    };
                    let x = x.clamp(12.0, layout.width - 12.0);
                    painter.line_segment(
                        [
                            Pos2::new(x, rail.top() + 4.0),
                            Pos2::new(x, rail.bottom() - 4.0),
                        ],
                        Stroke::new(4.0_f32, accent),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_launch_has_a_fixed_launcher_and_editable_empty_positions() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        let commands = click(&ctx, &model, Pos2::new(318.0, 650.0));
        assert_eq!(
            commands
                .iter()
                .filter(|c| matches!(c, Command::Launcher))
                .count(),
            1
        );
        for c in commands {
            model.apply(c);
        }
        assert_eq!(model.view, View::Launcher);
        for c in click(&ctx, &model, Pos2::new(40.0, 16.0)) {
            model.apply(c);
        }
        assert!(model.launcher.menu_open);
        for c in click_text(&ctx, &model, "Quick Launch favorites") {
            model.apply(c);
        }
        assert!(model.editing_favorites);
        for c in click(&ctx, &model, Pos2::new(285.0, 67.0)) {
            model.apply(c);
        }
        assert!(!model.editing_favorites);
    }

    #[test]
    fn a_strip_drag_keeps_launcher_search_focus_without_typing() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        open_search(&ctx, &mut model);
        assert!(model.shell_text_focus);
        for c in frame(
            &ctx,
            &model,
            vec![
                egui::Event::PointerMoved(Pos2::new(180.0, 706.0)),
                egui::Event::PointerButton {
                    pos: Pos2::new(180.0, 706.0),
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        ) {
            model.apply(c);
        }
        assert!(model.shell_text_focus);
        let mut canceled = vec![];
        cancel_pointer_input(&mut canceled);
        for c in frame(&ctx, &model, canceled) {
            model.apply(c);
        }
        for c in frame(&ctx, &model, vec![]) {
            model.apply(c);
        }
        assert!(model.shell_text_focus && model.launcher_query.is_empty());
    }

    fn frame(ctx: &egui::Context, model: &ShellModel, events: Vec<egui::Event>) -> Vec<Command> {
        prepare_pointer_input(ctx, &events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 720.0))),
            events,
            focused: true,
            ..Default::default()
        };
        let mut commands = Vec::new();
        let _output = ctx.run(input, |ctx| {
            commands = show(
                ctx,
                model,
                Layout {
                    width: 360.0,
                    height: 720.0,
                    notification_height: 0.0,
                    keyboard_height: if model.credential_keyboard_visible()
                        || model.shell_text_focus && model.view == View::Launcher
                    {
                        240.0
                    } else {
                        0.0
                    },
                },
            );
        });
        commands
    }

    // Locate visible text after fonts and layout have settled. The control's
    // actual painted position must remain tappable with the packet typography.
    fn text_position(ctx: &egui::Context, model: &ShellModel, label: &str) -> Pos2 {
        fn find(shape: &egui::epaint::Shape, label: &str) -> Option<Pos2> {
            match shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == label => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(|s| find(s, label)),
                _ => None,
            }
        }
        let mut target = None;
        for _ in 0..3 {
            prepare_pointer_input(ctx, &[]);
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 720.0))),
                    ..Default::default()
                },
                |ctx| {
                    let _ = show(
                        ctx,
                        model,
                        Layout {
                            width: 360.0,
                            height: 720.0,
                            notification_height: 0.0,
                            keyboard_height: 0.0,
                        },
                    );
                },
            );
            target = output.shapes.iter().find_map(|s| find(&s.shape, label));
        }
        target.unwrap_or_else(|| panic!("control text {label:?} must be visible"))
    }

    fn click_text(ctx: &egui::Context, model: &ShellModel, label: &str) -> Vec<Command> {
        click(ctx, model, text_position(ctx, model, label))
    }

    fn open_search(ctx: &egui::Context, model: &mut ShellModel) {
        for command in click(ctx, model, Pos2::new(40.0, 16.0)) {
            model.apply(command);
        }
        assert!(model.launcher.menu_open);
        for command in click_text(ctx, model, "Search applications") {
            model.apply(command);
        }
        assert!(model.launcher.search_open && model.shell_text_focus);
    }

    #[test]
    fn launcher_menu_creates_and_renames_pages_through_the_real_text_field() {
        use tarot_core::launcher::{Action, Edit};
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        let favorites = model.favorites.clone();
        for c in click(&ctx, &model, Pos2::new(40.0, 16.0)) {
            model.apply(c);
        }
        for c in click_text(&ctx, &model, "New page") {
            model.apply(c);
        }
        assert!(model.launcher.rename.is_some() && model.shell_text_focus);
        frame(&ctx, &model, vec![]);
        for c in frame(&ctx, &model, vec![egui::Event::Text("Work".into())]) {
            model.apply(c);
        }
        assert_eq!(model.launcher.rename.as_ref().unwrap().text, "Work");
        let mut effects = vec![];
        for c in click(&ctx, &model, Pos2::new(260.0, 186.0)) {
            effects.extend(model.apply(c));
        }
        assert!(
            matches!(effects.as_slice(), [tarot_core::Effect::Launcher(_)]),
            "{effects:?}"
        );
        let page = model.launcher.settings.selected;
        assert_eq!(model.launcher.settings.page(page).unwrap().name, "Work");
        assert!(model.launcher.rename.is_none() && !model.shell_text_focus);
        let saved = model.launcher.settings.clone();
        model.apply(Command::LauncherAction(Action::BeginName(Some(page))));
        frame(&ctx, &model, vec![]);
        frame(&ctx, &model, vec![]);
        for c in frame(&ctx, &model, vec![egui::Event::Text("Personal".into())]) {
            model.apply(c);
        }
        assert_eq!(model.launcher.rename.as_ref().unwrap().text, "Personal");
        for c in click(&ctx, &model, Pos2::new(90.0, 186.0)) {
            model.apply(c);
        }
        assert_eq!(model.launcher.settings, saved);
        model.apply(Command::LauncherAction(Action::BeginName(Some(page))));
        frame(&ctx, &model, vec![]);
        frame(&ctx, &model, vec![]);
        for c in frame(&ctx, &model, vec![egui::Event::Text("Personal".into())]) {
            model.apply(c);
        }
        for c in frame(
            &ctx,
            &model,
            vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        ) {
            model.apply(c);
        }
        assert_eq!(model.launcher.settings.page(page).unwrap().name, "Personal");
        assert_eq!(model.favorites, favorites);
        assert!(
            model
                .apply(Command::LauncherAction(Action::Edit(Edit::Select(page))))
                .is_empty()
        );
    }

    #[test]
    fn reopening_a_launcher_menu_after_page_creation_expands_its_visible_viewport() {
        use tarot_core::launcher::{Action, Edit};
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        model.apply(Command::LauncherAction(Action::Menu(true)));
        for _ in 0..3 {
            frame(&ctx, &model, vec![]);
        }
        model.apply(Command::LauncherAction(Action::Edit(Edit::Create(
            "Work".into(),
        ))));
        model.apply(Command::LauncherAction(Action::Menu(true)));
        for c in click_text(&ctx, &model, "Launcher pages") {
            model.apply(c);
        }
        let mut visible = false;
        for _ in 0..3 {
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 720.0))),
                    ..Default::default()
                },
                |ctx| {
                    show(
                        ctx,
                        &model,
                        Layout {
                            width: 360.0,
                            height: 720.0,
                            notification_height: 0.0,
                            keyboard_height: 0.0,
                        },
                    );
                },
            );
            visible = output.shapes.iter().any(|clipped| match &clipped.shape {
                egui::Shape::Text(text) if text.galley.job.text == "Work" => clipped
                    .clip_rect
                    .contains(text.pos + text.galley.size() * 0.5),
                _ => false,
            });
        }
        assert!(
            visible,
            "The new page must be painted inside the menu scroll viewport"
        );
    }

    #[test]
    fn calendar_art_uses_the_observed_local_day_in_dock_and_launcher() {
        fn has_text(shape: &egui::epaint::Shape, label: &str) -> bool {
            match shape {
                egui::epaint::Shape::Text(text) => text.galley.job.text == label,
                egui::epaint::Shape::Vec(shapes) => shapes.iter().any(|s| has_text(s, label)),
                _ => false,
            }
        }
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.applications.push(tarot_core::Application {
            id: "org.gnome.Calendar.desktop".into(),
            name: "Calendar".into(),
            description: String::new(),
            icon: None,
            keywords: vec![],
            unavailable: None,
        });
        model
            .favorites
            .applications
            .push("org.gnome.Calendar.desktop".into());
        for view in [View::Home, View::Launcher] {
            model.view = view;
            for day in [Some(1), Some(28), Some(31), None, Some(0), Some(32)] {
                model.calendar_day = day;
                let mut output = egui::FullOutput::default();
                for _ in 0..3 {
                    prepare_pointer_input(&ctx, &[]);
                    output = ctx.run(
                        egui::RawInput {
                            screen_rect: Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(360.0, 720.0),
                            )),
                            ..Default::default()
                        },
                        |ctx| {
                            let _ = show(
                                ctx,
                                &model,
                                Layout {
                                    width: 360.0,
                                    height: 720.0,
                                    notification_height: 0.0,
                                    keyboard_height: 0.0,
                                },
                            );
                        },
                    );
                }
                for digit in 0..=32 {
                    let present = output
                        .shapes
                        .iter()
                        .any(|s| has_text(&s.shape, &digit.to_string()));
                    assert_eq!(
                        present,
                        day == Some(digit) && (1..=31).contains(&digit),
                        "{view:?}, observed day {day:?}, painted date {digit}"
                    );
                }
            }
        }
    }

    #[test]
    fn credential_keyboard_types_only_into_private_entry_and_cancel_drops_it() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        model.launcher_query = "untouched".into();
        model.sync_network(tarot_core::network::Snapshot {
            available: true,
            secret_prompt: Some(tarot_core::network::SecretPrompt {
                id: 1,
                label: "Test".into(),
                kind: tarot_core::network::PasswordKind::Psk,
            }),
            ..Default::default()
        });
        for command in click(&ctx, &model, Pos2::new(21.0, 504.0)) {
            model.apply(command);
        }
        assert_eq!(model.network_input.as_ref().unwrap().password.expose(), "q");
        assert_eq!(model.launcher_query, "untouched");
        let effects = model.apply(Command::CredentialCancel);
        assert!(matches!(
            &effects[..],
            [tarot_core::Effect::Network(
                tarot_core::network::UiAction::CancelSecrets { request: 1 }
            )]
        ));
        assert!(model.network_input.is_none() && !model.credential_keyboard_visible());
    }

    #[test]
    fn private_touch_keyboard_can_enter_every_printable_ascii_character() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.sync_network(tarot_core::network::Snapshot {
            available: true,
            secret_prompt: Some(tarot_core::network::SecretPrompt {
                id: 1,
                label: "Test".into(),
                kind: tarot_core::network::PasswordKind::Sae,
            }),
            ..Default::default()
        });
        fn tap(ctx: &egui::Context, model: &mut ShellModel, x: f32, y: f32) {
            for command in click(ctx, model, Pos2::new(x, y)) {
                model.apply(command);
            }
        }
        let symbols = ["1234567890", "@#€%&*()-", "é🙂?!/_."];
        let more = ["`~^|\\[]{}$", "+=<>:;'\",", "É🙂?!/_."];
        let letters = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
        let mut page = 0;
        for byte in 0x20u8..=0x7e {
            let ch = char::from(byte);
            if ch == ' ' {
                tap(&ctx, &mut model, 140.0, 663.0);
                continue;
            }
            let wanted = if ch.is_ascii_alphabetic() {
                0
            } else if symbols.iter().any(|r| r.contains(ch)) {
                1
            } else {
                2
            };
            if page == 0 && wanted != 0 {
                tap(&ctx, &mut model, 30.0, 663.0);
                page = 1;
            }
            if page != 0 && wanted == 0 {
                tap(&ctx, &mut model, 30.0, 663.0);
                page = 0;
            }
            if page != wanted {
                tap(&ctx, &mut model, 28.0, 609.0);
                page = wanted;
            }
            if ch.is_ascii_uppercase() {
                tap(&ctx, &mut model, 28.0, 609.0);
            }
            let rows = match page {
                0 => letters,
                1 => symbols,
                _ => more,
            };
            let lower = ch.to_ascii_lowercase();
            let (row, column) = rows
                .iter()
                .enumerate()
                .find_map(|(row, text)| {
                    text.chars()
                        .position(|c| c == lower)
                        .map(|column| (row, column))
                })
                .unwrap();
            let x = if row < 2 {
                let n = rows[row].chars().count() as f32;
                let w = (350.0 - 4.0 * (n - 1.0)) / n;
                5.0 + w / 2.0 + column as f32 * (w + 4.0)
            } else {
                69.21429 + column as f32 * 36.92857
            };
            tap(&ctx, &mut model, x, [504.0, 557.0, 609.0][row]);
        }
        let expected: String = (0x20u8..=0x7e).map(char::from).collect();
        assert_eq!(
            model.network_input.as_ref().unwrap().password.expose(),
            expected
        );
    }

    fn click(ctx: &egui::Context, model: &ShellModel, position: Pos2) -> Vec<Command> {
        // Area's initial sizing frame disables interaction. Settle the layout
        // and enabled widget geometry before injecting an actual press.
        frame(ctx, model, Vec::new());
        frame(ctx, model, Vec::new());
        frame(
            ctx,
            model,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            model,
            vec![egui::Event::PointerButton {
                pos: position,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        )
    }

    #[test]
    fn launcher_keyboard_keeps_text_focus_and_can_hide_and_reopen() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        open_search(&ctx, &mut model);
        assert!(model.shell_text_focus && model.launcher.search_open);
        let keys = click(&ctx, &model, Pos2::new(21.0, 504.0));
        assert_eq!(
            keys,
            vec![Command::ShellKey(tarot_core::KeyboardAction::Text(
                "q".into()
            ))]
        );
        let typed = frame(&ctx, &model, vec![egui::Event::Text("q".into())]);
        assert!(
            typed.contains(&Command::SearchApplications("q".into())),
            "{typed:?}"
        );
        model.apply(Command::SearchApplications("q".into()));
        let hidden = click(&ctx, &model, Pos2::new(320.0, 663.0));
        assert!(
            hidden.contains(&Command::ShellTextFocus(false)),
            "{hidden:?}"
        );
        model.apply(Command::ShellTextFocus(false));
        assert!(!frame(&ctx, &model, Vec::new()).contains(&Command::ShellTextFocus(true)));
        let reopened = click(&ctx, &model, Pos2::new(100.0, 64.0));
        assert!(reopened.contains(&Command::ShellTextFocus(true)));
        for command in reopened {
            model.apply(command);
        }
        let typed = frame(&ctx, &model, vec![egui::Event::Text("calc".into())]);
        assert!(
            typed.contains(&Command::SearchApplications("qcalc".into())),
            "{typed:?}"
        );
        model.apply(Command::SearchApplications("qcalc".into()));
        let _ = click(&ctx, &model, Pos2::new(320.0, 663.0));
        model.apply(Command::ShellTextFocus(false));
        let _ = click(&ctx, &model, Pos2::new(100.0, 115.0));
        model.apply(Command::ShellTextFocus(true));
        let edited = frame(
            &ctx,
            &model,
            vec![egui::Event::Key {
                key: egui::Key::Backspace,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert!(
            edited.contains(&Command::SearchApplications("qcal".into())),
            "{edited:?}"
        );
    }

    #[test]
    fn cancelling_a_shell_key_preserves_search_focus_without_typing() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        let _ = click(&ctx, &model, Pos2::new(100.0, 115.0));
        model.apply(Command::ShellTextFocus(true));
        let _ = frame(&ctx, &model, Vec::new());
        let _ = frame(&ctx, &model, Vec::new());
        let mut events = vec![
            egui::Event::PointerMoved(Pos2::new(21.0, 504.0)),
            egui::Event::PointerButton {
                pos: Pos2::new(21.0, 504.0),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        cancel_pointer_input(&mut events);
        let commands = frame(&ctx, &model, events);
        assert!(
            !commands.iter().any(|command| matches!(
                command,
                Command::ShellKey(_) | Command::ShellTextFocus(false)
            )),
            "{commands:?}"
        );
        assert!(
            frame(&ctx, &model, vec![egui::Event::Text("z".into())])
                .contains(&Command::SearchApplications("z".into()))
        );
    }

    #[test]
    fn application_launcher_button_emits_one_command() {
        let ctx = egui::Context::default();
        let model = ShellModel::default();
        assert_eq!(
            click(&ctx, &model, Pos2::new(292.0, 648.0)),
            vec![Command::Launcher]
        );
    }

    #[test]
    fn a_system_page_cannot_type_into_the_launcher_search_behind_it() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        let _ = click(&ctx, &model, Pos2::new(100.0, 115.0));
        model.apply(Command::ShellTextFocus(true));
        model.apply(Command::SystemMenuOpen(true));
        model.apply(Command::SystemPage(tarot_core::SystemPage::Wifi));
        let commands = frame(&ctx, &model, vec![egui::Event::Text("private".into())]);
        assert!(
            !commands.iter().any(|c| matches!(
                c,
                Command::SearchApplications(_) | Command::ShellTextFocus(true)
            )),
            "{commands:?}"
        );
        model.apply(Command::ShellTextFocus(true));
        assert!(!model.shell_text_focus);
    }

    #[test]
    fn wifi_buttons_keep_saved_identity_and_respect_hardware_and_permissions() {
        use tarot_core::network::{Network, Permission, Profile, Security, UiAction};
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::SystemMenuOpen(true));
        model.apply(Command::SystemPage(tarot_core::SystemPage::Wifi));
        model.network.enabled = true;
        model.network.available = true;
        model.network.radio = true;
        model.network.hardware = true;
        model.network.networks.push(Network {
            id: 7,
            label: "Lab".into(),
            strength: 90,
            security: Security::Personal,
            connected: false,
            profiles: vec![Profile {
                id: 8,
                label: "Saved".into(),
            }],
            password_kind: Some(tarot_core::network::PasswordKind::Psk),
        });
        assert!(click_text(&ctx, &model, "Connect saved network").is_empty());
        model.network.control_permission = Permission::Yes;
        assert_eq!(
            click_text(&ctx, &model, "Connect saved network"),
            vec![Command::Network(UiAction::Connect {
                network: 7,
                profile: 8
            })]
        );
        model.network.radio_permission = Permission::Yes;
        model.network.radio = false;
        model.network.hardware = false;
        assert!(click_text(&ctx, &model, "Wi-Fi enabled").is_empty());
        model.network.radio = true;
        assert_eq!(
            click_text(&ctx, &model, "Wi-Fi enabled"),
            vec![Command::Network(UiAction::Radio { enabled: false })]
        );
    }

    #[test]
    fn scrim_dismissal_cannot_cover_controls_when_a_system_panel_reopens() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.power.enabled = true;
        model.power.battery = tarot_core::power::Battery {
            available: true,
            present: true,
            percent: Some(44),
            external_power: Some(true),
            ..Default::default()
        };
        model.power.brightness.maximum = 100;
        model.power.brightness.actual = Some(30);
        model.power.brightness.can_set = true;
        model.apply(Command::SystemMenuOpen(true));
        model.apply(Command::SystemPage(tarot_core::SystemPage::Audio));
        assert_eq!(
            click(&ctx, &model, Pos2::new(21.0, 504.0)),
            vec![Command::Menu(tarot_core::menu::Action::Dismiss)]
        );
        model.apply(Command::SystemMenuOpen(false));
        model.apply(Command::SystemPage(tarot_core::SystemPage::Power));
        for command in click(&ctx, &model, Pos2::new(310.0, 14.0)) {
            model.apply(command);
        }
        assert!(model.system_menu_open);
        for command in click_text(&ctx, &model, "Power & display") {
            model.apply(command);
        }
        assert_eq!(model.system_page, tarot_core::SystemPage::Power);
        assert_eq!(
            click_text(&ctx, &model, "40%"),
            vec![Command::Power(tarot_core::power::UiAction::Brightness {
                percent: 40
            })]
        );

        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.notifications.available = true;
        model.notifications.settings_available = true;
        model.apply(Command::NotificationsOpen(true));
        assert!(click(&ctx, &model, Pos2::new(21.0, 100.0)).is_empty());
        assert_eq!(
            click(&ctx, &model, Pos2::new(40.0, 706.0)),
            vec![Command::NotificationsOpen(false)]
        );
        model.apply(Command::NotificationsOpen(false));
        for command in click(&ctx, &model, Pos2::new(40.0, 706.0)) {
            model.apply(command);
        }
        assert!(model.notifications_open);
        assert!(click_text(&ctx, &model, "Settings").is_empty());
        assert_eq!(
            click_text(&ctx, &model, "Do not disturb"),
            vec![Command::Notification(
                tarot_core::notifications::UiAction::Settings {
                    dnd: true,
                    hide_previews: false
                }
            )]
        );
    }

    #[test]
    fn audio_touch_actions_keep_identity_and_wait_for_confirmation() {
        use tarot_core::audio::{Output, UiAction};
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::SystemMenuOpen(true));
        model.apply(Command::SystemPage(tarot_core::SystemPage::Audio));
        model.audio.enabled = true;
        model.audio.available = true;
        model.audio.outputs.push(Output {
            ports: Vec::new(),
            id: 9,
            label: "Speaker".into(),
            percent: 40,
            muted: false,
            default: true,
        });
        assert_eq!(
            click_text(&ctx, &model, "−"),
            vec![Command::Audio(UiAction::Volume {
                output: 9,
                percent: 30
            })]
        );
        assert_eq!(
            click_text(&ctx, &model, "+"),
            vec![Command::Audio(UiAction::Volume {
                output: 9,
                percent: 50
            })]
        );
        assert_eq!(
            click_text(&ctx, &model, "Mute"),
            vec![Command::Audio(UiAction::Mute {
                output: 9,
                muted: true
            })]
        );
        model.audio.pending = Some(UiAction::Mute {
            output: 9,
            muted: true,
        });
        assert!(click_text(&ctx, &model, "Mute").is_empty());
        model.audio.pending = None;
        model.audio.outputs.push(Output {
            ports: Vec::new(),
            id: 10,
            label: "JLab Studio".into(),
            percent: 100,
            muted: false,
            default: false,
        });
        let speaker = text_position(&ctx, &model, "Speaker");
        assert!(
            click_text(&ctx, &model, "Speaker").is_empty(),
            "the chosen output stays chosen"
        );
        assert_eq!(
            click_text(&ctx, &model, "JLab Studio"),
            vec![Command::Audio(UiAction::Default { output: 10 })]
        );
        model.audio.outputs[0].default = false;
        model.audio.outputs[1].default = true;
        assert_eq!(
            text_position(&ctx, &model, "Speaker"),
            speaker,
            "choosing another output must not move the list"
        );
        assert_eq!(
            click_text(&ctx, &model, "Speaker"),
            vec![Command::Audio(UiAction::Default { output: 9 })]
        );
        model.audio.available = false;
        assert!(click_text(&ctx, &model, "+").is_empty());
    }

    #[test]
    fn recording_controls_keep_input_and_route_identity_and_disable_missing_routes() {
        use tarot_core::audio::{Availability, Input, Port, UiAction};
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::SystemMenuOpen(true));
        model.apply(Command::SystemPage(tarot_core::SystemPage::Audio));
        model.audio.enabled = true;
        model.audio.available = true;
        model.audio.inputs.push(Input {
            id: 31,
            label: "Built-in microphone".into(),
            percent: 40,
            muted: false,
            default: false,
            ports: vec![
                Port {
                    id: 41,
                    label: "Built-in route".into(),
                    active: false,
                    available: Availability::Unknown,
                },
                Port {
                    id: 42,
                    label: "Missing route".into(),
                    active: false,
                    available: Availability::No,
                },
            ],
        });
        assert_eq!(
            click_text(&ctx, &model, "+"),
            vec![Command::Audio(UiAction::InputVolume {
                input: 31,
                percent: 50
            })]
        );
        assert_eq!(
            click_text(&ctx, &model, "Mute"),
            vec![Command::Audio(UiAction::InputMute {
                input: 31,
                muted: true
            })]
        );
        // The first painted match is the "Record from" radio row.
        assert_eq!(
            click_text(&ctx, &model, "Built-in microphone"),
            vec![Command::Audio(UiAction::DefaultInput { input: 31 })]
        );
        assert_eq!(
            click_text(&ctx, &model, "Built-in route"),
            vec![Command::Audio(UiAction::InputPort {
                input: 31,
                port: 41
            })]
        );
        assert!(click_text(&ctx, &model, "Missing route · Unavailable").is_empty());
        model.audio.pending = Some(UiAction::InputMute {
            input: 31,
            muted: true,
        });
        assert!(click_text(&ctx, &model, "Built-in route").is_empty());
    }

    #[test]
    fn a_visible_status_menu_lock_button_emits_only_a_lock_request() {
        fn position(shape: &egui::epaint::Shape) -> Option<Pos2> {
            match shape {
                egui::epaint::Shape::Text(text) if text.galley.job.text == "Lock" => {
                    Some(text.pos + text.galley.size() * 0.5)
                }
                egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(position),
                _ => None,
            }
        }
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.add_window(tarot_core::WindowId(7), "App".into(), "app".into());
        model.power.enabled = true;
        model.power.battery = tarot_core::power::Battery {
            available: true,
            present: true,
            percent: Some(24),
            state: tarot_core::power::ChargeState::Discharging,
            external_power: Some(true),
            ..Default::default()
        };
        model.power.brightness.maximum = 51;
        model.power.brightness.actual = Some(15);
        model.power.brightness.can_set = true;
        model.lock_available = true;
        model.apply(Command::SystemMenuOpen(true));
        let mut target = None;
        for _ in 0..3 {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 720.0))),
                ..Default::default()
            };
            let output = ctx.run(input, |ctx| {
                let _ = show(
                    ctx,
                    &model,
                    Layout {
                        width: 360.0,
                        height: 720.0,
                        notification_height: 0.0,
                        keyboard_height: 0.0,
                    },
                );
            });
            target = output.shapes.iter().find_map(|s| position(&s.shape));
        }
        let target = target.expect("configured lock control is visible");
        eprintln!("Phone-layout Lock button center: {target:?}");
        assert_eq!(click(&ctx, &model, target), vec![Command::Lock]);
        assert_eq!(model.view, View::App(tarot_core::WindowId(7)));
        assert_eq!(model.cards.len(), 1);
        model.lock_available = false;
        assert!(!click(&ctx, &model, target).contains(&Command::Lock));
    }

    #[test]
    fn status_menu_has_touch_controls_and_blocks_underlying_navigation() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        assert_eq!(
            click(&ctx, &model, Pos2::new(310.0, 14.0)),
            vec![Command::Menu(tarot_core::menu::Action::ToggleDevice)]
        );
        model.apply(Command::Menu(tarot_core::menu::Action::ToggleDevice));
        model.power.enabled = true;
        model.power.battery = tarot_core::power::Battery {
            available: true,
            present: true,
            percent: Some(44),
            state: tarot_core::power::ChargeState::Discharging,
            external_power: Some(true),
            ..Default::default()
        };
        model.power.brightness.maximum = 100;
        model.power.brightness.actual = Some(30);
        for c in click_text(&ctx, &model, "Power & display") {
            model.apply(c);
        }
        assert!(click_text(&ctx, &model, "40%").is_empty());
        model.power.brightness.can_set = true;
        assert_eq!(
            click_text(&ctx, &model, "40%"),
            vec![Command::Power(tarot_core::power::UiAction::Brightness {
                percent: 40
            })]
        );
        assert_eq!(
            click(&ctx, &model, Pos2::new(292.0, 648.0)),
            vec![Command::Menu(tarot_core::menu::Action::Dismiss)]
        );
    }

    #[test]
    fn dashboard_allows_search_navigation_keyboard_and_exposed_cards() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::NotificationsOpen(true));
        let typed = frame(&ctx, &model, vec![egui::Event::Text("v".into())]);
        assert!(typed.contains(&Command::JustType(tarot_core::search::Action::Open)));
        for command in typed {
            model.apply(command);
        }
        assert!(!model.notifications_open && model.just_type_active());
        frame(&ctx, &model, vec![]);
        assert!(
            click(&ctx, &model, Pos2::new(21.0, 504.0)).contains(&Command::ShellKey(
                tarot_core::KeyboardAction::Text("q".into())
            ))
        );
        assert!(!model.notifications_open);
        model.apply(Command::NotificationsOpen(true));
        model.add_window_in_background(tarot_core::WindowId(7), "Example".into(), "test".into());
        model.view = View::Overview;
        model.selected = Some(tarot_core::WindowId(7));
        let layout = tarot_core::Layout {
            width: 360.0,
            height: 720.0,
            keyboard_height: 0.0,
            notification_height: 0.0,
        }
        .with_notifications(&model);
        let visual = tarot_core::motion::Presentation::settled(&model, layout);
        let rect = visual
            .windows
            .iter()
            .find(|w| w.id == tarot_core::WindowId(7))
            .unwrap()
            .rect;
        let commands = click(
            &ctx,
            &model,
            Pos2::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0),
        );
        assert!(
            commands.contains(&Command::Activate(tarot_core::WindowId(7))),
            "{commands:?}"
        );
    }

    #[test]
    fn notification_dashboard_reserves_its_well_and_keeps_touch_settings() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        assert_eq!(
            click(&ctx, &model, Pos2::new(40.0, 706.0)),
            vec![Command::NotificationsOpen(true)]
        );
        model.apply(Command::NotificationsOpen(true));
        model.notifications.available = true;
        assert!(click_text(&ctx, &model, "Settings").is_empty());
        assert!(click_text(&ctx, &model, "Do not disturb").is_empty());
        model.notifications.settings_available = true;
        assert_eq!(
            click_text(&ctx, &model, "Do not disturb"),
            vec![Command::Notification(
                tarot_core::notifications::UiAction::Settings {
                    dnd: true,
                    hide_previews: false
                }
            )]
        );
        assert!(click(&ctx, &model, Pos2::new(160.0, 100.0)).is_empty());
        assert_eq!(
            click(&ctx, &model, Pos2::new(40.0, 706.0)),
            vec![Command::NotificationsOpen(false)]
        );
    }

    #[test]
    fn now_playing_controls_offer_only_supported_actions_and_one_request() {
        use tarot_core::media::{Action, Control, Player, Snapshot, Status};
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::NotificationsOpen(true));
        model.notifications.available = true;
        model.media = Snapshot {
            enabled: true,
            available: true,
            generation: 7,
            players: vec![Player {
                id: 3,
                identity: "Fixture Player".into(),
                title: "Morning Song".into(),
                artist: "A".into(),
                status: Status::Paused,
                can_play: true,
                can_pause: true,
                can_go_next: true,
                can_go_previous: false,
            }],
            ..Default::default()
        };
        text_position(&ctx, &model, "Morning Song");
        // The Play glyph is the only closed four-point stroke; buttons are
        // laid out in one row, so the others sit one button width either side.
        let play_center = glyph_center(&ctx, &model);
        let spacing = 64.0 + ctx.style().spacing.item_spacing.x;
        let button = |index: f32| Pos2::new(play_center.x + (index - 1.0) * spacing, play_center.y);
        assert!(
            click(&ctx, &model, button(0.0)).is_empty(),
            "no previous capability"
        );
        let play = click(&ctx, &model, button(1.0));
        assert_eq!(
            play,
            vec![Command::Media(Control {
                player: 3,
                generation: 7,
                action: Action::PlayPause
            })]
        );
        for command in play {
            model.apply(command);
        }
        assert!(model.media.pending.is_some());
        assert!(
            click(&ctx, &model, button(2.0)).is_empty(),
            "pending blocks a second request"
        );
        model.media.pending = None;
        model.media.generation = 8;
        assert_eq!(
            click(&ctx, &model, button(2.0)),
            vec![Command::Media(Control {
                player: 3,
                generation: 8,
                action: Action::Next
            })]
        );
        model.media.players.clear();
        let output = settle_text(&ctx, &model);
        assert!(!output.contains(&"Fixture Player".to_owned()));
    }

    fn glyph_center(ctx: &egui::Context, model: &ShellModel) -> Pos2 {
        fn find(shape: &egui::epaint::Shape) -> Option<Pos2> {
            match shape {
                egui::epaint::Shape::Path(path)
                    if path.points.len() == 4 && path.points[0] == path.points[3] =>
                {
                    let rect = Rect::from_points(&path.points);
                    // The Play triangle's bounding box is 12 x 16 points.
                    ((rect.width() - 12.0).abs() < 0.5 && (rect.height() - 16.0).abs() < 0.5)
                        .then(|| rect.center() + Vec2::new(-1.0, 0.0))
                }
                egui::epaint::Shape::Vec(shapes) => shapes.iter().find_map(find),
                _ => None,
            }
        }
        let mut found = None;
        for _ in 0..3 {
            prepare_pointer_input(ctx, &[]);
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 720.0))),
                    ..Default::default()
                },
                |ctx| {
                    let _ = show(
                        ctx,
                        model,
                        Layout {
                            width: 360.0,
                            height: 720.0,
                            keyboard_height: 0.0,
                            notification_height: 0.0,
                        }
                        .with_notifications(model),
                    );
                },
            );
            // Only a glyph inside its clip rectangle can be touched.
            found = output
                .shapes
                .iter()
                .find_map(|c| find(&c.shape).filter(|p| c.clip_rect.contains(*p)));
        }
        found.expect("the Play glyph must be painted")
    }

    fn settle_text(ctx: &egui::Context, model: &ShellModel) -> Vec<String> {
        fn collect(shape: &egui::epaint::Shape, out: &mut Vec<String>) {
            match shape {
                egui::epaint::Shape::Text(text) => out.push(text.galley.job.text.clone()),
                egui::epaint::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect(s, out)),
                _ => {}
            }
        }
        let mut found = Vec::new();
        for _ in 0..3 {
            prepare_pointer_input(ctx, &[]);
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 720.0))),
                    ..Default::default()
                },
                |ctx| {
                    let _ = show(
                        ctx,
                        model,
                        Layout {
                            width: 360.0,
                            height: 720.0,
                            keyboard_height: 0.0,
                            notification_height: 0.0,
                        }
                        .with_notifications(model),
                    );
                },
            );
            found.clear();
            for clipped in &output.shapes {
                collect(&clipped.shape, &mut found);
            }
        }
        found
    }

    #[test]
    fn gesture_strip_has_a_tappable_overview_alternative_in_app_view() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.add_window(tarot_core::WindowId(7), "Example".into(), "test".into());
        assert_eq!(
            click(&ctx, &model, Pos2::new(180.0, 706.0)),
            vec![Command::Overview]
        );
    }

    #[test]
    fn installed_application_tile_emits_its_stable_desktop_id() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        model.applications.push(tarot_core::Application {
            id: "example.desktop".into(),
            name: "Example".into(),
            description: String::new(),
            icon: None,
            keywords: Vec::new(),
            unavailable: None,
        });
        assert_eq!(
            click(&ctx, &model, Pos2::new(64.0, 188.0)),
            vec![Command::Launch("example.desktop".into())]
        );
    }

    #[test]
    fn launcher_search_matches_all_words_across_keywords_and_description() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        model.applications = vec![
            tarot_core::Application {
                id: "notes.desktop".into(),
                name: "Math Notes".into(),
                description: "Write notes".into(),
                icon: None,
                keywords: Vec::new(),
                unavailable: None,
            },
            tarot_core::Application {
                id: "calc.desktop".into(),
                name: "Calculator".into(),
                description: "Work with numbers".into(),
                icon: None,
                keywords: vec!["Math".into()],
                unavailable: None,
            },
        ];
        model.apply(Command::SearchApplications("  MATH   numbers  ".into()));
        let commands = click(&ctx, &model, Pos2::new(64.0, 188.0));
        let effects: Vec<_> = commands.into_iter().flat_map(|c| model.apply(c)).collect();
        assert_eq!(
            effects,
            vec![tarot_core::Effect::Launch("calc.desktop".into())]
        );
    }

    #[test]
    fn just_type_enters_from_cards_and_opens_a_real_setting_without_changing_the_query() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.add_window(
            tarot_core::WindowId(1),
            "Original".into(),
            "original".into(),
        );
        model.apply(Command::Overview);
        for c in click(&ctx, &model, Pos2::new(120.0, 60.0)) {
            model.apply(c);
        }
        assert!(model.just_type_active());
        assert_eq!(model.search.origin, Some(View::Overview));
        for c in frame(&ctx, &model, vec![egui::Event::Text("volume".into())]) {
            model.apply(c);
        }
        assert_eq!(model.launcher_query, "volume");
        for c in click(&ctx, &model, Pos2::new(180.0, 145.0)) {
            model.apply(c);
        }
        assert!(model.system_menu_open);
        assert_eq!(model.system_page, tarot_core::SystemPage::Audio);
        assert_eq!(model.launcher_query, "volume");
        model.apply(Command::SystemMenuOpen(false));
        for c in click(&ctx, &model, Pos2::new(40.0, 16.0)) {
            model.apply(c);
        }
        for c in click_text(&ctx, &model, "Close Just Type") {
            model.apply(c);
        }
        assert_eq!(model.view, View::Overview);
        assert_eq!(model.cards.len(), 1);
        assert!(model.launcher_query.is_empty());
    }

    #[test]
    fn a_real_held_result_cannot_activate_a_replacement_query_result() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::JustType(tarot_core::search::Action::Open));
        model.apply(Command::SearchApplications("rotation".into()));
        for _ in 0..3 {
            frame(&ctx, &model, vec![]);
        }
        let pos = Pos2::new(180.0, 145.0);
        let down = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        for c in frame(&ctx, &model, down) {
            model.apply(c);
        }
        model.apply(Command::SearchApplications("volume".into()));
        frame(&ctx, &model, vec![]);
        let up = vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }];
        let commands = frame(&ctx, &model, up);
        assert!(!commands.iter().any(|c| matches!(
            c,
            Command::JustType(tarot_core::search::Action::Choose { .. })
        )));
        assert!(!model.system_menu_open);
        for c in click(&ctx, &model, pos) {
            model.apply(c);
        }
        assert!(model.system_menu_open && model.system_page == tarot_core::SystemPage::Audio);
    }

    #[test]
    fn cancelled_press_neither_clicks_nor_sticks_and_next_tap_works() {
        for same_frame in [false, true] {
            let ctx = egui::Context::default();
            let model = ShellModel::default();
            frame(&ctx, &model, Vec::new());
            frame(&ctx, &model, Vec::new());
            let pos = Pos2::new(292.0, 648.0);
            let mut events = vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ];
            if !same_frame {
                assert!(frame(&ctx, &model, std::mem::take(&mut events)).is_empty());
            }
            cancel_pointer_input(&mut events);
            assert!(frame(&ctx, &model, events).is_empty());
            assert!(!ctx.input(|input| input.pointer.any_down()));
            assert_eq!(click(&ctx, &model, pos), vec![Command::Launcher]);
        }
    }
}
