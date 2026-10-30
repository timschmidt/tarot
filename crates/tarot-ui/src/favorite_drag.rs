//! egui adapter for the pure favorite gesture. Hit regions come from painted widgets.
use egui::{Color32, Context, Id, Pos2, Rect, Vec2};
use tarot_core::{
    Command, Layout, ShellModel,
    favorite_drag::{Gesture, Preview, Scene, Source, Target},
};

fn key() -> Id {
    Id::new("favorite_drag")
}

#[derive(Clone)]
struct Hit {
    source: Source,
    rect: Rect,
}

#[derive(Clone, Default)]
struct Memory {
    gesture: Gesture,
    hits: Vec<Hit>,
    targets: Vec<(tarot_core::launcher::PageId, usize, Rect)>,
    areas: Vec<(tarot_core::launcher::PageId, Rect)>,
    scene: Option<Scene>,
    frame: Option<u64>,
    result: Frame,
    blocked: bool,
    pressed_launcher: bool,
}

#[derive(Clone, Default)]
pub(crate) struct Frame {
    pub preview: Option<Preview>,
    pub pending: bool,
    consumed: bool,
    edit: Option<tarot_core::quick_launch::Edit>,
    page_edit: Option<tarot_core::launcher::Edit>,
}

fn target(
    memory: &Memory,
    pos: Pos2,
    model: &ShellModel,
) -> Option<(tarot_core::launcher::PageId, usize)> {
    let page = memory.gesture.page()?;
    memory
        .targets
        .iter()
        .rev()
        .find(|(p, _, r)| *p == page && r.contains(pos))
        .map(|(p, at, _)| (*p, *at))
        .or_else(|| {
            memory
                .areas
                .iter()
                .find(|(p, r)| *p == page && r.contains(pos))
                .map(|_| {
                    (
                        page,
                        model
                            .launcher
                            .settings
                            .applications(page, &model.applications)
                            .len(),
                    )
                })
        })
}

pub(crate) fn begin(ctx: &Context, model: &ShellModel, layout: Layout) -> Frame {
    let mut memory = ctx
        .data_mut(|data| data.get_temp::<Memory>(key()))
        .unwrap_or_default();
    let frame = ctx.cumulative_frame_nr();
    if memory.frame != Some(frame) {
        let (events, seconds, focused) =
            ctx.input(|input| (input.events.clone(), input.time, input.focused));
        let now = (seconds.max(0.0) * 1000.0) as u64;
        let mut result = Frame {
            consumed: memory.blocked,
            ..Default::default()
        };
        memory.gesture.reconcile(model, layout);
        if !focused {
            result.consumed |= memory.gesture.active();
            memory.gesture.cancel();
            memory.blocked = false;
            memory.pressed_launcher = false;
        } else {
            for event in events {
                match event {
                    egui::Event::PointerGone => {
                        result.consumed |= memory.gesture.active();
                        memory.gesture.cancel();
                        memory.blocked = false;
                        memory.pressed_launcher = false;
                    }
                    egui::Event::PointerMoved(pos) => memory.gesture.motion((pos.x, pos.y)),
                    egui::Event::MouseWheel { .. } => {
                        memory.blocked |= memory.gesture.in_progress();
                        memory.gesture.cancel();
                    }
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        pressed: true,
                        ..
                    } => {
                        memory.blocked |= memory.gesture.in_progress();
                        memory.gesture.cancel();
                    }
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        ..
                    } => {
                        if !memory.blocked
                            && memory.scene.as_ref() == Some(&Scene::new(model, layout))
                            && !tarot_core::notifications::overlay_contains(
                                model, layout, pos.x, pos.y,
                            )
                            && let Some(hit) =
                                memory.hits.iter().rev().find(|hit| hit.rect.contains(pos))
                        {
                            memory.pressed_launcher = matches!(hit.source, Source::Launcher(_));
                            memory.gesture.begin(
                                hit.source.clone(),
                                (pos.x, pos.y),
                                now,
                                model,
                                layout,
                            );
                        }
                    }
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        ..
                    } => {
                        memory.gesture.motion((pos.x, pos.y));
                        memory.gesture.tick(now);
                        let target = target(&memory, pos, model);
                        memory.gesture.page_target(target);
                        let release = memory.gesture.release(now, model, layout);
                        result.consumed |= memory.blocked || release.consumed;
                        result.edit = release.edit;
                        result.page_edit = release.page_edit;
                        memory.blocked = false;
                        memory.pressed_launcher = false;
                    }
                    _ => {}
                }
            }
            memory.gesture.tick(now);
            if let Some(preview) = memory.gesture.preview(model, layout) {
                let target = target(
                    &memory,
                    Pos2::new(preview.finger.0, preview.finger.1),
                    model,
                );
                memory.gesture.page_target(target);
            }
            memory.blocked |= memory.gesture.active();
            result.consumed |= memory.blocked;
            result.preview = memory.gesture.preview(model, layout);
        }
        // Keep even an unavailable result under the pressed finger until
        // release; collapsing the keyboard earlier would move that button.
        result.pending =
            !memory.blocked && (memory.gesture.in_progress() || memory.pressed_launcher);
        if let Some(delay) = memory.gesture.wake_after(now) {
            ctx.request_repaint_after(delay);
        }
        memory.result = result;
        memory.frame = Some(frame);
    }
    // Every egui pass replaces the hit map; events and release intents are
    // evaluated once per frame, and the final pass retains the same outcome.
    memory.hits.clear();
    memory.targets.clear();
    memory.areas.clear();
    memory.scene = Some(Scene::new(model, layout));
    let result = memory.result.clone();
    ctx.data_mut(|data| data.insert_temp(key(), memory));
    result
}

