use crate::{favorite_drag, launcher_paging, theme};
use egui::{Align2, Color32, FontId, Id, Painter, Pos2, Rect, Sense, Vec2};
use tarot_core::{
    Application, Command, Layout, ShellModel,
    launcher::{Action, Edit, PageId},
};

fn action(commands: &mut Vec<Command>, value: Action) {
    commands.push(Command::LauncherAction(value));
}
pub(crate) fn text_id(model: &ShellModel) -> Id {
    if model.launcher.rename.is_some() {
        Id::new(("launcher_page_name", model.launcher.name_epoch))
    } else {
        Id::new("launcher_search")
    }
}
fn searching(model: &ShellModel) -> bool {
    model.launcher.search_open || (model.shell_text_focus && model.launcher.rename.is_none())
}
pub(crate) fn grid_bounds(model: &ShellModel, layout: Layout, drag: bool) -> Rect {
    let search = searching(model) && !drag;
    let compact = search && model.shell_text_focus && layout.height < 480.0;
    let top = if compact {
        84.0
    } else if search {
        154.0
    } else {
        96.0
    };
    let content = layout.content();
    let mut bottom = content.y + content.height - if compact { 4.0 } else { 12.0 };
    if drag {
        bottom = bottom.min(tarot_core::quick_launch::slot(layout, 0, None).y - 8.0)
    }
    if model.launch_feedback.is_some() {
        bottom -= 60.0
    }
    Rect::from_min_max(
        Pos2::new(if search { 16.0 } else { 0.0 }, top),
        Pos2::new(
            layout.width - if search { 16.0 } else { 0.0 },
            bottom.max(top),
        ),
    )
}

fn text_field(
    ui: &mut egui::Ui,
    model: &ShellModel,
    rect: Rect,
    text: &mut String,
    hint: &str,
    limit: usize,
) -> egui::Response {
    let id = text_id(model);
    ui.ctx().memory_mut(|m| {
        if model.shell_text_focus {
            m.request_focus(id)
        } else {
            m.surrender_focus(id)
        }
    });
    ui.put(
        rect,
        egui::TextEdit::singleline(text)
            .id(id)
            .hint_text(hint)
            .char_limit(limit),
    )
}

