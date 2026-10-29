//! The packet's local-first search sheet. Providers run only after selection.
use crate::{favorite_drag, launcher, theme};
use egui::{Align2, Color32, FontId, Id, Pos2, Rect, Sense, Stroke, Vec2};
use tarot_core::{
    Command, Layout, ShellModel,
    search::{self, Action, Setting, Target},
};

const PAPER: Color32 = Color32::from_rgb(230, 234, 223);
const TEXT: Color32 = Color32::from_rgb(53, 70, 46);
const SECONDARY: Color32 = Color32::from_rgb(99, 124, 88);
const BAND: Color32 = Color32::from_rgb(193, 204, 183);

fn action(commands: &mut Vec<Command>, value: Action) {
    commands.push(Command::JustType(value));
}

pub(crate) fn affordance(ui: &mut egui::Ui, layout: Layout, commands: &mut Vec<Command>) {
    let rect = Rect::from_min_size(Pos2::new(9.0, 40.0), Vec2::new(layout.width - 18.0, 40.0));
    if ui
        .put(
            rect,
            egui::Button::new(egui::RichText::new("Just type…").italics().size(20.0))
                .fill(Color32::from_black_alpha(64))
                .corner_radius(20.0),
        )
        .clicked()
    {
        action(commands, Action::Open);
    }
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    model: &ShellModel,
    layout: Layout,
    commands: &mut Vec<Command>,
) -> bool {
    let compact = layout.height < 480.0;
    let bottom = if model.shell_text_focus {
        layout.keyboard().y
    } else {
        layout.gesture().y
    };
    let body = Rect::from_min_max(
        Pos2::new(0.0, Layout::STATUS),
        Pos2::new(layout.width, bottom),
    );
    ui.painter().rect_filled(body, 0.0, PAPER);
    let y = if compact { 36.0 } else { 43.0 };
    let field = Rect::from_min_size(Pos2::new(9.0, y), Vec2::new(layout.width - 18.0, 42.0));
    ui.painter().rect(
        field,
        21.0,
        Color32::from_rgb(247, 249, 238),
        Stroke::new(1.0_f32, Color32::from_rgb(164, 180, 157)),
        egui::StrokeKind::Inside,
    );
    let glass = Pos2::new(30.0, y + 19.0);
    ui.painter()
        .circle_stroke(glass, 5.0, Stroke::new(1.5_f32, SECONDARY));
    ui.painter().line_segment(
        [glass + Vec2::splat(4.0), glass + Vec2::splat(10.0)],
        Stroke::new(1.5_f32, SECONDARY),
    );
    let id = launcher::text_id(model);
    ui.ctx().memory_mut(|m| {
        if model.shell_text_focus && ui.is_enabled() {
            m.request_focus(id);
        } else {
            m.surrender_focus(id);
        }
    });
    let mut query = model.launcher_query.clone();
    let response = ui.put(
        Rect::from_min_size(
            Pos2::new(51.0, y + 6.0),
            Vec2::new((layout.width - 110.0).max(1.0), 32.0),
        ),
        egui::TextEdit::singleline(&mut query)
            .id(id)
            .frame(false)
            .text_color(TEXT)
            .font(FontId::proportional(22.0))
            .hint_text("Just type…")
            .char_limit(search::QUERY_CHARS),
    );
    if response.changed() {
        commands.push(Command::SearchApplications(query));
    }
    let cleared = ui
        .put(
            Rect::from_min_size(Pos2::new(layout.width - 54.0, y - 1.0), Vec2::splat(44.0)),
            egui::Button::new(egui::RichText::new("×").color(SECONDARY).size(26.0)).frame(false),
        )
        .on_hover_text("Clear search")
        .clicked();
    if cleared {
        action(commands, Action::Clear);
        ui.ctx().memory_mut(|m| m.request_focus(id));
    }

    let bounds = Rect::from_min_max(
        Pos2::new(0.0, if compact { 84.0 } else { 99.0 }),
        Pos2::new(
            layout.width,
            (bottom - 4.0).max(if compact { 84.0 } else { 99.0 }),
        ),
    );
    let words = search::words(&model.launcher_query);
    let applications: Vec<_> = model
        .applications
        .iter()
        .filter(|a| !words.is_empty() && search::application_matches(&words, a))
        .collect();
    let settings: Vec<_> = Setting::ALL
        .into_iter()
        .filter(|s| s.matches(&words))
        .collect();
    let notifications: Vec<_> = model
        .notifications
        .items
        .iter()
        .filter(|n| {
            !words.is_empty()
                && search::notification_matches(&words, n, model.notifications.hide_previews)
        })
        .collect();
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(bounds)
            .id_salt("just_type_results"),
    );
    child.set_clip_rect(bounds.intersect(ui.clip_rect()));
    child.visuals_mut().override_text_color = Some(TEXT);
    child.visuals_mut().selection.bg_fill = BAND;
    child.visuals_mut().selection.stroke.color = TEXT;
    egui::ScrollArea::vertical()
        .id_salt(("just_type_scroll", model.search.revision))
        .auto_shrink([false, false])
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .max_height(bounds.height())
        .show_viewport(&mut child, |ui, viewport| {
            ui.set_min_width(bounds.width());
            let origin = ui.max_rect().min;
            let mut y = 0.0;
            if let Some(feedback) = &model.launch_feedback {
                let r = Rect::from_min_size(origin, Vec2::new(layout.width, 58.0));
                ui.painter().rect_filled(r, 0.0, BAND);
                ui.put(
                    Rect::from_min_max(r.min + Vec2::new(12.0, 4.0), r.max - Vec2::new(48.0, 4.0)),
                    egui::Label::new(
                        egui::RichText::new(&feedback.message)
                            .size(15.0)
                            .color(TEXT),
                    )
                    .wrap(),
                );
                if ui
                    .put(
                        Rect::from_min_size(
                            Pos2::new(r.right() - 48.0, r.top() + 4.0),
                            Vec2::splat(44.0),
                        ),
                        egui::Button::new("×").frame(false),
                    )
                    .clicked()
                {
                    commands.push(Command::DismissLaunchFeedback);
                }
                y += 62.0;
            }
            if !applications.is_empty() {
                if !compact {
                    header(ui, origin, y, "APPLICATIONS", layout.width);
                    y += 35.0;
                }
                let height = if compact { 56.0 } else { 112.0 };
                let rows = applications.len().div_ceil(3);
                let first = ((viewport.min.y - y).max(0.0) / height).floor() as usize;
                let last = (((viewport.max.y - y).max(0.0) / height).ceil() as usize).min(rows);
                let width = (layout.width - 48.0) / 3.0;
                if let Some(index) = applications.iter().position(|a| {
                    model.search.selected.as_ref() == Some(&Target::Application(a.id.clone()))
                }) {
                    let rect = Rect::from_min_size(
                        origin
                            + Vec2::new(
                                16.0 + (index % 3) as f32 * (width + 8.0),
                                y + (index / 3) as f32 * height,
                            ),
                        Vec2::new(width, height),
                    );
                    selected(
                        ui,
                        model,
                        &Target::Application(applications[index].id.clone()),
                        rect,
                    );
                }
                for row in first..last {
                    for col in 0..3 {
                        let index = row * 3 + col;
                        let Some(application) = applications.get(index) else {
                            break;
                        };
                        let rect = Rect::from_min_size(
                            origin
                                + Vec2::new(
                                    16.0 + col as f32 * (width + 8.0),
                                    y + row as f32 * height,
                                ),
                            Vec2::new(width, height),
                        );
                        let target = Target::Application(application.id.clone());
                        let response = ui.interact(
                            rect,
                            Id::new(("just_type_app", model.search.revision, &application.id)),
                            Sense::click(),
                        );
                        response.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                ui.is_enabled(),
                                format!("Open {}", application.name),
                            )
                        });
                        favorite_drag::register(
                            ui,
                            tarot_core::favorite_drag::Source::Launcher(application.id.clone()),
                            rect,
                        );
                        selected(ui, model, &target, rect);
                        launcher::paint_application(
                            ui,
                            application,
                            &response,
                            model,
                            launcher::AppStyle {
                                compact,
                                dragged: false,
                                label: TEXT,
                            },
                            commands,
                        );
                        if response.clicked() {
                            choose(commands, model, target);
                        }
                        response.on_hover_text(
                            application
                                .unavailable
                                .as_deref()
                                .unwrap_or(&application.description),
                        );
                    }
                }
                y += rows as f32 * height + 8.0;
            }
            if !settings.is_empty() {
                if !compact || y > 0.0 {
                    header(ui, origin, y, "SETTINGS", layout.width);
                    y += 25.0;
                }
                let height = if compact { 56.0 } else { 64.0 };
                for setting in settings {
                    let rect = Rect::from_min_size(
                        origin + Vec2::new(0.0, y),
                        Vec2::new(layout.width, height),
                    );
                    row(
                        ui,
                        model,
                        Target::Setting(setting),
                        rect,
                        setting.title(),
                        setting.detail(),
                        commands,
                    );
                    y += height;
                }
                y += 8.0;
            }
            if !notifications.is_empty() {
                if !compact || y > 0.0 {
                    header(ui, origin, y, "NOTIFICATIONS", layout.width);
                    y += 25.0;
                }
                let height = if compact { 56.0 } else { 72.0 };
                for notification in notifications {
                    let rect = Rect::from_min_size(
                        origin + Vec2::new(0.0, y),
                        Vec2::new(layout.width, height),
                    );
                    let title = if model.notifications.hide_previews {
                        notification.app_name.as_str()
                    } else {
                        notification.summary.as_str()
                    };
                    let detail = if model.notifications.hide_previews {
                        "Private preview hidden"
                    } else {
                        notification.body.as_str()
                    };
                    row(
                        ui,
                        model,
                        Target::Notification {
                            id: notification.id,
                            revision: notification.revision,
                        },
                        rect,
                        title,
                        detail,
                        commands,
                    );
                    y += height;
                }
                y += 8.0;
            }
            if search::website(&model.launcher_query).is_some() {
                if !compact || y > 0.0 {
                    header(ui, origin, y, "ACTIONS", layout.width);
                    y += 25.0;
                }
                let height = if compact { 56.0 } else { 64.0 };
                row(
                    ui,
                    model,
                    Target::OpenWebsite,
                    Rect::from_min_size(
                        origin + Vec2::new(0.0, y),
                        Vec2::new(layout.width, height),
                    ),
                    "Open website",
                    "Use the default browser",
                    commands,
                );
                y += height + 8.0;
            }
            if !words.is_empty() {
                if !compact || y > 0.0 {
                    header(ui, origin, y, "SEARCH USING", layout.width);
                    y += 25.0;
                }
                let height = if compact { 56.0 } else { 64.0 };
                row(
                    ui,
                    model,
                    Target::DuckDuckGo,
                    Rect::from_min_size(
                        origin + Vec2::new(0.0, y),
                        Vec2::new(layout.width, height),
                    ),
                    "DuckDuckGo",
                    "Search the web with this text",
                    commands,
                );
                y += height + 8.0;
            }
            ui.set_min_height(y);
        });
    response.has_focus() || cleared
}

