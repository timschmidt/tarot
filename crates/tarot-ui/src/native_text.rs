//! Normalize direct IME commits for the pinned egui/egui-winit 0.33 host.
//!
//! egui-winit forwards a Wayland commit without starting a composition when
//! no preedit was sent. TextEdit then compares the cursor with the previous
//! composition's cursor and can discard a valid second commit. Preserve real
//! preedit sessions, including their stale-cursor guard, and start only bare
//! commits. This belongs to the egui client host, not the compositor protocol.
use egui::{Context, Event, ImeEvent};
use std::sync::Arc;

#[derive(Clone, Default)]
struct State {
    composing: bool,
    frame_start: Option<(u64, bool)>,
}

/// Install once on an eframe context. Also applies to immediate child viewports.
pub fn install(ctx: &Context) {
    ctx.on_begin_pass("tarot-native-text", Arc::new(prepare_input));
}

fn prepare_input(ctx: &Context) {
    let id = egui::Id::new("tarot-native-text-state").with(ctx.viewport_id());
    let mut state = ctx
        .data_mut(|data| data.get_temp::<State>(id))
        .unwrap_or_default();
    let frame = ctx.cumulative_frame_nr();
    if let Some((_, composing)) = state.frame_start.filter(|(previous, _)| *previous == frame) {
        state.composing = composing;
    } else {
        state.frame_start = Some((frame, state.composing));
    }
    ctx.input_mut(|input| {
        if !input.focused {
            state.composing = false;
        }
        let mut had_composition = state.composing;
        let events = std::mem::take(&mut input.events);
        for event in events {
            match &event {
                Event::Ime(ImeEvent::Enabled | ImeEvent::Preedit(_)) => {
                    state.composing = true;
                    had_composition = true;
                }
                Event::Ime(ImeEvent::Disabled) => state.composing = false,
                Event::Ime(ImeEvent::Commit(_)) => {
                    if !had_composition {
                        input.events.push(Event::Ime(ImeEvent::Enabled));
                    }
                    state.composing = false;
                    had_composition = false;
                }
                _ => {}
            }
            input.events.push(event);
        }
    });
    ctx.data_mut(|data| data.insert_temp(id, state));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &Context, text: &mut String, events: Vec<Event>) {
        let input = egui::RawInput {
            focused: true,
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add(egui::TextEdit::singleline(text).id_salt("ime-field"))
                    .request_focus();
            });
        });
    }

    #[test]
    fn repeated_bare_commits_and_keyboard_backspace_edit_the_actual_field() {
        let ctx = Context::default();
        install(&ctx);
        let mut text = String::new();
        frame(&ctx, &mut text, vec![]);
        for part in ["q", "é", "🙂"] {
            frame(
                &ctx,
                &mut text,
                vec![Event::Ime(ImeEvent::Commit(part.into()))],
            );
        }
        assert_eq!(text, "qé🙂");
        frame(
            &ctx,
            &mut text,
            vec![Event::Key {
                key: egui::Key::Backspace,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        assert_eq!(text, "qé");
    }

    #[test]
    fn existing_preedit_selection_is_replaced_and_a_later_bare_commit_appends() {
        let ctx = Context::default();
        install(&ctx);
        let mut text = String::new();
        frame(&ctx, &mut text, vec![]);
        frame(
            &ctx,
            &mut text,
            vec![
                Event::Ime(ImeEvent::Enabled),
                Event::Ime(ImeEvent::Preedit("kan".into())),
            ],
        );
        assert_eq!(text, "kan");
        frame(
            &ctx,
            &mut text,
            vec![
                Event::Ime(ImeEvent::Disabled),
                Event::Ime(ImeEvent::Commit("漢".into())),
            ],
        );
        assert_eq!(text, "漢");
        frame(
            &ctx,
            &mut text,
            vec![Event::Ime(ImeEvent::Commit("🙂".into()))],
        );
        assert_eq!(text, "漢🙂");
    }

    #[test]
    fn moving_the_cursor_during_composition_retains_the_stale_commit_guard() {
        let ctx = Context::default();
        install(&ctx);
        let mut text = String::new();
        frame(&ctx, &mut text, vec![]);
        frame(
            &ctx,
            &mut text,
            vec![
                Event::Ime(ImeEvent::Enabled),
                Event::Ime(ImeEvent::Preedit("kan".into())),
            ],
        );
        let id = ctx.memory(|memory| memory.focused()).unwrap();
        let mut state = egui::TextEdit::load_state(&ctx, id).unwrap();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(3),
            )));
        state.store(&ctx, id);
        frame(
            &ctx,
            &mut text,
            vec![
                Event::Ime(ImeEvent::Disabled),
                Event::Ime(ImeEvent::Commit("漢".into())),
            ],
        );
        assert_eq!(
            text, "kan",
            "composition must not commit at a cursor the user moved"
        );
    }
}