pub(crate) fn show(
    ui: &mut egui::Ui,
    painter: &Painter,
    model: &ShellModel,
    layout: Layout,
    drag: Option<&tarot_core::favorite_drag::Preview>,
    paging: &launcher_paging::Frame,
    commands: &mut Vec<Command>,
) -> bool {
    if model.editing_favorites {
        painter.text(
            Pos2::new(20.0, 67.0),
            Align2::LEFT_CENTER,
            "Quick Launch",
            FontId::proportional(22.0),
            Color32::WHITE,
        );
        if ui
            .put(
                Rect::from_min_size(
                    Pos2::new(layout.width - 124.0, 43.0),
                    Vec2::new(108.0, 48.0),
                ),
                egui::Button::new("Done"),
            )
            .clicked()
        {
            commands.push(Command::EditFavorites(false));
        }
        crate::quick_launch::editor(ui, model, layout, commands);
        return false;
    }
    if let Some(rename) = &model.launcher.rename {
        let compact = layout.height < 480.0;
        let title = model
            .launcher
            .notice
            .as_deref()
            .unwrap_or(if rename.page.is_some() {
                "Rename page"
            } else {
                "New page"
            });
        painter.text(
            Pos2::new(20.0, if compact { 58.0 } else { 67.0 }),
            Align2::LEFT_CENTER,
            title,
            FontId::proportional(if model.launcher.notice.is_some() {
                14.0
            } else {
                22.0
            }),
            Color32::WHITE,
        );
        let mut text = rename.text.clone();
        let id = text_id(model);
        let response = text_field(
            ui,
            model,
            Rect::from_min_size(
                Pos2::new(20.0, if compact { 88.0 } else { 104.0 }),
                Vec2::new(layout.width - 40.0, 44.0),
            ),
            &mut text,
            "Page name",
            64,
        );
        let first = ui.ctx().data_mut(|d| {
            let key = id.with("initialized");
            let first = !d.get_temp::<bool>(key).unwrap_or(false);
            d.insert_temp(key, true);
            first
        });
        if first
            && rename.page.is_some()
            && let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id)
        {
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::new(0),
                    egui::text::CCursor::new(text.chars().count()),
                )));
            state.store(ui.ctx(), id);
        }
        if response.changed() {
            action(commands, Action::Name(text));
        }
        if response.lost_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter)) {
            action(commands, Action::AcceptName);
            return true;
        }
        let y = if compact { 36.0 } else { 164.0 };
        let width = if compact {
            100.0
        } else {
            (layout.width - 48.0) / 2.0
        };
        let cancel_x = if compact { layout.width - 232.0 } else { 20.0 };
        let save_x = if compact {
            layout.width - 124.0
        } else {
            28.0 + width
        };
        if ui
            .put(
                Rect::from_min_size(Pos2::new(cancel_x, y), Vec2::new(width, 44.0)),
                egui::Button::new("Cancel"),
            )
            .clicked()
        {
            action(commands, Action::Dismiss);
        }
        if ui
            .put(
                Rect::from_min_size(Pos2::new(save_x, y), Vec2::new(width, 44.0)),
                egui::Button::new("Save name"),
            )
            .clicked()
        {
            action(commands, Action::AcceptName);
        }
        return response.has_focus();
    }
    if model.just_type_active() && drag.is_none() {
        return crate::just_type::show(ui, model, layout, commands);
    }
    let search = searching(model) && drag.is_none();
    let compact = search && model.shell_text_focus && layout.height < 480.0;
    let page = drag
        .map(|d| d.page)
        .or_else(|| paging.preview.map(|p| p.page))
        .unwrap_or(model.launcher.settings.selected);
    if !compact {
        let name = if search {
            "Search"
        } else {
            model
                .launcher
                .settings
                .page(page)
                .map_or("Applications", |p| p.name.as_str())
        };
        ui.put(
            Rect::from_min_size(Pos2::new(56.0, 40.0), Vec2::new(layout.width - 112.0, 48.0)),
            egui::Label::new(egui::RichText::new(name).size(22.0).color(Color32::WHITE)).truncate(),
        );
        if !search && drag.is_none() {
            for (delta, x, label) in [
                (-1, 4.0, "Previous Launcher page"),
                (1, layout.width - 48.0, "Next Launcher page"),
            ] {
                let enabled =
                    model.launcher.settings.neighbor(page, delta).is_some() && !paging.consumed;
                let response = ui.put(
                    Rect::from_min_size(Pos2::new(x, 42.0), Vec2::splat(44.0)),
                    egui::Button::new(
                        egui::RichText::new(if delta < 0 { "‹" } else { "›" })
                            .size(30.0)
                            .color(if enabled {
                                Color32::WHITE
                            } else {
                                Color32::from_gray(95)
                            }),
                    )
                    .frame(false)
                    .sense(if enabled {
                        Sense::click()
                    } else {
                        Sense::hover()
                    }),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label)
                });
                if response.clicked() && enabled {
                    action(commands, Action::Edit(Edit::Navigate(delta)));
                }
            }
        }
    }
    let mut focused = false;
    if search {
        let mut query = model.launcher_query.clone();
        let response = text_field(
            ui,
            model,
            Rect::from_min_size(
                Pos2::new(20.0, if compact { 36.0 } else { 96.0 }),
                Vec2::new(
                    layout.width - if compact { 80.0 } else { 40.0 },
                    if compact { 44.0 } else { 40.0 },
                ),
            ),
            &mut query,
            "Search applications",
            1024,
        );
        focused = response.has_focus();
        if response.changed() {
            commands.push(Command::SearchApplications(query));
        }
        if compact
            && ui
                .put(
                    Rect::from_min_size(Pos2::new(layout.width - 56.0, 36.0), Vec2::splat(44.0)),
                    egui::Button::new("×").frame(false),
                )
                .on_hover_text("Close search")
                .clicked()
        {
            action(commands, Action::Search(false));
        }
    } else if drag.is_none() && !model.launcher.menu_open {
        let typed = ui.ctx().input(|i| {
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
            action(commands, Action::Search(true));
            commands.push(Command::SearchApplications(typed));
        }
    }
    let bounds = grid_bounds(model, layout, drag.is_some());
    let mut layer = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
    layer.set_clip_rect(bounds.intersect(ui.clip_rect()));
    if model.launcher.menu_open || paging.consumed {
        layer.disable();
    }
    if let Some(p) = paging.preview {
        grid(
            &mut layer,
            model,
            layout,
            Grid {
                page: p.page,
                search: false,
                compact: false,
                drag: None,
                offset: p.offset,
            },
            commands,
        );
        if let Some(next) = p.neighbor {
            grid(
                &mut layer,
                model,
                layout,
                Grid {
                    page: next,
                    search: false,
                    compact: false,
                    drag: None,
                    offset: p.offset - p.offset.signum() * layout.width,
                },
                commands,
            );
        }
    } else {
        grid(
            &mut layer,
            model,
            layout,
            Grid {
                page,
                search,
                compact,
                drag,
                offset: 0.0,
            },
            commands,
        );
    }
    if let Some(feedback) = &model.launch_feedback {
        let bottom = if model.shell_text_focus {
            layout.keyboard().y - 12.0
        } else {
            layout.gesture().y - 12.0
        };
        let r = Rect::from_min_size(
            Pos2::new(20.0, bottom - 54.0),
            Vec2::new(layout.width - 40.0, 50.0),
        );
        painter.rect_filled(r, 8.0, Color32::from_black_alpha(180));
        ui.put(
            Rect::from_min_max(r.min + Vec2::splat(8.0), r.max - Vec2::new(44.0, 4.0)),
            egui::Label::new(&feedback.message).wrap(),
        );
        if ui
            .put(
                Rect::from_min_size(
                    Pos2::new(r.right() - 44.0, r.top() + 3.0),
                    Vec2::splat(44.0),
                ),
                egui::Button::new("×").frame(false),
            )
            .on_hover_text("Dismiss message")
            .clicked()
        {
            commands.push(Command::DismissLaunchFeedback);
        }
    }
    focused
}