fn choose(commands: &mut Vec<Command>, model: &ShellModel, target: Target) {
    action(
        commands,
        Action::Choose {
            revision: model.search.revision,
            target,
        },
    );
}
fn header(ui: &egui::Ui, origin: Pos2, y: f32, title: &str, width: f32) {
    let rect = Rect::from_min_size(origin + Vec2::new(0.0, y), Vec2::new(width, 25.0));
    if !ui.is_rect_visible(rect) {
        return;
    }
    ui.painter().rect_filled(rect, 0.0, BAND);
    ui.painter().text(
        rect.left_center() + Vec2::new(14.0, 0.0),
        Align2::LEFT_CENTER,
        title,
        theme::bold(ui.ctx(), 13.0),
        SECONDARY,
    );
}
fn selected(ui: &egui::Ui, model: &ShellModel, target: &Target, rect: Rect) {
    if model.search.selected.as_ref() == Some(target) {
        let id = Id::new("just_type_scroll_selection");
        let token = (model.search.revision, target.clone());
        if ui
            .ctx()
            .data_mut(|d| d.get_temp::<(u64, Target)>(id))
            .as_ref()
            != Some(&token)
        {
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
            ui.ctx().data_mut(|d| d.insert_temp(id, token));
        }
        ui.painter().rect_filled(rect.shrink(2.0), 4.0, BAND);
    }
}
fn row(
    ui: &mut egui::Ui,
    model: &ShellModel,
    target: Target,
    rect: Rect,
    title: &str,
    detail: &str,
    commands: &mut Vec<Command>,
) {
    selected(ui, model, &target, rect);
    if !ui.is_rect_visible(rect) {
        return;
    }
    let busy = matches!(target, Target::DuckDuckGo | Target::OpenWebsite)
        && model
            .launch_feedback
            .as_ref()
            .is_some_and(|f| f.pending && f.application_id == "@just-type-web");
    let response = ui.interact(
        rect,
        Id::new((
            "just_type_result",
            model.search.revision,
            format!("{target:?}"),
        )),
        if busy { Sense::hover() } else { Sense::click() },
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled() && !busy, title)
    });
    let painter = ui
        .painter()
        .with_clip_rect(rect.shrink2(Vec2::new(14.0, 0.0)));
    painter.text(
        rect.min + Vec2::new(18.0, 18.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional(20.0),
        TEXT,
    );
    let mut job = egui::text::LayoutJob::simple(
        detail.into(),
        FontId::proportional(14.0),
        SECONDARY,
        rect.width() - 36.0,
    );
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galley = painter.layout_job(job);
    painter.galley(rect.min + Vec2::new(18.0, 34.0), galley, SECONDARY);
    if response.clicked() {
        choose(commands, model, target);
    }
}

