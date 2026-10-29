//! Shared Nocturne material and anchored, bounded top-menu surfaces.
use egui::{Color32, FontId, Id, Pos2, Rect, Sense, Stroke, Vec2};
use tarot_core::{
    Command, Layout, ShellModel, SystemPage,
    menu::{Action, Item, Panel, Target},
};

pub const TEXT: Color32 = Color32::from_rgb(231, 236, 225);
const SECONDARY: Color32 = Color32::from_rgb(182, 196, 173);

fn background(rect: Rect) -> egui::Shape {
    let radius = 10.0_f32.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let mut edge = Vec::new();
    for (center, start) in [
        (rect.left_top() + Vec2::splat(radius), std::f32::consts::PI),
        (
            rect.right_top() + Vec2::new(-radius, radius),
            std::f32::consts::PI * 1.5,
        ),
        (rect.right_bottom() - Vec2::splat(radius), 0.0),
        (
            rect.left_bottom() + Vec2::new(radius, -radius),
            std::f32::consts::FRAC_PI_2,
        ),
    ] {
        for step in 0..=8 {
            let angle = start + std::f32::consts::FRAC_PI_2 * step as f32 / 8.0;
            edge.push(center + Vec2::new(angle.cos(), angle.sin()) * radius);
        }
    }
    let color = |y: f32| {
        let t = ((y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
        Color32::from_rgb(
            (72.0 - 34.0 * t) as u8,
            (77.0 - 32.0 * t) as u8,
            (76.0 - 30.0 * t) as u8,
        )
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.center(), color(rect.center().y));
    for p in &edge {
        mesh.colored_vertex(*p, color(p.y));
    }
    for at in 0..edge.len() {
        mesh.add_triangle(0, at as u32 + 1, ((at + 1) % edge.len()) as u32 + 1);
    }
    egui::Shape::Vec(vec![
        egui::epaint::Shadow {
            offset: [0, 5],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(150),
        }
        .as_shape(rect, 10_u8)
        .into(),
        egui::Shape::mesh(mesh),
        egui::Shape::rect_stroke(
            rect,
            10.0,
            Stroke::new(0.7_f32, Color32::from_rgb(123, 138, 125)),
            egui::StrokeKind::Inside,
        ),
    ])
}

pub(crate) fn panel(
    ctx: &egui::Context,
    model: &ShellModel,
    layout: Layout,
    commands: &mut Vec<Command>,
    contents: impl FnOnce(&mut egui::Ui, &mut Vec<Command>),
) {
    let full = Rect::from_min_max(
        Pos2::new(0.0, Layout::STATUS),
        Pos2::new(layout.width, layout.gesture().y),
    );
    ctx.set_sublayer(
        egui::LayerId::new(egui::Order::Foreground, "top_menu_scrim".into()),
        egui::LayerId::new(egui::Order::Foreground, "top_menu_panel".into()),
    );
    egui::Area::new("top_menu_scrim".into())
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
                    Id::new(("top_menu_dismiss", model.menus.revision)),
                    Sense::click(),
                )
                .clicked()
            {
                commands.push(Command::Menu(Action::Dismiss));
            }
        });
    let left = model.system_page == SystemPage::Activity;
    let width = (if left {
        236.0_f32
    } else if model.system_page == SystemPage::Device {
        250.0
    } else {
        440.0
    })
    .min(layout.width - 16.0)
    .max(1.0);
    let origin = Pos2::new(
        if left {
            4.0
        } else {
            layout.width - width - 4.0
        },
        Layout::STATUS,
    );
    let height = (layout.gesture().y - layout.keyboard_height - Layout::STATUS - 8.0).max(1.0);
    egui::Area::new("top_menu_panel".into())
        .order(egui::Order::Foreground)
        .fixed_pos(origin)
        .movable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.set_max_height(height);
            ui.set_clip_rect(Rect::from_min_size(origin, Vec2::new(width, height)));
            let shape = ui.painter().add(egui::Shape::Noop);
            let frame = egui::Frame::new().inner_margin(8.0).show(ui, |ui| {
                ui.set_width((width - 16.0).max(1.0));
                ui.set_max_height((height - 16.0).max(1.0));
                ui.style_mut().visuals.override_text_color = Some(TEXT);
                ui.style_mut().visuals.selection.bg_fill = Color32::from_rgb(95, 115, 91);
                ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::from_rgb(58, 66, 61);
                ui.style_mut().visuals.widgets.hovered.bg_fill = Color32::from_rgb(76, 88, 78);
                ui.style_mut().visuals.widgets.active.bg_fill = Color32::from_rgb(99, 116, 97);
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.spacing_mut().interact_size.y = 48.0;
                let service_revision = match model.system_page {
                    SystemPage::Power => model.power.generation,
                    SystemPage::Audio => model.audio.generation,
                    SystemPage::Wifi => model.network.generation,
                    _ => 0,
                };
                ui.push_id((model.menus.revision, service_revision), |ui| {
                    contents(ui, commands)
                });
            });
            ui.painter().set(shape, background(frame.response.rect));
        });
}

