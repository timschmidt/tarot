pub use tarot_core::KeyboardAction as Action;
pub mod terminal;

/// Retain the order of taps that arrived while the native keyboard waited for
/// a frame callback. egui widgets expose one click per frame: giving them two
/// completed taps together silently loses input, including page/modifier keys.
/// Stop at the first release so later motion cannot move this click's target.
/// The caller must schedule another frame while events remain queued.
pub fn take_pointer_frame(events: &mut Vec<egui::Event>) -> Vec<egui::Event> {
    let end = events
        .iter()
        .position(|event| {
            matches!(
                event,
                egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    ..
                }
            )
        })
        .map_or(events.len(), |index| index + 1);
    events.drain(..end).collect()
}

#[derive(Clone, Default)]
pub struct KeyboardUi {
    shift: bool,
    symbols: bool,
    symbols_more: bool,
    frame_start: Option<(u64, bool, bool, bool)>,
}

impl KeyboardUi {
    pub fn reset(&mut self) {
        self.shift = false;
        self.symbols = false;
        self.symbols_more = false;
        self.frame_start = None;
    }

    pub fn show(&mut self, ctx: &egui::Context, numeric: bool, secure: bool) -> Vec<Action> {
        let mut actions = Vec::new();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(38, 40, 44))
                    .inner_margin(5.0),
            )
            .show(ctx, |ui| {
                actions = self.show_in_ui(ui, numeric, secure);
            });
        actions
    }

    pub fn show_in_ui(&mut self, ui: &mut egui::Ui, numeric: bool, secure: bool) -> Vec<Action> {
        self.show_layout(
            ui,
            numeric,
            if secure { "Private entry" } else { "English" },
        )
    }

    pub fn show_terminal(
        &mut self,
        ctx: &egui::Context,
        controls: &mut terminal::Controls,
    ) -> (Vec<Action>, Vec<terminal::Key>) {
        let mut actions = Vec::new();
        let mut keys = Vec::new();
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(38, 40, 44))
                    .inner_margin(5.0),
            )
            .show(ctx, |ui| {
                keys = controls.show(ui);
                actions = self.show_layout(ui, false, controls.label());
            });
        (actions, keys)
    }

    fn show_layout(&mut self, ui: &mut egui::Ui, numeric: bool, label: &str) -> Vec<Action> {
        crate::theme::install(ui.ctx());
        let frame = ui.ctx().cumulative_frame_nr();
        if let Some((previous, shift, symbols, symbols_more)) = self
            .frame_start
            .filter(|(previous, _, _, _)| *previous == frame)
        {
            let _ = previous;
            self.shift = shift;
            self.symbols = symbols;
            self.symbols_more = symbols_more;
        } else {
            self.frame_start = Some((frame, self.shift, self.symbols, self.symbols_more));
        }
        let mut actions = Vec::new();
        key_style(ui);
        ui.label(
            egui::RichText::new(label)
                .size(12.0)
                .color(egui::Color32::LIGHT_GRAY),
        );
        let row_height = ((ui.available_height() - 12.0) / 4.0).max(20.0);
        if numeric {
            for row in ["123", "456", "789"] {
                self.row(ui, row, row_height, &mut actions);
            }
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 12.0) / 4.0;
                for (label, action) in [
                    ("Hide", Action::Hide),
                    ("0", Action::Text("0".into())),
                    ("Bksp", Action::Backspace),
                    ("Enter", Action::Enter),
                ] {
                    if key(ui, label, w, row_height) {
                        actions.push(action);
                    }
                }
            });
        } else {
            self.row(
                ui,
                if self.symbols && self.symbols_more {
                    "`~^|\\[]{}$"
                } else if self.symbols {
                    "1234567890"
                } else {
                    "qwertyuiop"
                },
                row_height,
                &mut actions,
            );
            self.row(
                ui,
                if self.symbols && self.symbols_more {
                    "+=<>:;'\","
                } else if self.symbols {
                    "@#€%&*()-"
                } else {
                    "asdfghjkl"
                },
                row_height,
                &mut actions,
            );
            ui.horizontal(|ui| {
                let width = ui.available_width();
                let side = width * 0.125;
                let letter = (width - side * 2.0 - 32.0) / 7.0;
                if key(
                    ui,
                    if self.symbols {
                        if self.symbols_more { "123" } else { "#+=" }
                    } else if self.shift {
                        "SHIFT"
                    } else {
                        "Shift"
                    },
                    side,
                    row_height,
                ) {
                    if self.symbols {
                        self.symbols_more = !self.symbols_more;
                    } else {
                        self.shift = !self.shift;
                    }
                }
                for ch in (if self.symbols && self.symbols_more {
                    "É🙂?!/_."
                } else if self.symbols {
                    "é🙂?!/_."
                } else {
                    "zxcvbnm"
                })
                .chars()
                {
                    let text = self.character(ch);
                    if key(ui, &text, letter, row_height) {
                        actions.push(Action::Text(text));
                        self.shift = false;
                    }
                }
                if key(ui, "Bksp", side, row_height) {
                    actions.push(Action::Backspace);
                }
            });
            ui.horizontal(|ui| {
                let w = ui.available_width();
                if key(
                    ui,
                    if self.symbols { "ABC" } else { "123" },
                    w * 0.15,
                    row_height,
                ) {
                    self.symbols = !self.symbols;
                    self.symbols_more = false;
                    self.shift = false;
                }
                if key(ui, "Space", w * 0.45 - 4.0, row_height) {
                    actions.push(Action::Text(" ".into()));
                }
                if key(ui, "Enter", w * 0.18 - 4.0, row_height) {
                    actions.push(Action::Enter);
                }
                if key(ui, "Hide", w * 0.22 - 4.0, row_height) {
                    actions.push(Action::Hide);
                }
            });
        }

        actions
    }

    fn character(&self, ch: char) -> String {
        if self.shift {
            ch.to_uppercase().collect()
        } else {
            ch.to_string()
        }
    }
    fn row(&mut self, ui: &mut egui::Ui, row: &str, height: f32, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            let count = row.chars().count() as f32;
            let width = (ui.available_width() - 4.0 * (count - 1.0)) / count;
            for ch in row.chars() {
                let text = self.character(ch);
                if key(ui, &text, width, height) {
                    actions.push(Action::Text(text));
                    self.shift = false;
                }
            }
        });
    }
}