#[cfg(test)]
mod scroll_diagnostic_tests {
    use super::*;
    use egui::{Context, Event, Modifiers, PointerButton};
    use tarot_core::{Application, quick_launch};

    fn model() -> ShellModel {
        let mut m = ShellModel::default();
        m.applications = (0..3)
            .map(|n| Application {
                id: format!("volume{n}.desktop"),
                name: format!("Volume {n}"),
                description: String::new(),
                icon: None,
                keywords: vec!["volume".into()],
                unavailable: None,
            })
            .collect();
        m.apply(Command::JustType(Action::Open));
        m.apply(Command::SearchApplications("volume".into()));
        m
    }
    fn button(pos: Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }
    fn frame(
        ctx: &Context,
        m: &mut ShellModel,
        t: f64,
        events: Vec<Event>,
    ) -> (Vec<Command>, egui::FullOutput) {
        crate::prepare_pointer_input(ctx, &events);
        let mut commands = Vec::new();
        let layout = Layout {
            width: 720.0,
            height: 360.0,
            keyboard_height: if m.shell_text_focus { 180.0 } else { 0.0 },
        };
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(720.0, 360.0))),
                time: Some(t),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                commands = crate::show(ctx, m, layout);
            },
        );
        for c in &commands {
            m.apply(c.clone());
        }
        (commands, output)
    }
    fn warm(ctx: &Context, m: &mut ShellModel) {
        for i in 0..4 {
            frame(ctx, m, i as f64 * 0.01, vec![]);
        }
        m.apply(Command::ShellTextFocus(true));
        frame(ctx, m, 0.04, vec![]);
        frame(ctx, m, 0.05, vec![]);
        assert!(
            m.shell_text_focus,
            "Warm-up must retain the landscape keyboard"
        );
    }
    fn activates(c: &Command) -> bool {
        matches!(
            c,
            Command::Launch(_) | Command::JustType(Action::Choose { .. })
        )
    }
    #[test]
    fn dense_search_tile_tap_and_held_favorite_drop_remain_distinct() {
        let source = Pos2::new(592.0, 112.0);
        let ctx = Context::default();
        let mut m = model();
        warm(&ctx, &mut m);
        frame(
            &ctx,
            &mut m,
            0.1,
            vec![Event::PointerMoved(source), button(source, true)],
        );
        let (commands, _) = frame(
            &ctx,
            &mut m,
            0.2,
            vec![button(source, false), Event::PointerGone],
        );
        assert_eq!(commands.iter().filter(|c| activates(c)).count(), 1);
        assert!(commands.iter().any(|c| matches!(c,Command::JustType(Action::Choose{target:Target::Application(id),..}) if id=="volume2.desktop")));

        let ctx = Context::default();
        let mut m = model();
        warm(&ctx, &mut m);
        frame(
            &ctx,
            &mut m,
            0.1,
            vec![Event::PointerMoved(source), button(source, true)],
        );
        let (commands, _) = frame(&ctx, &mut m, 0.65, vec![]);
        assert!(!commands.iter().any(activates));
        assert!(!m.shell_text_focus);
        let layout = Layout {
            width: 720.0,
            height: 360.0,
            keyboard_height: 0.0,
        };
        let slot = quick_launch::slot(layout, 0, None);
        let drop = Pos2::new(slot.x + slot.width / 2.0, slot.y + slot.height / 2.0);
        frame(&ctx, &mut m, 0.7, vec![Event::PointerMoved(drop)]);
        let (commands, _) = frame(
            &ctx,
            &mut m,
            0.8,
            vec![button(drop, false), Event::PointerGone],
        );
        assert!(!commands.iter().any(activates));
        assert_eq!(
            commands
                .iter()
                .filter(|c| matches!(c, Command::Favorite(_)))
                .count(),
            1
        );
        assert_eq!(m.favorites.applications, vec!["volume2.desktop"]);
        assert_eq!(m.launcher_query, "volume");
        assert!(
            !frame(&ctx, &mut m, 0.9, vec![])
                .0
                .iter()
                .any(|c| matches!(c, Command::Favorite(_)))
        );
    }
    #[test]
    fn dense_search_drag_reveals_a_setting_without_launch_or_favorite() {
        let ctx = Context::default();
        let mut m = model();
        warm(&ctx, &mut m);
        let start = Pos2::new(592.0, 132.0);
        let mut commands = frame(
            &ctx,
            &mut m,
            0.1,
            vec![Event::PointerMoved(start), button(start, true)],
        )
        .0;
        for i in 1..=4 {
            commands.extend(
                frame(
                    &ctx,
                    &mut m,
                    0.1 + i as f64 * 0.02,
                    vec![Event::PointerMoved(Pos2::new(
                        592.0,
                        132.0 - i as f32 * 10.0,
                    ))],
                )
                .0,
            );
        }
        commands.extend(
            frame(
                &ctx,
                &mut m,
                0.2,
                vec![button(Pos2::new(592.0, 92.0), false)],
            )
            .0,
        );
        let mut output = frame(&ctx, &mut m, 0.22, vec![]).1;
        for i in 0..12 {
            let (c, o) = frame(&ctx, &mut m, 0.24 + i as f64 * 0.016, vec![]);
            commands.extend(c);
            output = o;
        }
        assert!(
            !commands
                .iter()
                .any(|c| activates(c) || matches!(c, Command::Favorite(_)))
        );
        assert!(m.favorites.applications.is_empty());
        assert_eq!(m.launcher_query, "volume");
        assert!(
            m.shell_text_focus,
            "Scrolling must retain the keyboard and constrained viewport"
        );
        assert!(output.shapes.iter().any(|s| matches!(&s.shape,egui::Shape::Text(t) if t.galley.job.text.contains("outputs") && s.clip_rect.intersects(Rect::from_min_size(t.pos,t.galley.size())))));
    }
}
