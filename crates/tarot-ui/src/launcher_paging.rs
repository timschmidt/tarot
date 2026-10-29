//! Page swipes follow the finger; vertical movement belongs to the page scroller.
use egui::{Context, Event, Id};
use tarot_core::{
    Command, Layout, ShellModel, View,
    launcher::{Action, Edit, PageId, Settings, Swipe, SwipePreview},
};

#[derive(Clone)]
struct Settle {
    preview: SwipePreview,
    selected: PageId,
    settings: Settings,
    start: f64,
    width: f32,
}
#[derive(Clone, Default)]
struct Memory {
    swipe: Swipe,
    scene: Option<tarot_core::favorite_drag::Scene>,
    settle: Option<Settle>,
    frame: Option<u64>,
    result: Frame,
}
#[derive(Clone, Default)]
pub(crate) struct Frame {
    pub preview: Option<SwipePreview>,
    pub consumed: bool,
    select: Option<PageId>,
}

pub(crate) fn begin(
    ctx: &Context,
    model: &ShellModel,
    layout: Layout,
    icon_drag: bool,
    ready: bool,
) -> Frame {
    let key = Id::new("launcher_paging");
    let mut m = ctx
        .data_mut(|d| d.get_temp::<Memory>(key))
        .unwrap_or_default();
    let frame = ctx.cumulative_frame_nr();
    if m.frame != Some(frame) {
        let (events, time, focused) = ctx.input(|i| (i.events.clone(), i.time, i.focused));
        let now = (time.max(0.0) * 1000.0) as u64;
        let scene = tarot_core::favorite_drag::Scene::new(model, layout);
        let enabled = ready
            && focused
            && model.view == View::Launcher
            && !model.editing_favorites
            && !model.launcher.search_open
            && !model.shell_text_focus
            && model.launcher.rename.is_none()
            && tarot_core::favorite_drag::Scene::allowed(model)
            && !icon_drag;
        let mut result = Frame::default();
        if !enabled || m.scene.as_ref().is_some_and(|s| *s != scene) {
            result.consumed = m.swipe.cancel();
        }
        if !enabled {
            m.settle = None;
        }
        if enabled {
            if events
                .iter()
                .any(|e| matches!(e, Event::Text(_) | Event::Paste(_)))
            {
                result.consumed |= m.swipe.cancel();
                m.settle = None;
            } else {
                for event in events {
                    match event {
                        Event::PointerGone => {
                            let cancelled = m.swipe.cancel();
                            result.consumed |= cancelled;
                            if cancelled {
                                m.settle = None;
                            }
                        }
                        Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            ..
                        } => {
                            // A new contact owns a new interaction, including
                            // a header/menu tap while the previous page settles.
                            m.settle = None;
                            m.swipe.cancel();
                            if crate::launcher::grid_bounds(model, layout, false).contains(pos) {
                                m.swipe.begin(
                                    (pos.x, pos.y),
                                    now,
                                    &model.launcher.settings,
                                    layout.width,
                                );
                            }
                        }
                        Event::PointerMoved(pos) => m.swipe.motion((pos.x, pos.y)),
                        Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: false,
                            ..
                        } => {
                            m.swipe.motion((pos.x, pos.y));
                            let preview = m.swipe.preview(&model.launcher.settings, layout.width);
                            let (consumed, selected) =
                                m.swipe.release(now, &model.launcher.settings, layout.width);
                            result.consumed |= consumed;
                            result.select = selected;
                            if let Some(preview) = preview.filter(|_| !model.reduced_motion) {
                                m.settle = Some(Settle {
                                    preview,
                                    selected: selected.unwrap_or(model.launcher.settings.selected),
                                    settings: model.launcher.settings.clone(),
                                    start: time,
                                    width: layout.width,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                result.preview = m.swipe.preview(&model.launcher.settings, layout.width);
                result.consumed |= m.swipe.horizontal();
                if result.preview.is_none()
                    && let Some(s) = &m.settle
                {
                    let mut expected = s.settings.clone();
                    expected.selected = s.selected;
                    if (model.launcher.settings == expected
                        || model.launcher.settings == s.settings)
                        && s.width == layout.width
                        && !model.reduced_motion
                    {
                        let progress = ((time - s.start)
                            / (tarot_core::design::LAUNCHER_MS as f64 / 1000.0))
                            .clamp(0.0, 1.0) as f32;
                        if progress < 1.0 {
                            let end = if s.selected == s.preview.page {
                                0.0
                            } else {
                                s.preview.offset.signum() * layout.width
                            };
                            let ease = 1.0 - (1.0 - progress).powi(3);
                            result.preview = Some(SwipePreview {
                                offset: s.preview.offset + (end - s.preview.offset) * ease,
                                ..s.preview
                            });
                            result.consumed = true;
                            ctx.request_repaint();
                        } else {
                            m.settle = None;
                        }
                    } else {
                        m.settle = None;
                    }
                }
            }
        }
        m.scene = Some(scene);
        m.frame = Some(frame);
        m.result = result;
    }
    let result = m.result.clone();
    ctx.data_mut(|d| d.insert_temp(key, m));
    result
}

pub(crate) fn finish(frame: &Frame, commands: &mut Vec<Command>) {
    if frame.consumed {
        commands.clear();
        if let Some(page) = frame.select {
            commands.push(Command::LauncherAction(Action::Edit(Edit::Select(page))));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Rect, Vec2};
    fn model() -> ShellModel {
        let mut m = ShellModel::default();
        m.apply(Command::Launcher);
        m.launcher
            .settings
            .edit(Edit::Create("Work".into()), &[])
            .unwrap();
        m.launcher
            .settings
            .edit(Edit::Select(PageId(1)), &[])
            .unwrap();
        for n in 0..36 {
            m.applications.push(tarot_core::Application {
                id: format!("app{n}.desktop"),
                name: format!("App {n}"),
                description: String::new(),
                icon: None,
                keywords: vec![],
                unavailable: None,
            });
        }
        m
    }
    fn render(
        ctx: &Context,
        m: &ShellModel,
        time: f64,
        events: Vec<Event>,
        discard: bool,
    ) -> Vec<Command> {
        crate::prepare_pointer_input(ctx, &events);
        let mut result = Vec::new();
        let _ = ctx.run(
            egui::RawInput {
                time: Some(time),
                events,
                focused: true,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(360.0, 720.0))),
                ..Default::default()
            },
            |ctx| {
                result = crate::show(
                    ctx,
                    m,
                    Layout {
                        width: 360.0,
                        height: 720.0,
                        keyboard_height: 0.0,
                    },
                );
                if discard && ctx.current_pass_index() == 0 {
                    ctx.request_discard("Verify one page release across passes");
                }
            },
        );
        result
    }
    fn button(x: f32, y: f32, pressed: bool) -> Event {
        Event::PointerButton {
            pos: Pos2::new(x, y),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        }
    }
    fn settle(ctx: &Context, m: &ShellModel) {
        render(ctx, m, 0.0, vec![], false);
        render(ctx, m, 0.01, vec![], false);
    }
    #[test]
    fn actual_horizontal_swipe_selects_once_and_animates_across_the_model_update() {
        for (reduced, touch) in [(false, false), (true, false), (false, true), (true, true)] {
            let ctx = Context::default();
            let mut m = model();
            m.reduced_motion = reduced;
            settle(&ctx, &m);
            render(
                &ctx,
                &m,
                0.1,
                vec![
                    Event::PointerMoved(Pos2::new(180.0, 150.0)),
                    button(180.0, 150.0, true),
                ],
                false,
            );
            let motion = render(
                &ctx,
                &m,
                0.2,
                vec![Event::PointerMoved(Pos2::new(70.0, 152.0))],
                true,
            );
            assert!(motion.is_empty(), "{motion:?}");
            let mut release_events = vec![button(70.0, 152.0, false)];
            if touch {
                release_events.push(Event::PointerGone);
            }
            let release = render(&ctx, &m, 0.3, release_events, true);
            assert_eq!(
                release,
                vec![Command::LauncherAction(Action::Edit(Edit::Select(PageId(
                    2
                ))))]
            );
            m.apply(release[0].clone());
            assert!(render(&ctx, &m, 0.4, vec![], false).is_empty());
            let memory = ctx
                .data_mut(|d| d.get_temp::<Memory>(Id::new("launcher_paging")))
                .unwrap();
            assert_eq!(memory.result.preview.is_some(), !reduced);
            assert!(render(&ctx, &m, 0.8, vec![], false).is_empty());
            assert!(
                ctx.data_mut(|d| d.get_temp::<Memory>(Id::new("launcher_paging")))
                    .unwrap()
                    .result
                    .preview
                    .is_none()
            );
        }
    }
    #[test]
    fn vertical_scroll_axis_and_canceled_horizontal_press_cannot_launch_or_change_page() {
        for canceled in [false, true] {
            let ctx = Context::default();
            let m = model();
            settle(&ctx, &m);
            render(
                &ctx,
                &m,
                0.1,
                vec![
                    Event::PointerMoved(Pos2::new(180.0, 350.0)),
                    button(180.0, 350.0, true),
                ],
                false,
            );
            let (x, y) = if canceled {
                (70.0, 350.0)
            } else {
                (180.0, 200.0)
            };
            render(
                &ctx,
                &m,
                0.2,
                vec![Event::PointerMoved(Pos2::new(x, y))],
                false,
            );
            let mut events = vec![];
            if canceled {
                crate::cancel_pointer_input(&mut events);
            } else {
                events.push(button(x, y, false));
            }
            let commands = render(&ctx, &m, 0.3, events, true);
            assert!(
                !commands
                    .iter()
                    .any(|c| matches!(c, Command::Launch(_) | Command::LauncherAction(_))),
                "{commands:?}"
            );
            assert_eq!(m.launcher.settings.selected, PageId(1));
        }
    }
    #[test]
    fn a_fresh_header_press_interrupts_page_settling_and_opens_the_menu() {
        let ctx = Context::default();
        let mut m = model();
        settle(&ctx, &m);
        render(
            &ctx,
            &m,
            0.1,
            vec![
                Event::PointerMoved(Pos2::new(180.0, 150.0)),
                button(180.0, 150.0, true),
            ],
            false,
        );
        render(
            &ctx,
            &m,
            0.2,
            vec![Event::PointerMoved(Pos2::new(70.0, 152.0))],
            false,
        );
        for command in render(
            &ctx,
            &m,
            0.3,
            vec![button(70.0, 152.0, false), Event::PointerGone],
            false,
        ) {
            m.apply(command);
        }
        assert_eq!(m.launcher.settings.selected, PageId(2));
        render(
            &ctx,
            &m,
            0.31,
            vec![
                Event::PointerMoved(Pos2::new(40.0, 16.0)),
                button(40.0, 16.0, true),
            ],
            false,
        );
        let commands = render(&ctx, &m, 0.4, vec![button(40.0, 16.0, false)], false);
        assert_eq!(
            commands,
            vec![Command::Menu(tarot_core::menu::Action::ToggleActivity)]
        );
        assert!(
            ctx.data_mut(|d| d.get_temp::<Memory>(Id::new("launcher_paging")))
                .unwrap()
                .result
                .preview
                .is_none()
        );
    }
}
