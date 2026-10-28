//! The shared egui shell view; all platform effects are returned as commands.
pub mod audio;
pub mod credentials;
pub mod keyboard;
pub mod lock_recovery;
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
    theme::install(ctx);
    theme::prepare_art(ctx);
    let mut commands = Vec::new();
    let mut search_focused = false;
    let search_id = egui::Id::new("launcher_search");
    let modal = model.system_menu_open || model.notifications_open;
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
            let status = Rect::from_min_size(Pos2::ZERO, Vec2::new(layout.width, Layout::STATUS));
            painter.rect_filled(status, 0.0, theme::STATUS);
            let title = match model.view {
                View::App(id) => model
                    .cards
                    .iter()
                    .find(|card| card.id == id)
                    .map(|card| card.title.as_str())
                    .unwrap_or("Tarot"),
                View::Launcher => "Launcher",
                _ => "Tarot",
            };
            painter
                .with_clip_rect(Rect::from_min_max(
                    Pos2::ZERO,
                    Pos2::new(layout.width / 2.0 - 36.0, Layout::STATUS),
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
            painter.text(
                Pos2::new(layout.width - 10.0, Layout::STATUS / 2.0),
                Align2::RIGHT_CENTER,
                system::status_label(model),
                FontId::proportional(11.0),
                Color32::from_gray(185),
            );
            let status_button = Rect::from_min_size(
                Pos2::new(layout.width - 104.0, 0.0),
                Vec2::new(104.0, Layout::STATUS),
            );
            if ui
                .interact(status_button, ui.id().with("system-status"), Sense::click())
                .clicked()
            {
                commands.push(Command::SystemMenuOpen(!model.system_menu_open));
            }

            match model.view {
                View::Home => {}
                View::Overview => {}
                View::Launcher => {
                    search_focused = ui
                        .scope(|ui| {
                            if modal {
                                ui.disable();
                            }
                            show_launcher(ui, &painter, model, layout, &mut commands)
                        })
                        .inner
                        || (keyboard_interaction && !modal)
                }
                View::App(_) => {}
            }

            if presentation.overview > 0.001 {
                show_cards(ui, model, layout, presentation, &mut commands);
            }

            if presentation.dock > 0.001
                && (model.view != View::Launcher || model.editing_favorites)
            {
                ui.scope(|ui| {
                    ui.set_opacity(presentation.dock);
                    if matches!(model.view, View::App(_)) || modal {
                        ui.disable();
                    }
                    quick_launch::show(ui, model, layout, None, &mut commands);
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
            if ui
                .put(
                    alerts,
                    egui::Button::new(format!("Alerts {}", model.notifications.items.len()))
                        .frame(false),
                )
                .on_hover_text("Open notification dashboard")
                .clicked()
            {
                commands.push(Command::NotificationsOpen(!model.notifications_open));
            }
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
    if model.view == View::Launcher && model.shell_text_focus {
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
                    .fill(Color32::from_rgb(38, 40, 44))
                    .inner_margin(5.0)
                    .show(ui, |ui| {
                        actions = keyboard.show_in_ui(ui, false, false);
                    });
            });
        ctx.data_mut(|data| data.insert_temp(keyboard_id, keyboard));
        let hide =
            actions.contains(&keyboard::Action::Hide) || actions.contains(&keyboard::Action::Enter);
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
    let menu_open = model.view == View::Overview && egui::Popup::is_any_open(ctx);
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
                quick_launch::show(ui, model, layout, Some(wave), &mut commands);
            });
    }
    if let Some(feedback) = &model.volume_feedback {
        audio::show_key_feedback(ctx, feedback, layout);
    }
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
                || model.notifications_open
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
                painter.rect_stroke(
                    card_rect.expand(2.0),
                    8.0,
                    Stroke::new(2.0_f32, Color32::from_white_alpha(125)),
                    StrokeKind::Outside,
                );
            }
            let response = ui.interact(card_rect, ui.id().with("activate"), Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(
                    egui::WidgetType::Button,
                    true,
                    format!("Open {}", card.title),
                )
            });
            if response.clicked() {
                commands.push(Command::Activate(card.id));
            }
            let label = if card.closing {
                format!("Closing… {}", card.title)
            } else {
                card.title.clone()
            };
            if representative == Some(card.id) && presentation.arrange.is_none() {
                painter
                    .with_clip_rect(Rect::from_min_size(
                        Pos2::new(card_rect.left(), card_rect.bottom() + 4.0),
                        Vec2::new(card_rect.width(), 32.0),
                    ))
                    .text(
                        Pos2::new(card_rect.center().x, card_rect.bottom() + 20.0),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(14.0),
                        Color32::WHITE,
                    );
            }
            // Only the selected card exposes controls; neighboring members
            // remain selectable through their visible card strips.
            if Some(card.id) == model.selected && presentation.arrange.is_none() {
                let close = Rect::from_center_size(
                    Pos2::new(card_rect.right() - 16.0, card_rect.top() - 26.0),
                    Vec2::splat(48.0),
                );
                if ui
                    .scope(|ui| {
                        if card.closing || model.view != View::Overview {
                            ui.disable();
                        }
                        ui.put(close, egui::Button::new("×"))
                            .on_hover_text(format!("Close {}", card.title))
                    })
                    .inner
                    .clicked()
                {
                    commands.push(Command::Close(card.id));
                }
                let options = Rect::from_center_size(
                    Pos2::new(card_rect.left() + 16.0, card_rect.top() - 26.0),
                    Vec2::splat(48.0),
                );
                let response = ui
                    .add_enabled_ui(!card.closing, |ui| {
                        ui.put(options, egui::Button::new("…"))
                            .on_hover_text(format!("Arrange {}", card.title))
                    })
                    .inner;
                egui::Popup::menu(&response).show(|ui| {
                    ui.set_min_width(220.0);
                    ui.spacing_mut().interact_size = Vec2::new(220.0, 48.0);
                    let index = model
                        .cards
                        .iter()
                        .position(|item| item.id == card.id)
                        .unwrap();
                    for (label, neighbor, before) in [
                        (
                            "Move left",
                            index.checked_sub(1).and_then(|i| model.cards.get(i)),
                            true,
                        ),
                        ("Move right", model.cards.get(index + 1), false),
                    ] {
                        if let Some(neighbor) = neighbor {
                            let target = if neighbor.stack == card.stack {
                                CardDrop::Into {
                                    anchor: neighbor.id,
                                    before,
                                }
                            } else {
                                CardDrop::Separate {
                                    anchor: neighbor.id,
                                    before,
                                }
                            };
                            if ui.button(label).clicked() {
                                commands.push(Command::PlaceCard {
                                    card: card.id,
                                    target,
                                });
                                ui.close();
                            }
                            if neighbor.stack != card.stack
                                && ui
                                    .button(format!("Stack with {}", neighbor.title))
                                    .clicked()
                            {
                                commands.push(Command::Stack {
                                    card: card.id,
                                    with: neighbor.id,
                                });
                                ui.close();
                            }
                        }
                    }
                    if model.stack_members(card.id).len() > 1
                        && ui.button("Separate from stack").clicked()
                    {
                        commands.push(Command::Unstack(card.id));
                        ui.close();
                    }
                });
                if members.len() > 1 {
                    let index = members.iter().position(|id| *id == card.id).unwrap();
                    painter.text(
                        Pos2::new(card_rect.center().x, card_rect.top() - 26.0),
                        Align2::CENTER_CENTER,
                        format!("{} / {}", index + 1, members.len()),
                        FontId::proportional(13.0),
                        Color32::WHITE,
                    );
                }
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

fn show_launcher(
    ui: &mut egui::Ui,
    painter: &egui::Painter,
    model: &ShellModel,
    layout: Layout,
    commands: &mut Vec<Command>,
) -> bool {
    painter.text(
        Pos2::new(20.0, 67.0),
        Align2::LEFT_CENTER,
        if model.editing_favorites {
            "Quick Launch"
        } else {
            "Applications"
        },
        FontId::proportional(22.0),
        Color32::WHITE,
    );
    if ui
        .put(
            Rect::from_min_size(
                Pos2::new(layout.width - 124.0, 43.0),
                Vec2::new(108.0, 48.0),
            ),
            egui::Button::new(if model.editing_favorites {
                "Done"
            } else {
                "Quick Launch"
            }),
        )
        .clicked()
    {
        commands.push(Command::EditFavorites(!model.editing_favorites));
    }
    if model.editing_favorites {
        quick_launch::editor(ui, model, layout, commands);
        return false;
    }
    let search = Rect::from_min_size(Pos2::new(20.0, 96.0), Vec2::new(layout.width - 40.0, 40.0));
    let mut query = model.launcher_query.clone();
    let response = ui.put(
        search,
        egui::TextEdit::singleline(&mut query)
            .id(egui::Id::new("launcher_search"))
            .hint_text("Search applications"),
    );
    let focused = response.has_focus();
    if response.changed() {
        commands.push(Command::SearchApplications(query.clone()));
    }
    let words: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
    let apps: Vec<_> = model
        .applications
        .iter()
        .filter(|app| {
            let haystack = format!(
                "{} {} {}",
                app.name,
                app.description,
                app.keywords.join(" ")
            )
            .to_lowercase();
            words.iter().all(|word| haystack.contains(word))
        })
        .collect();
    let bottom = if model.shell_text_focus {
        layout.keyboard().y - 12.0
    } else {
        layout.gesture().y - 12.0
    };
    let feedback_height = if model.launch_feedback.is_some() {
        60.0
    } else {
        0.0
    };
    let bounds = Rect::from_min_max(
        Pos2::new(16.0, 154.0),
        Pos2::new(layout.width - 16.0, bottom - feedback_height),
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
    child.spacing_mut().item_spacing.y = 10.0;
    egui::ScrollArea::vertical()
        .id_salt("installed_apps")
        .max_height(bounds.height())
        .show_rows(
            &mut child,
            104.0,
            apps.len().div_ceil(3).max(1),
            |ui, rows| {
                if apps.is_empty() {
                    ui.label(if model.applications.is_empty() {
                        "No installed applications found"
                    } else {
                        "No matching applications"
                    });
                }
                let width = (bounds.width() - 24.0) / 3.0;
                egui::Grid::new("application_grid")
                    .num_columns(3)
                    .spacing(Vec2::new(8.0, 10.0))
                    .show(ui, |ui| {
                        for (index, app) in apps
                            .iter()
                            .enumerate()
                            .skip(rows.start * 3)
                            .take(rows.len() * 3)
                        {
                            let response = ui
                                .push_id(&app.id, |ui| {
                                    ui.add_sized(
                                        Vec2::new(width, 104.0),
                                        egui::Button::new("").frame(false),
                                    )
                                })
                                .inner;
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    format!("Open {}", app.name),
                                )
                            });
                            let center =
                                Pos2::new(response.rect.center().x, response.rect.top() + 28.0);
                            let hue = app.id.bytes().fold(0u8, u8::wrapping_add);
                            let color = if app.unavailable.is_some() {
                                Color32::from_gray(80)
                            } else {
                                Color32::from_rgb(55 + hue % 80, 100 + hue % 90, 110 + hue % 75)
                            };
                            if !theme::paint_icon(
                                ui.ctx(),
                                ui.painter(),
                                &app.id,
                                false,
                                Rect::from_center_size(
                                    center,
                                    Vec2::splat(tarot_core::design::LAUNCHER_ICON),
                                ),
                                if app.unavailable.is_some() {
                                    Color32::from_gray(110)
                                } else {
                                    Color32::WHITE
                                },
                                model.calendar_day,
                            ) {
                                ui.painter().circle_filled(center, 24.0, color);
                                let initial = app
                                    .name
                                    .chars()
                                    .next()
                                    .unwrap_or('?')
                                    .to_uppercase()
                                    .to_string();
                                ui.painter().text(
                                    center,
                                    Align2::CENTER_CENTER,
                                    initial,
                                    FontId::proportional(25.0),
                                    Color32::WHITE,
                                );
                            }
                            let label = ui.painter().layout(
                                app.name.clone(),
                                FontId::proportional(14.0),
                                Color32::WHITE,
                                width - 4.0,
                            );
                            let label_x = response.rect.center().x - label.size().x / 2.0;
                            ui.painter().with_clip_rect(response.rect).galley(
                                Pos2::new(label_x, response.rect.top() + 60.0),
                                label,
                                Color32::WHITE,
                            );
                            if response.clicked() {
                                commands.push(Command::Launch(app.id.clone()));
                            }
                            response.on_hover_text(
                                app.unavailable.as_deref().unwrap_or(&app.description),
                            );
                            if index % 3 == 2 {
                                ui.end_row();
                            }
                        }
                    });
            },
        );
    if let Some(feedback) = &model.launch_feedback {
        let rect = Rect::from_min_size(
            Pos2::new(20.0, bottom - 54.0),
            Vec2::new(layout.width - 40.0, 50.0),
        );
        painter.rect_filled(rect, 8.0, Color32::from_black_alpha(180));
        let text = Rect::from_min_max(rect.min + Vec2::splat(8.0), rect.max - Vec2::new(44.0, 4.0));
        ui.put(text, egui::Label::new(&feedback.message).wrap());
        let close = Rect::from_min_size(
            Pos2::new(rect.right() - 44.0, rect.top() + 3.0),
            Vec2::splat(44.0),
        );
        if ui
            .put(close, egui::Button::new("×").frame(false))
            .on_hover_text("Dismiss message")
            .clicked()
        {
            commands.push(Command::DismissLaunchFeedback);
        }
    }
    focused
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
        for c in click(&ctx, &model, Pos2::new(285.0, 67.0)) {
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
        for c in click(&ctx, &model, Pos2::new(80.0, 116.0)) {
            model.apply(c);
        }
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
        let focus = click(&ctx, &model, Pos2::new(100.0, 115.0));
        assert!(focus.contains(&Command::ShellTextFocus(true)), "{focus:?}");
        model.apply(Command::ShellTextFocus(true));
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
        assert!(
            click(&ctx, &model, Pos2::new(100.0, 115.0)).contains(&Command::ShellTextFocus(true))
        );
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
            vec![Command::SystemMenuOpen(false)]
        );
        model.apply(Command::SystemMenuOpen(false));
        model.apply(Command::SystemPage(tarot_core::SystemPage::Power));
        for command in click(&ctx, &model, Pos2::new(310.0, 14.0)) {
            model.apply(command);
        }
        assert!(model.system_menu_open);
        assert_eq!(
            click(&ctx, &model, Pos2::new(124.0, 220.0)),
            vec![Command::Power(tarot_core::power::UiAction::Brightness {
                percent: 40
            })]
        );

        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.notifications.available = true;
        model.notifications.settings_available = true;
        model.apply(Command::NotificationsOpen(true));
        assert_eq!(
            click(&ctx, &model, Pos2::new(21.0, 100.0)),
            vec![Command::NotificationsOpen(false)]
        );
        model.apply(Command::NotificationsOpen(false));
        for command in click(&ctx, &model, Pos2::new(40.0, 706.0)) {
            model.apply(command);
        }
        assert!(model.notifications_open);
        assert_eq!(
            click(&ctx, &model, Pos2::new(50.0, 300.0)),
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
        model.audio.available = false;
        assert!(click_text(&ctx, &model, "+").is_empty());
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
            vec![Command::SystemMenuOpen(true)]
        );
        model.apply(Command::SystemMenuOpen(true));
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
        assert!(click(&ctx, &model, Pos2::new(124.0, 220.0)).is_empty());
        model.power.brightness.can_set = true;
        assert_eq!(
            click(&ctx, &model, Pos2::new(124.0, 220.0)),
            vec![Command::Power(tarot_core::power::UiAction::Brightness {
                percent: 40
            })]
        );
        assert_eq!(
            click(&ctx, &model, Pos2::new(292.0, 648.0)),
            vec![Command::SystemMenuOpen(false)]
        );
    }

    #[test]
    fn notification_dashboard_captures_taps_and_has_touch_settings() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        assert_eq!(
            click(&ctx, &model, Pos2::new(40.0, 706.0)),
            vec![Command::NotificationsOpen(true)]
        );
        model.apply(Command::NotificationsOpen(true));
        model.notifications.available = true;
        assert!(click(&ctx, &model, Pos2::new(50.0, 300.0)).is_empty());
        model.notifications.settings_available = true;
        assert_eq!(
            click(&ctx, &model, Pos2::new(50.0, 300.0)),
            vec![Command::Notification(
                tarot_core::notifications::UiAction::Settings {
                    dnd: true,
                    hide_previews: false,
                }
            )]
        );
        // This point belongs to the covered launcher button in the home view.
        assert!(!click(&ctx, &model, Pos2::new(292.0, 648.0)).contains(&Command::Launcher));
        assert_eq!(
            click(&ctx, &model, Pos2::new(160.0, 100.0)),
            vec![Command::NotificationsOpen(false)]
        );
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
        assert_eq!(
            click(&ctx, &model, Pos2::new(64.0, 188.0)),
            vec![Command::Launch("calc.desktop".into())]
        );
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
