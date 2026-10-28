//! Terminal-only touch controls. Modifiers are local, one-shot selections.
use super::key_style;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Tab,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Paste,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Modifiers {
    pub control: bool,
    pub alt: bool,
}

impl Modifiers {
    pub fn any(self) -> bool {
        self.control || self.alt
    }
}

#[derive(Clone, Copy, Default)]
struct Selection {
    modifiers: Modifiers,
    more: bool,
    unsupported: bool,
}

#[derive(Clone, Default)]
pub struct Controls {
    selection: Selection,
    frame_start: Option<(u64, Selection)>,
}

impl Controls {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn take_modifiers(&mut self) -> Modifiers {
        self.selection.unsupported = false;
        std::mem::take(&mut self.selection.modifiers)
    }

    pub fn unsupported(&mut self) {
        self.selection.unsupported = true;
    }

    pub fn label(&self) -> &'static str {
        if self.selection.unsupported {
            "ASCII shortcuts only"
        } else {
            "English · terminal"
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Vec<Key> {
        let frame = ui.ctx().cumulative_frame_nr();
        if let Some((_, selection)) = self.frame_start.filter(|(previous, _)| *previous == frame) {
            self.selection = selection;
        } else {
            self.frame_start = Some((frame, self.selection));
        }
        key_style(ui);
        // Preserve 44-unit targets in portrait, and fit constrained outputs.
        let height = 44.0_f32.min(((ui.available_height() - 38.0) / 5.0).max(20.0));
        let mut keys = Vec::new();
        ui.horizontal(|ui| {
            if self.selection.more {
                let width = (ui.available_width() - 20.0) / 6.0;
                for (label, key) in [
                    ("Home", Key::Home),
                    ("End", Key::End),
                    ("←", Key::Left),
                    ("→", Key::Right),
                    ("Paste", Key::Paste),
                ] {
                    if button(ui, label, width, height, false) {
                        keys.push(key);
                    }
                }
                if button(ui, "More", width, height, true) {
                    self.selection.more = false;
                }
            } else {
                let width = (ui.available_width() - 24.0) / 7.0;
                if button(ui, "Ctrl", width, height, self.selection.modifiers.control) {
                    self.selection.modifiers.control = !self.selection.modifiers.control;
                    self.selection.unsupported = false;
                }
                if button(ui, "Alt", width, height, self.selection.modifiers.alt) {
                    self.selection.modifiers.alt = !self.selection.modifiers.alt;
                    self.selection.unsupported = false;
                }
                for (label, key) in [
                    ("Tab", Key::Tab),
                    ("Esc", Key::Escape),
                    ("↑", Key::Up),
                    ("↓", Key::Down),
                ] {
                    if button(ui, label, width, height, false) {
                        keys.push(key);
                    }
                }
                if button(ui, "More", width, height, false) {
                    self.selection.more = true;
                }
            }
        });
        keys
    }
}

fn button(ui: &mut egui::Ui, label: &str, width: f32, height: f32, selected: bool) -> bool {
    ui.add_sized(
        [width, height],
        egui::Button::new(egui::RichText::new(label).size(13.0))
            .corner_radius(5.0)
            .selected(selected),
    )
    .clicked()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard::{Action, KeyboardUi};

    fn frame(
        ctx: &egui::Context,
        keys: &mut KeyboardUi,
        controls: &mut Controls,
        height: f32,
        events: Vec<egui::Event>,
    ) -> (Vec<Action>, Vec<Key>) {
        let mut result = (Vec::new(), Vec::new());
        crate::theme::install(ctx);
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(360.0, height),
                )),
                events,
                ..Default::default()
            },
            |ctx| result = keys.show_terminal(ctx, controls),
        );
        result
    }
    fn tap(
        ctx: &egui::Context,
        keys: &mut KeyboardUi,
        controls: &mut Controls,
        height: f32,
        x: f32,
        y: f32,
    ) -> (Vec<Action>, Vec<Key>) {
        let pos = egui::pos2(x, y);
        frame(ctx, keys, controls, height, Vec::new());
        frame(
            ctx,
            keys,
            controls,
            height,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        frame(
            ctx,
            keys,
            controls,
            height,
            vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        )
    }

    #[test]
    fn terminal_touch_modifiers_pages_and_reset_preserve_text_keys() {
        let ctx = egui::Context::default();
        let mut keys = KeyboardUi::default();
        let mut controls = Controls::default();
        tap(&ctx, &mut keys, &mut controls, 288.0, 28.0, 27.0);
        tap(&ctx, &mut keys, &mut controls, 288.0, 79.0, 27.0);
        assert_eq!(
            controls.take_modifiers(),
            Modifiers {
                control: true,
                alt: true
            }
        );
        assert_eq!(controls.take_modifiers(), Modifiers::default());
        assert_eq!(
            tap(&ctx, &mut keys, &mut controls, 288.0, 21.0, 108.0).0,
            vec![Action::Text("q".into())]
        );
        tap(&ctx, &mut keys, &mut controls, 288.0, 332.0, 27.0);
        assert_eq!(
            tap(&ctx, &mut keys, &mut controls, 288.0, 269.0, 27.0).1,
            vec![Key::Paste]
        );
        controls.reset();
        assert_eq!(
            tap(&ctx, &mut keys, &mut controls, 288.0, 129.0, 27.0).1,
            vec![Key::Tab]
        );
        assert_eq!(
            tap(&ctx, &mut keys, &mut controls, 288.0, 329.0, 267.0).0,
            vec![Action::Hide]
        );
    }

    #[test]
    fn constrained_terminal_layout_keeps_hide_reachable() {
        let ctx = egui::Context::default();
        let mut keys = KeyboardUi::default();
        let mut controls = Controls::default();
        assert_eq!(
            tap(&ctx, &mut keys, &mut controls, 180.0, 329.0, 160.0).0,
            vec![Action::Hide]
        );
    }
}