fn key_style(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
    ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::from_rgb(220, 221, 224);
    ui.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(220, 221, 224);
    ui.visuals_mut().widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(20, 23, 26);
    ui.visuals_mut().widgets.hovered.fg_stroke.color = egui::Color32::BLACK;
    ui.visuals_mut().widgets.hovered.bg_fill = egui::Color32::from_rgb(245, 246, 248);
    ui.visuals_mut().widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(245, 246, 248);
    ui.visuals_mut().widgets.active.bg_fill = egui::Color32::from_rgb(150, 185, 220);
}

fn key(ui: &mut egui::Ui, label: &str, width: f32, height: f32) -> bool {
    let label = if label == "🙂" { ":)" } else { label };
    ui.add_sized(
        [width, height],
        egui::Button::new(
            egui::RichText::new(label).size(if label.chars().count() > 2 {
                14.0
            } else {
                21.0
            }),
        )
        .corner_radius(5.0),
    )
    .clicked()
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use egui::{Context, Event, Modifiers, PointerButton, Pos2, RawInput, Rect};

    fn tap(events: &mut Vec<Event>, x: f32, y: f32) {
        let pos = egui::pos2(x, y);
        events.push(Event::PointerMoved(pos));
        for pressed in [true, false] {
            events.push(Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            });
        }
        // Native wl_touch emits this after each completed contact.
        events.push(Event::PointerGone);
    }

    fn frame(ctx: &Context, keyboard: &mut KeyboardUi, events: Vec<Event>) -> Vec<Action> {
        let mut actions = Vec::new();
        let _ = ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(360.0, 240.0))),
                time: Some(ctx.cumulative_frame_nr() as f64 / 60.0),
                events,
                focused: true,
                ..Default::default()
            },
            |ctx| actions = keyboard.show(ctx, false, false),
        );
        actions
    }

    fn queued(ctx: &Context, keyboard: &mut KeyboardUi, events: &mut Vec<Event>) -> Vec<Action> {
        let mut actions = Vec::new();
        while !events.is_empty() {
            actions.extend(frame(ctx, keyboard, take_pointer_frame(events)));
        }
        actions
    }

    #[test]
    fn completed_native_taps_survive_a_stalled_frame() {
        let ctx = Context::default();
        let mut keyboard = KeyboardUi::default();
        frame(&ctx, &mut keyboard, vec![]);
        frame(&ctx, &mut keyboard, vec![]);
        let mut events = Vec::new();
        for (x, y) in [(143.07143, 165.0), (22.66667, 113.0), (140.66667, 113.0)] {
            tap(&mut events, x, y);
        }
        assert_eq!(
            queued(&ctx, &mut keyboard, &mut events),
            vec![
                Action::Text("c".into()),
                Action::Text("a".into()),
                Action::Text("f".into())
            ]
        );
        assert!(frame(&ctx, &mut keyboard, vec![]).is_empty());
    }

    #[test]
    fn queued_shift_and_symbol_pages_apply_before_later_letters() {
        let ctx = Context::default();
        let mut keyboard = KeyboardUi::default();
        frame(&ctx, &mut keyboard, vec![]);
        frame(&ctx, &mut keyboard, vec![]);
        let mut events = Vec::new();
        for (x, y) in [
            (23.0, 165.0),
            (22.66667, 113.0),
            (140.66667, 113.0),
            (30.0, 217.0),
            (69.21429, 165.0),
            (30.0, 217.0),
            (22.66667, 113.0),
        ] {
            tap(&mut events, x, y);
        }
        assert_eq!(
            queued(&ctx, &mut keyboard, &mut events),
            vec![
                Action::Text("A".into()),
                Action::Text("f".into()),
                Action::Text("é".into()),
                Action::Text("a".into())
            ]
        );
    }

    #[test]
    fn canceled_held_contact_cannot_activate_a_key_before_queued_new_taps() {
        let ctx = Context::default();
        let mut keyboard = KeyboardUi::default();
        frame(&ctx, &mut keyboard, vec![]);
        let mut held = Vec::new();
        tap(&mut held, 140.66667, 113.0);
        held.truncate(2);
        assert!(frame(&ctx, &mut keyboard, held).is_empty());
        let mut events = Vec::new();
        crate::cancel_pointer_input(&mut events);
        tap(&mut events, 143.07143, 165.0);
        tap(&mut events, 22.66667, 113.0);
        assert_eq!(
            queued(&ctx, &mut keyboard, &mut events),
            vec![Action::Text("c".into()), Action::Text("a".into())]
        );
    }

    #[test]
    fn queued_terminal_modifier_is_consumed_by_only_the_next_key() {
        let ctx = Context::default();
        let mut keyboard = KeyboardUi::default();
        let mut controls = terminal::Controls::default();
        let mut events = Vec::new();
        for (x, y) in [(28.0, 27.0), (21.0, 108.0), (21.0, 108.0)] {
            tap(&mut events, x, y);
        }
        let mut typed = Vec::new();
        // Warm up real widgets before processing the entire delayed input batch.
        for _ in 0..2 {
            let _ = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(360.0, 288.0))),
                    ..Default::default()
                },
                |ctx| {
                    keyboard.show_terminal(ctx, &mut controls);
                },
            );
        }
        while !events.is_empty() {
            let mut result = (Vec::new(), Vec::new());
            let _ = ctx.run(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(360.0, 288.0))),
                    events: take_pointer_frame(&mut events),
                    ..Default::default()
                },
                |ctx| {
                    result = keyboard.show_terminal(ctx, &mut controls);
                },
            );
            assert!(result.1.is_empty());
            for action in result.0 {
                typed.push((action, controls.take_modifiers()));
            }
        }
        assert_eq!(
            typed,
            vec![
                (
                    Action::Text("q".into()),
                    terminal::Modifiers {
                        control: true,
                        alt: false
                    }
                ),
                (Action::Text("q".into()), terminal::Modifiers::default())
            ]
        );
    }
}