struct Grid<'a> {
    page: PageId,
    search: bool,
    compact: bool,
    drag: Option<&'a tarot_core::favorite_drag::Preview>,
    offset: f32,
}
fn grid(
    ui: &mut egui::Ui,
    model: &ShellModel,
    layout: Layout,
    grid: Grid<'_>,
    commands: &mut Vec<Command>,
) {
    let Grid {
        page,
        search,
        compact,
        drag,
        offset,
    } = grid;
    let base = grid_bounds(model, layout, drag.is_some());
    if base.height() < 44.0 {
        return;
    }
    let bounds = base.translate(Vec2::new(offset, 0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bounds).id_salt((
        "launcher_page",
        page.0,
        search,
    )));
    child.set_clip_rect(base.intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing.y = 8.0;
    if !search {
        favorite_drag::register_page(&child, page, None, bounds);
    }
    let words: Vec<_> = model
        .launcher_query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect();
    let apps: Vec<_> = if search {
        model
            .applications
            .iter()
            .filter(|a| {
                let haystack =
                    format!("{} {} {}", a.name, a.description, a.keywords.join(" ")).to_lowercase();
                words.iter().all(|w| haystack.contains(w))
            })
            .collect()
    } else {
        model
            .launcher
            .settings
            .applications(page, &model.applications)
    };
    let row_height = if compact { 72.0 } else { 104.0 };
    let output=egui::ScrollArea::vertical().id_salt(("launcher_scroll",page.0,search))
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .scroll_source(egui::scroll_area::ScrollSource{drag:drag.is_none(),..Default::default()})
        .max_height(bounds.height()).show_rows(&mut child,row_height,apps.len().div_ceil(3).max(1),|ui,rows| {
            if apps.is_empty() {ui.add_space(12.0);ui.label(if search {"No matching applications"}else{"No applications on this page"});}
            let width=(bounds.width()-16.0)/3.0;
            egui::Grid::new(("launcher_grid",page.0,search)).num_columns(3).spacing(Vec2::new(8.0,8.0)).show(ui,|ui| {
                for (index,app) in apps.iter().enumerate().skip(rows.start*3).take(rows.len()*3) {
                    let response=ui.push_id(&app.id,|ui|ui.add_sized(Vec2::new(width,row_height),egui::Button::new("").frame(false))).inner;
                    response.widget_info(||egui::WidgetInfo::labeled(egui::WidgetType::Button,ui.is_enabled(),format!("Open {}",app.name)));
                    favorite_drag::register(ui,tarot_core::favorite_drag::Source::Launcher(app.id.clone()),response.rect);
                    if !search {favorite_drag::register_page(ui,page,Some(index),response.rect.expand(4.0));}
                    if drag.is_some_and(|d|matches!(d.target,Some(tarot_core::favorite_drag::Target::Page{page:p,at}) if p==page && at==index)) {
                        ui.painter().rect_stroke(response.rect.shrink(2.0),8.0,egui::Stroke::new(2.0_f32,theme::FOCUS),egui::StrokeKind::Inside);
                    }
                    paint_application(ui,app,&response,model,AppStyle { compact, dragged: drag.is_some_and(|d|d.source.id()==app.id), label: Color32::WHITE },commands);
                    if response.clicked() {commands.push(Command::Launch(app.id.clone()));}
                    response.on_hover_text(app.unavailable.as_deref().unwrap_or(&app.description));
                    if index%3==2 {ui.end_row();}
                }
            });
        });
    if output.content_size.y > output.inner_rect.height() + 1.0
        && output.state.offset.y + output.inner_rect.height() < output.content_size.y - 1.0
    {
        let at = Pos2::new(base.center().x, base.bottom() - 5.0);
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                at + Vec2::new(-6.0, -4.0),
                at + Vec2::new(6.0, -4.0),
                at + Vec2::new(0.0, 2.0),
            ],
            Color32::from_gray(180),
            egui::Stroke::NONE,
        ));
    }
}

