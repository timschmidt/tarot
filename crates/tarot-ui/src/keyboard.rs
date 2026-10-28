pub use tarot_core::KeyboardAction as Action;

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
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
        ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::from_rgb(220, 221, 224);
        ui.visuals_mut().widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(220, 221, 224);
        ui.visuals_mut().widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(20, 23, 26);
        ui.visuals_mut().widgets.hovered.fg_stroke.color = egui::Color32::BLACK;
        ui.visuals_mut().widgets.hovered.bg_fill = egui::Color32::from_rgb(245, 246, 248);
        ui.visuals_mut().widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(245, 246, 248);
        ui.visuals_mut().widgets.active.bg_fill = egui::Color32::from_rgb(150, 185, 220);
        ui.label(
            egui::RichText::new(if secure { "Private entry" } else { "English" })
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