fn choose(commands: &mut Vec<Command>, model: &ShellModel, target: Target) {
    commands.push(Command::Menu(Action::Choose {
        revision: model.menus.revision,
        target,
    }));
}

fn row(ui: &mut egui::Ui, model: &ShellModel, item: Item, commands: &mut Vec<Command>) {
    let inset = if item.checked { 30.0 } else { 12.0 };
    let width = (ui.available_width() - inset - 24.0).max(1.0);
    let label = ui.painter().layout(
        item.label.clone(),
        FontId::proportional(20.0),
        if item.enabled {
            TEXT
        } else {
            Color32::from_gray(145)
        },
        width,
    );
    let detail = ui
        .painter()
        .layout(item.detail, FontId::proportional(14.0), SECONDARY, width);
    let height = 56.0_f32.max(
        label.size().y
            + if detail.is_empty() {
                20.0
            } else {
                detail.size().y + 16.0
            },
    );
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let response = ui.interact(
        rect,
        Id::new(("top_menu_row", model.menus.revision, &item.target)),
        if item.enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, item.enabled, &item.label)
    });
    if model.menus.selected.as_ref() == Some(&item.target) {
        let id = Id::new("top_menu_selected_scroll");
        let token = (model.menus.revision, item.target.clone());
        if ui
            .ctx()
            .data_mut(|d| d.get_temp::<(u64, Target)>(id))
            .as_ref()
            != Some(&token)
        {
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
            ui.ctx().data_mut(|d| d.insert_temp(id, token));
        }
        ui.painter()
            .rect_filled(rect, 2.0, Color32::from_rgb(81, 99, 81));
    }
    if response.is_pointer_button_down_on() {
        ui.painter()
            .rect_filled(rect, 2.0, Color32::from_rgb(93, 111, 91));
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 2.0, Color32::from_white_alpha(12));
    }
    let top = rect.top()
        + (height
            - label.size().y
            - if detail.is_empty() {
                0.0
            } else {
                detail.size().y + 3.0
            })
            / 2.0;
    let label_height = label.size().y;
    ui.painter()
        .galley(Pos2::new(rect.left() + inset, top), label, TEXT);
    if !detail.is_empty() {
        ui.painter().galley(
            Pos2::new(rect.left() + inset, top + label_height + 3.0),
            detail,
            SECONDARY,
        );
    }
    if item.checked {
        ui.painter().text(
            Pos2::new(rect.left() + 12.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            "✓",
            FontId::proportional(16.0),
            SECONDARY,
        );
    }
    if item.submenu {
        ui.painter().text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            egui::Align2::CENTER_CENTER,
            "›",
            FontId::proportional(26.0),
            SECONDARY,
        );
    }
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(0.7_f32, Color32::from_black_alpha(160)),
    );
    if response.gained_focus() {
        commands.push(Command::Menu(Action::Select(item.target.clone())));
    }
    if response.clicked() {
        choose(commands, model, item.target);
    }
}