pub(crate) fn register_page(
    ui: &egui::Ui,
    page: tarot_core::launcher::PageId,
    at: Option<usize>,
    rect: Rect,
) {
    let rect = rect.intersect(ui.clip_rect());
    if ui.is_enabled() && rect.is_positive() {
        ui.ctx().data_mut(|data| {
            if let Some(mut m) = data.get_temp::<Memory>(key()) {
                if let Some(at) = at {
                    m.targets.push((page, at, rect));
                } else {
                    m.areas.push((page, rect));
                }
                data.insert_temp(key(), m);
            }
        });
    }
}

pub(crate) fn register(ui: &egui::Ui, source: Source, rect: Rect) {
    let rect = rect.intersect(ui.clip_rect());
    if ui.is_enabled() && rect.is_positive() {
        ui.ctx().data_mut(|data| {
            if let Some(mut memory) = data.get_temp::<Memory>(key()) {
                memory.hits.push(Hit { source, rect });
                data.insert_temp(key(), memory);
            }
        });
    }
}

pub(crate) fn finish(
    ctx: &Context,
    frame: &Frame,
    model: &ShellModel,
    layout: Layout,
    commands: &mut Vec<Command>,
) {
    if frame.consumed {
        commands.clear();
        ctx.memory_mut(|memory| memory.surrender_focus(Id::new("launcher_search")));
        if model.shell_text_focus {
            commands.push(Command::ShellTextFocus(false));
        }
        if let Some(edit) = &frame.edit {
            commands.push(Command::Favorite(edit.clone()));
        }
        if let Some(edit) = &frame.page_edit {
            commands.push(Command::LauncherAction(tarot_core::launcher::Action::Edit(
                edit.clone(),
            )));
        }
    }
    let Some(preview) = &frame.preview else {
        return;
    };
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::new("favorite_drag_overlay"),
    ));
    if matches!(preview.source, Source::Dock { .. }) {
        let r = tarot_core::favorite_drag::removal(layout);
        let rect = Rect::from_min_size(Pos2::new(r.x, r.y), Vec2::new(r.width, r.height));
        painter.rect_filled(
            rect,
            8.0,
            if preview.target == Some(Target::Remove) {
                Color32::from_rgb(150, 45, 45)
            } else {
                Color32::from_rgba_unmultiplied(30, 35, 40, 240)
            },
        );
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Remove from Quick Launch",
            egui::FontId::proportional(15.0),
            Color32::WHITE,
        );
    }
    let half = tarot_core::design::ICON * 0.56;
    let center = Pos2::new(
        preview
            .finger
            .0
            .clamp(half + 4.0, (layout.width - half - 4.0).max(half + 4.0)),
        (preview.finger.1 - 20.0).clamp(
            Layout::STATUS + half + 4.0,
            (layout.gesture().y - half - 10.0).max(Layout::STATUS + half + 4.0),
        ),
    );
    let rect = Rect::from_center_size(center, Vec2::splat(tarot_core::design::ICON * 1.12));
    painter.circle_filled(
        center + Vec2::new(0.0, 6.0),
        34.0,
        Color32::from_black_alpha(100),
    );
    crate::quick_launch::paint_drag_icon(ctx, &painter, preview.source.id(), rect, model);
    let label = match preview.target {
        Some(Target::Full(_)) => "Quick Launch is full",
        Some(Target::Remove) => "Release to remove favorite",
        Some(Target::Place(_)) => "Release to place favorite",
        Some(Target::Unavailable(_)) => "This application cannot be a favorite",
        Some(Target::Page { .. }) => "Release to place application",
        None => "Release outside to cancel",
    };
    let mut at = Pos2::new(layout.width / 2.0, quick_launch_top(layout) - 70.0);
    let galley = painter.layout_no_wrap(
        label.into(),
        egui::FontId::proportional(15.0),
        Color32::WHITE,
    );
    let label_size = galley.size() + Vec2::new(16.0, 12.0);
    if Rect::from_center_size(at, label_size).intersects(rect.expand(8.0)) {
        let above = rect.top() - label_size.y / 2.0 - 8.0;
        at.y = if above - label_size.y / 2.0 >= Layout::STATUS + 4.0 {
            above
        } else {
            rect.bottom() + label_size.y / 2.0 + 8.0
        };
    }
    let label_rect = Rect::from_center_size(at, label_size);
    painter.rect_filled(label_rect, 6.0, Color32::from_black_alpha(230));
    painter.galley(label_rect.min + Vec2::new(8.0, 6.0), galley, Color32::WHITE);
}