pub(crate) struct AppStyle {
    pub compact: bool,
    pub dragged: bool,
    pub label: Color32,
}
pub(crate) fn paint_application(
    ui: &egui::Ui,
    app: &Application,
    response: &egui::Response,
    model: &ShellModel,
    style: AppStyle,
    commands: &mut Vec<Command>,
) {
    let AppStyle {
        compact,
        dragged,
        label: label_color,
    } = style;
    let opacity = if dragged { 0.3 } else { 1.0 };
    let center = Pos2::new(
        response.rect.center().x,
        response.rect.top() + if compact { 16.0 } else { 34.0 },
    );
    let tint = if app.unavailable.is_some() {
        Color32::from_gray(110)
    } else {
        Color32::WHITE.gamma_multiply(opacity)
    };
    let icon_rect = Rect::from_center_size(
        center,
        Vec2::splat(if compact {
            28.0
        } else {
            tarot_core::design::LAUNCHER_ICON
        }),
    );
    let original = theme::paint_icon(
        ui.ctx(),
        ui.painter(),
        &app.id,
        false,
        icon_rect,
        tint,
        model.calendar_day,
    );
    let native = !original
        && crate::native_icons::paint(ui.ctx(), ui.painter(), model, &app.id, icon_rect, tint);
    if !original && !native && ui.is_rect_visible(icon_rect) {
        crate::native_icons::request(model, &app.id, commands);
    }
    if !original && !native {
        let hue = app.id.bytes().fold(0u8, u8::wrapping_add);
        let color = if app.unavailable.is_some() {
            Color32::from_gray(80)
        } else {
            Color32::from_rgb(55 + hue % 80, 100 + hue % 90, 110 + hue % 75)
        };
        ui.painter().circle_filled(
            center,
            if compact { 14.0 } else { 24.0 },
            color.gamma_multiply(opacity),
        );
        ui.painter().text(
            center,
            Align2::CENTER_CENTER,
            app.name
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string(),
            FontId::proportional(if compact { 18.0 } else { 25.0 }),
            Color32::WHITE.gamma_multiply(opacity),
        );
    }
    let mut job = egui::text::LayoutJob::simple(
        app.name.clone(),
        FontId::proportional(14.0),
        label_color.gamma_multiply(opacity),
        response.rect.width() - 8.0,
    );
    job.wrap.max_rows = if compact { 1 } else { 2 };
    job.wrap.overflow_character = Some('…');
    let label = ui.painter().layout_job(job);
    let x = response.rect.center().x - label.size().x / 2.0;
    ui.painter()
        .with_clip_rect(response.rect.intersect(ui.clip_rect()))
        .galley(
            Pos2::new(x, response.rect.top() + if compact { 34.0 } else { 70.0 }),
            label,
            label_color,
        );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn landscape_name_controls_stay_above_the_keyboard_and_save_the_actual_field() {
        let ctx = egui::Context::default();
        let mut model = ShellModel::default();
        model.apply(Command::Launcher);
        model.apply(Command::LauncherAction(Action::BeginName(None)));
        let layout = Layout {
            width: 720.0,
            height: 360.0,
            notification_height: 0.0,
            keyboard_height: 180.0,
        };
        let render = |time, model: &ShellModel, events: Vec<egui::Event>| {
            crate::prepare_pointer_input(&ctx, &events);
            let mut commands = vec![];
            let output = ctx.run(
                egui::RawInput {
                    time: Some(time),
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(720.0, 360.0))),
                    focused: true,
                    events,
                    ..Default::default()
                },
                |ctx| {
                    commands = crate::show(ctx, model, layout);
                },
            );
            (output, commands)
        };
        render(0.0, &model, vec![]);
        render(0.01, &model, vec![]);
        for command in render(0.02, &model, vec![egui::Event::Text("sideways".into())]).1 {
            model.apply(command);
        }
        assert_eq!(model.launcher.rename.as_ref().unwrap().text, "sideways");
        let output = render(0.03, &model, vec![]).0;
        let text = output
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.job.text == "Save name" => Some(t),
                _ => None,
            })
            .unwrap();
        assert!(
            text.pos.y + text.galley.size().y <= layout.keyboard().y,
            "Name controls must not be covered by the keyboard"
        );
        let point = text.pos + text.galley.size() * 0.5;
        let event = |pressed| egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        render(
            0.04,
            &model,
            vec![egui::Event::PointerMoved(point), event(true)],
        );
        let mut effects = vec![];
        for command in render(0.05, &model, vec![event(false)]).1 {
            effects.extend(model.apply(command));
        }
        assert!(matches!(
            effects.as_slice(),
            [tarot_core::Effect::Launcher(_)]
        ));
        assert_eq!(
            model
                .launcher
                .settings
                .page(model.launcher.settings.selected)
                .unwrap()
                .name,
            "sideways"
        );
    }
}