pub(crate) fn header(
    ui: &mut egui::Ui,
    model: &ShellModel,
    title: &str,
    commands: &mut Vec<Command>,
) {
    ui.horizontal(|ui| {
        if ui
            .add_sized([64.0, 44.0], egui::Button::new("‹ Back"))
            .clicked()
        {
            commands.push(Command::Menu(Action::Back));
        }
        ui.add(egui::Label::new(egui::RichText::new(title).size(18.0).strong()).wrap());
    });
    ui.separator();
    let _ = model;
}

pub(crate) fn roots(
    ui: &mut egui::Ui,
    model: &ShellModel,
    layout: Layout,
    commands: &mut Vec<Command>,
) {
    egui::ScrollArea::vertical()
        .id_salt(("top_menu_rows", model.system_page, model.menus.panel))
        .max_height(ui.available_height())
        .show(ui, |ui| {
            if model.system_page == SystemPage::Device {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(if model.date_label.is_empty() {
                        "Date unavailable"
                    } else {
                        &model.date_label
                    })
                    .font(crate::theme::bold(ui.ctx(), 17.0)),
                );
                ui.add_space(6.0);
                let battery = model.power.battery.level_label();
                let battery = if model.power.battery.available && model.power.battery.present {
                    format!("Battery {battery}")
                } else {
                    battery
                };
                ui.label(egui::RichText::new(battery).size(15.0).color(SECONDARY));
                ui.add_space(12.0);
                ui.separator();
            } else if model.menus.panel != Panel::Root {
                let title = match model.menus.panel {
                    Panel::Pages => "Launcher pages",
                    Panel::Page(id) => model
                        .launcher
                        .settings
                        .page(id)
                        .map_or("Page", |p| p.name.as_str()),
                    Panel::Arrange(id) => model
                        .cards
                        .iter()
                        .find(|c| c.id == id)
                        .map_or("Arrange card", |c| c.title.as_str()),
                    Panel::Root => unreachable!(),
                };
                header(ui, model, title, commands);
            } else if matches!(model.view, tarot_core::View::App(_)) {
                ui.label(
                    egui::RichText::new("Card controls")
                        .size(14.0)
                        .color(SECONDARY),
                );
            }
            for item in tarot_core::menu::items(model) {
                row(ui, model, item, commands);
            }
            if model.system_page == SystemPage::Device {
                ui.add_space(10.0);
                brightness(ui, model, commands);
            }
            let _ = layout;
        });
}