fn quick_launch_top(layout: Layout) -> f32 {
    tarot_core::quick_launch::slot(layout, 0, None).y
}

#[cfg(test)]
mod tests {
    use super::*;
    use tarot_core::{Application, Effect, View, quick_launch::Edit};
    fn model() -> ShellModel {
        let mut m = ShellModel::default();
        m.apply(Command::Launcher);
        m.favorites_saved = true;
        for n in 0..5 {
            m.applications.push(Application {
                id: format!("app{n}.desktop"),
                name: format!("App {n}"),
                description: String::new(),
                icon: None,
                keywords: vec![],
                unavailable: None,
            });
        }
        m.favorites.applications = vec!["app0.desktop".into(), "app1.desktop".into()];
        m
    }
    fn layout() -> Layout {
        Layout {
            width: 360.0,
            height: 720.0,
            notification_height: 0.0,
            keyboard_height: 0.0,
        }
    }
    fn render(
        ctx: &Context,
        m: &ShellModel,
        time: f64,
        events: Vec<egui::Event>,
        discard: bool,
    ) -> Vec<Command> {
        crate::prepare_pointer_input(ctx, &events);
        let mut commands = Vec::new();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 720.0))),
                time: Some(time),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| {
                commands = crate::show(ctx, m, layout());
                if discard && ctx.current_pass_index() == 0 {
                    ctx.request_discard("Verify favorite release across passes");
                }
            },
        );
        commands
    }
    fn button(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn source(ctx: &Context) -> Pos2 {
        ctx.data_mut(|data| data.get_temp::<Memory>(key()))
            .unwrap()
            .hits
            .iter()
            .find(|h| h.source == Source::Launcher("app2.desktop".into()))
            .unwrap()
            .rect
            .center()
    }
    fn active(ctx: &Context) -> bool {
        ctx.data_mut(|data| data.get_temp::<Memory>(key()))
            .unwrap()
            .result
            .preview
            .is_some()
    }
    fn arm(ctx: &Context, m: &ShellModel) -> Pos2 {
        render(ctx, m, 0.0, vec![], false);
        render(ctx, m, 0.01, vec![], false);
        let pos = source(ctx);
        render(
            ctx,
            m,
            0.1,
            vec![egui::Event::PointerMoved(pos), button(pos, true)],
            false,
        );
        let commands = render(ctx, m, 0.6, vec![], false);
        assert!(active(ctx));
        assert!(
            !commands
                .iter()
                .any(|c| matches!(c, Command::Launch(_) | Command::Favorite(_)))
        );
        pos
    }
    #[test]
    fn compact_search_keeps_a_tappable_result_stable_through_press_and_failure() {
        for unavailable in [false, true] {
            let ctx = Context::default();
            let mut m = model();
            if unavailable {
                m.applications[2].unavailable = Some("Fixture unavailable".into());
            }
            m.shell_text_focus = true;
            ctx.memory_mut(|memory| memory.request_focus(Id::new("launcher_search")));
            let render = |time, events: Vec<egui::Event>, m: &ShellModel| {
                crate::prepare_pointer_input(&ctx, &events);
                let mut commands = Vec::new();
                let _ = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(720.0, 360.0))),
                        time: Some(time),
                        events,
                        focused: true,
                        ..Default::default()
                    },
                    |ctx| {
                        commands = crate::show(
                            ctx,
                            m,
                            Layout {
                                width: 720.0,
                                height: 360.0,
                                notification_height: 0.0,
                                keyboard_height: 180.0,
                            },
                        );
                    },
                );
                commands
            };
            render(0.0, vec![], &m);
            render(0.01, vec![], &m);
            let point = source(&ctx);
            let hit = ctx
                .data_mut(|data| data.get_temp::<Memory>(key()))
                .unwrap()
                .hits
                .into_iter()
                .find(|h| h.source == Source::Launcher("app2.desktop".into()))
                .unwrap();
            assert!(hit.rect.height() >= 44.0 && hit.rect.bottom() <= 144.0);
            let commands = render(
                0.1,
                vec![egui::Event::PointerMoved(point), button(point, true)],
                &m,
            );
            assert!(!commands.contains(&Command::ShellTextFocus(false)));
            assert_eq!(source(&ctx), point);
            let commands = render(0.2, vec![button(point, false)], &m);
            assert_eq!(
                commands
                    .iter()
                    .filter(|c| matches!(c, Command::Launch(_)))
                    .cloned()
                    .collect::<Vec<_>>(),
                vec![Command::Launch("app2.desktop".into())]
            );
            m.apply(Command::Launch("app2.desktop".into()));
            render(0.3, vec![], &m);
            assert!(
                ctx.data_mut(|data| data.get_temp::<Memory>(key()))
                    .unwrap()
                    .hits
                    .is_empty()
            );
            let close = Pos2::new(678.0, 111.0);
            render(
                0.4,
                vec![egui::Event::PointerMoved(close), button(close, true)],
                &m,
            );
            let commands = render(0.5, vec![button(close, false)], &m);
            assert!(commands.contains(&Command::DismissLaunchFeedback));
        }
    }

    #[test]
    fn painted_hold_survives_late_icon_and_emits_one_intent_with_an_extra_pass() {
        let ctx = Context::default();
        let mut m = model();
        m.applications[2].icon = Some("app2".into());
        assert!(matches!(
            m.apply(Command::LoadIcon("app2.desktop".into())).as_slice(),
            [Effect::LoadIcon { .. }]
        ));
        arm(&ctx, &m);
        m.icons.complete(
            "app2.desktop".into(),
            tarot_core::icons::Raster::new([1, 1], vec![0, 255, 0, 255]),
        );
        let before = m.favorites.clone();
        let target = Pos2::new(38.0, 645.0);
        render(
            &ctx,
            &m,
            0.7,
            vec![egui::Event::PointerMoved(target)],
            false,
        );
        assert_eq!(m.favorites, before);
        let commands = render(&ctx, &m, 0.8, vec![button(target, false)], true);
        let edits: Vec<_> = commands
            .iter()
            .filter(|c| matches!(c, Command::Favorite(_)))
            .cloned()
            .collect();
        assert_eq!(
            edits,
            vec![Command::Favorite(Edit::Place {
                id: "app2.desktop".into(),
                at: 0
            })]
        );
        assert!(!commands.iter().any(|c| matches!(c, Command::Launch(_))));
        assert!(matches!(
            m.apply(edits[0].clone()).as_slice(),
            [Effect::Favorites(_)]
        ));
        assert!(
            !render(&ctx, &m, 0.9, vec![], false)
                .iter()
                .any(|c| matches!(c, Command::Favorite(_)))
        );
        assert_eq!(
            m.favorites.applications,
            ["app2.desktop", "app0.desktop", "app1.desktop"]
        );
    }
    #[test]
    fn actual_cancel_events_and_modal_changes_retire_the_held_launcher_press() {
        for modal in [false, true] {
            let ctx = Context::default();
            let mut m = model();
            let pos = arm(&ctx, &m);
            let mut events = Vec::new();
            if modal {
                m.system_menu_open = true;
            } else {
                crate::cancel_pointer_input(&mut events);
            }
            let commands = render(&ctx, &m, 0.7, events, false);
            assert!(!active(&ctx));
            assert!(
                !commands
                    .iter()
                    .any(|c| matches!(c, Command::Launch(_) | Command::Favorite(_)))
            );
            let commands = render(&ctx, &m, 0.8, vec![button(pos, false)], false);
            assert!(
                !commands
                    .iter()
                    .any(|c| matches!(c, Command::Launch(_) | Command::Favorite(_)))
            );
        }
    }
    #[test]
    fn movement_before_hold_remains_scroll_and_fixed_launcher_cannot_receive_a_drop() {
        let ctx = Context::default();
        let m = model();
        render(&ctx, &m, 0.0, vec![], false);
        render(&ctx, &m, 0.01, vec![], false);
        let pos = source(&ctx);
        render(
            &ctx,
            &m,
            0.1,
            vec![egui::Event::PointerMoved(pos), button(pos, true)],
            false,
        );
        render(
            &ctx,
            &m,
            0.15,
            vec![egui::Event::PointerMoved(pos + Vec2::new(0.0, 60.0))],
            false,
        );
        render(&ctx, &m, 0.6, vec![], false);
        assert!(!active(&ctx));
        let commands = render(
            &ctx,
            &m,
            0.7,
            vec![button(pos + Vec2::new(0.0, 60.0), false)],
            false,
        );
        assert!(
            !commands
                .iter()
                .any(|c| matches!(c, Command::Launch(_) | Command::Favorite(_)))
        );
        let ctx = Context::default();
        arm(&ctx, &m);
        let target = Pos2::new(320.0, 645.0);
        render(
            &ctx,
            &m,
            0.7,
            vec![egui::Event::PointerMoved(target)],
            false,
        );
        let commands = render(&ctx, &m, 0.8, vec![button(target, false)], false);
        assert!(commands.is_empty());
        assert_eq!(m.view, View::Launcher);
    }
    #[test]
    fn actual_icon_hold_edge_pause_and_empty_page_drop_preserve_favorite_identity() {
        use tarot_core::launcher::{Edit as PageEdit, PageId};
        let ctx = Context::default();
        let mut m = model();
        m.launcher
            .settings
            .edit(PageEdit::Create("Work".into()), &m.applications)
            .unwrap();
        m.launcher
            .settings
            .edit(PageEdit::Select(PageId(1)), &m.applications)
            .unwrap();
        let before = m.launcher.settings.clone();
        let favorites = m.favorites.clone();
        arm(&ctx, &m);
        let edge = Pos2::new(350.0, 300.0);
        render(&ctx, &m, 0.8, vec![egui::Event::PointerMoved(edge)], false);
        render(&ctx, &m, 1.41, vec![], true);
        assert_eq!(
            ctx.data_mut(|d| d.get_temp::<Memory>(key()))
                .unwrap()
                .result
                .preview
                .unwrap()
                .page,
            PageId(2)
        );
        assert_eq!(m.launcher.settings, before);
        let drop = Pos2::new(180.0, 150.0);
        render(&ctx, &m, 1.5, vec![egui::Event::PointerMoved(drop)], false);
        let commands = render(&ctx, &m, 1.6, vec![button(drop, false)], true);
        assert_eq!(commands.len(), 1, "{commands:?}");
        let effects = m.apply(commands[0].clone());
        assert!(matches!(effects.as_slice(), [Effect::Launcher(_)]));
        assert_eq!(
            m.launcher.settings.applications(PageId(2), &m.applications)[0].id,
            "app2.desktop"
        );
        assert_eq!(m.favorites, favorites);
        assert!(
            !render(&ctx, &m, 1.7, vec![], false)
                .iter()
                .any(|c| matches!(c, Command::LauncherAction(_) | Command::Launch(_)))
        );
    }
    #[test]
    fn same_page_reorder_then_edge_drop_keeps_the_empty_page_reachable() {
        use tarot_core::launcher::{Edit as PageEdit, PageId};
        let ctx = Context::default();
        let mut m = model();
        for n in 5..36 {
            m.applications.push(Application {
                id: format!("app{n}.desktop"),
                name: format!("App {n}"),
                description: String::new(),
                icon: None,
                keywords: vec![],
                unavailable: None,
            });
        }
        m.launcher
            .settings
            .edit(PageEdit::Create("Work".into()), &m.applications)
            .unwrap();
        m.launcher
            .settings
            .edit(PageEdit::Select(PageId(1)), &m.applications)
            .unwrap();
        arm(&ctx, &m);
        let target = Pos2::new(180.0, 148.0);
        render(
            &ctx,
            &m,
            0.8,
            vec![egui::Event::PointerMoved(target)],
            false,
        );
        let commands = render(&ctx, &m, 0.9, vec![button(target, false)], false);
        assert!(matches!(commands.as_slice(), [Command::LauncherAction(_)]));
        for command in commands {
            m.apply(command);
        }
        let mut canceled = vec![];
        crate::cancel_pointer_input(&mut canceled);
        render(&ctx, &m, 0.95, canceled, false);
        render(&ctx, &m, 1.0, vec![], false);
        let at = source(&ctx);
        render(
            &ctx,
            &m,
            1.1,
            vec![egui::Event::PointerMoved(at), button(at, true)],
            false,
        );
        render(&ctx, &m, 1.7, vec![], false);
        render(
            &ctx,
            &m,
            1.8,
            vec![egui::Event::PointerMoved(Pos2::new(350.0, 300.0))],
            false,
        );
        render(&ctx, &m, 2.41, vec![], false);
        let memory = ctx.data_mut(|d| d.get_temp::<Memory>(key())).unwrap();
        assert_eq!(memory.result.preview.as_ref().unwrap().page, PageId(2));
        assert!(
            memory
                .areas
                .iter()
                .any(|(page, rect)| *page == PageId(2) && rect.contains(Pos2::new(180.0, 150.0))),
            "Empty page targets: {:?}",
            memory.areas
        );
        let drop = Pos2::new(180.0, 150.0);
        render(&ctx, &m, 2.5, vec![egui::Event::PointerMoved(drop)], false);
        let commands = render(&ctx, &m, 2.6, vec![button(drop, false)], true);
        assert_eq!(
            commands,
            vec![Command::LauncherAction(tarot_core::launcher::Action::Edit(
                PageEdit::Place {
                    application: "app2.desktop".into(),
                    page: PageId(2),
                    at: 0
                }
            ))]
        );
    }
}