fn brightness(ui: &mut egui::Ui, model: &ShellModel, commands: &mut Vec<Command>) {
    let b = &model.power.brightness;
    let observed = b.percent().unwrap_or(50);
    let id = Id::new("top_menu_brightness_draft");
    let mut value = ui
        .ctx()
        .data_mut(|d| d.get_temp::<(u64, u64, u8)>(id))
        .filter(|(m, g, _)| *m == model.menus.revision && *g == model.power.generation)
        .map_or(observed, |(_, _, v)| v)
        .clamp(5, 100);
    ui.label(b.percent().map_or_else(
        || "Brightness unavailable".into(),
        |v| format!("Brightness {v}%"),
    ));
    let r = ui
        .push_id((model.menus.revision, model.power.generation), |ui| {
            ui.spacing_mut().slider_width = ui.available_width();
            ui.visuals_mut().selection.bg_fill = Color32::from_rgb(153, 173, 147);
            let widgets = &mut ui.visuals_mut().widgets;
            for visuals in [
                &mut widgets.inactive,
                &mut widgets.hovered,
                &mut widgets.active,
            ] {
                visuals.bg_fill = Color32::from_rgb(194, 202, 185);
            }
            ui.add_enabled(
                model.power.enabled && b.can_set && b.pending_percent.is_none(),
                egui::Slider::new(&mut value, 5..=100)
                    .show_value(false)
                    .trailing_fill(true)
                    .handle_shape(egui::style::HandleShape::Circle),
            )
        })
        .inner;
    if r.dragged() {
        ui.ctx()
            .data_mut(|d| d.insert_temp(id, (model.menus.revision, model.power.generation, value)));
    }
    if r.drag_stopped() || (r.changed() && !r.dragged()) {
        choose(
            commands,
            model,
            Target::Brightness {
                generation: model.power.generation,
                percent: value,
            },
        );
        ui.ctx().data_mut(|d| d.remove::<(u64, u64, u8)>(id));
    }
    if b.pending_percent.is_some() {
        ui.label("Changing brightness…");
    }
    if let Some(error) = b.action_error.as_ref().or(b.error.as_ref()) {
        ui.label(egui::RichText::new(error).color(Color32::LIGHT_RED));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, PointerButton};
    use tarot_core::{View, WindowId};

    fn render(
        ctx: &egui::Context,
        model: &ShellModel,
        size: Vec2,
        events: Vec<Event>,
    ) -> (Vec<Command>, egui::FullOutput) {
        crate::prepare_pointer_input(ctx, &events);
        let mut commands = vec![];
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                commands = crate::show(
                    ctx,
                    model,
                    Layout {
                        width: size.x,
                        height: size.y,
                        keyboard_height: 0.0,
                    },
                );
            },
        );
        (commands, output)
    }
    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn position(output: &egui::FullOutput, label: &str) -> Pos2 {
        output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t)
                    if t.galley.job.text == label
                        && s.clip_rect.contains(t.pos + t.galley.size() * 0.5) =>
                {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{label} must be painted inside its viewport"))
    }
    fn settle(ctx: &egui::Context, model: &ShellModel, size: Vec2) -> egui::FullOutput {
        let mut last = None;
        for _ in 0..4 {
            last = Some(render(ctx, model, size, vec![]).1);
        }
        last.unwrap()
    }

    #[test]
    fn a_real_held_card_menu_press_cannot_close_after_switching_roots_and_back() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        let size = Vec2::new(360.0, 720.0);
        model.add_window(WindowId(1), "Unsaved editor".into(), "editor".into());
        model.apply(Command::Menu(Action::ToggleActivity));
        let pos = position(&settle(&ctx, &model, size), "Close card");
        assert!(
            render(
                &ctx,
                &model,
                size,
                vec![Event::PointerMoved(pos), button(pos, true)]
            )
            .0
            .is_empty()
        );
        model.apply(Command::Menu(Action::ToggleDevice));
        render(&ctx, &model, size, vec![]);
        model.apply(Command::Menu(Action::ToggleActivity));
        settle(&ctx, &model, size);
        let commands = render(&ctx, &model, size, vec![button(pos, false)]).0;
        assert!(
            !commands
                .iter()
                .any(|c| matches!(c, Command::Menu(Action::Choose { .. }))),
            "{commands:?}"
        );
        assert!(!model.cards[0].closing);
        render(
            &ctx,
            &model,
            size,
            vec![Event::PointerMoved(pos), button(pos, true)],
        );
        let commands = render(&ctx, &model, size, vec![button(pos, false)]).0;
        assert_eq!(
            commands,
            vec![Command::Menu(Action::Choose {
                revision: model.menus.revision,
                target: Target::CloseCard(WindowId(1))
            })]
        );
    }

    #[test]
    fn outside_contact_dismisses_once_without_underlying_card_or_launcher_action() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        let size = Vec2::new(360.0, 720.0);
        model.add_window(WindowId(1), "App".into(), "app".into());
        model.apply(Command::Overview);
        model.apply(Command::Menu(Action::ToggleActivity));
        settle(&ctx, &model, size);
        let pos = Pos2::new(320.0, 400.0);
        assert!(
            render(
                &ctx,
                &model,
                size,
                vec![Event::PointerMoved(pos), button(pos, true)]
            )
            .0
            .is_empty()
        );
        let commands = render(&ctx, &model, size, vec![button(pos, false)]).0;
        assert_eq!(commands, vec![Command::Menu(Action::Dismiss)]);
        for c in commands {
            model.apply(c);
        }
        assert_eq!(model.view, View::Overview);
        assert!(render(&ctx, &model, size, vec![]).0.is_empty());
    }

    #[test]
    fn opening_or_repainting_device_menu_never_changes_brightness_even_at_zero() {
        for size in [Vec2::new(360.0, 720.0), Vec2::new(720.0, 360.0)] {
            for observed in [0, 30, 100] {
                let ctx = egui::Context::default();
                let mut model = ShellModel::default();
                model.date_label = "Thursday, October 29".into();
                model.power.enabled = true;
                model.power.brightness.can_set = true;
                model.power.brightness.maximum = 100;
                model.power.brightness.actual = Some(observed);
                model.apply(Command::Menu(Action::ToggleDevice));
                for frame in 0..8 {
                    let (commands, output) = render(&ctx, &model, size, vec![]);
                    assert!(commands.is_empty(), "rendering emitted {commands:?}");
                    if frame == 7 {
                        position(&output, "Wi-Fi");
                    }
                    assert_eq!(model.power.brightness.actual, Some(observed));
                }
            }
        }
    }

    #[test]
    fn brightness_drag_commits_once_on_release_and_a_new_service_snapshot_retires_it() {
        for replace in [false, true] {
            let ctx = egui::Context::default();
            let mut model = ShellModel::default();
            let size = Vec2::new(360.0, 720.0);
            model.power.enabled = true;
            model.power.generation = 4;
            model.power.brightness.maximum = 100;
            model.power.brightness.actual = Some(30);
            model.power.brightness.can_set = true;
            model.apply(Command::Menu(Action::ToggleDevice));
            let output = settle(&ctx, &model, size);
            let handle = output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::Shape::Circle(c)
                        if c.fill == Color32::from_rgb(194, 202, 185) && c.radius > 10.0 =>
                    {
                        Some(c.center)
                    }
                    _ => None,
                })
                .expect("the rendered brightness handle must be visible");
            assert!(
                render(
                    &ctx,
                    &model,
                    size,
                    vec![Event::PointerMoved(handle), button(handle, true)]
                )
                .0
                .is_empty()
            );
            let end = handle + Vec2::new(50.0, 0.0);
            assert!(
                render(&ctx, &model, size, vec![Event::PointerMoved(end)])
                    .0
                    .is_empty()
            );
            if replace {
                model.power.generation += 1;
                render(&ctx, &model, size, vec![]);
            }
            let commands = render(&ctx, &model, size, vec![button(end, false)]).0;
            if replace {
                assert!(commands.is_empty(), "{commands:?}");
            } else {
                assert!(
                    matches!(
                        commands.as_slice(),
                        [Command::Menu(Action::Choose {
                            revision: _,
                            target: Target::Brightness {
                                generation: 4,
                                percent: 31..=100
                            }
                        })]
                    ),
                    "{commands:?}"
                );
            }
            assert_eq!(
                model.power.brightness.actual,
                Some(30),
                "the UI cannot invent confirmation"
            );
            assert!(render(&ctx, &model, size, vec![]).0.is_empty());
        }
    }

    #[test]
    fn detail_page_back_is_reachable_by_tab_and_enter() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        let size = Vec2::new(360.0, 720.0);
        model.apply(Command::Menu(Action::ToggleDevice));
        model.apply(Command::SystemPage(SystemPage::Power));
        let back = position(&settle(&ctx, &model, size), "‹ Back");
        let key = |key, pressed| Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        let mut reached = false;
        for count in 1..=16 {
            render(&ctx, &model, size, vec![key(egui::Key::Tab, true)]);
            render(&ctx, &model, size, vec![key(egui::Key::Tab, false)]);
            if ctx
                .memory(|m| m.focused())
                .and_then(|id| ctx.read_response(id))
                .is_some_and(|r| {
                    r.rect.contains(back) && r.rect.width() <= 100.0 && r.rect.height() <= 60.0
                })
            {
                eprintln!("Back reached after {count} Tab presses");
                reached = true;
                break;
            }
        }
        assert!(reached, "Back must participate in keyboard focus traversal");
        assert_eq!(
            render(&ctx, &model, size, vec![key(egui::Key::Enter, true)]).0,
            vec![Command::Menu(Action::Back)]
        );
    }
}
