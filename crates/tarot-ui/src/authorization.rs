//! Compositor-owned authentication view; ordinary clients never receive entry.
use egui::Vec2;
use tarot_core::{
    Command, KeyboardAction, ShellModel,
    authorization::{Action, Phase, Response, UiAction},
};
fn send(commands: &mut Vec<Command>, action: Action) {
    commands.push(Command::Authorization(action));
}
pub fn key(model: &ShellModel, action: KeyboardAction, commands: &mut Vec<Command>) {
    let Some((request, q)) = model.authorization.question() else {
        return;
    };
    let turn = q.id;
    let action = match action {
        KeyboardAction::Text(text) => {
            let Some(text) = Response::new(text) else {
                return;
            };
            Action::Text {
                request,
                turn,
                text,
            }
        }
        KeyboardAction::Backspace => Action::Backspace { request, turn },
        KeyboardAction::Enter => Action::Submit { request, turn },
        KeyboardAction::Hide => Action::Keyboard(false),
    };
    send(commands, action);
}
fn button(ui: &mut egui::Ui, text: &str) -> bool {
    ui.add_sized(
        Vec2::new(ui.available_width(), 48.0),
        egui::Button::new(text),
    )
    .clicked()
}
pub fn show(ui: &mut egui::Ui, model: &ShellModel, commands: &mut Vec<Command>) {
    let state = &model.authorization;
    egui::ScrollArea::vertical()
        .id_salt("authorization_controls")
        .max_height(ui.available_height().max(1.0))
        .show(ui, |ui| {
            let Some(r) = &state.snapshot.request else {
                ui.label(
                    state
                        .snapshot
                        .error
                        .as_deref()
                        .unwrap_or("Authentication request ended."),
                );
                if button(ui, "Close") {
                    commands.push(Command::SystemMenuOpen(false));
                }
                return;
            };
            ui.push_id(("authorization_request", r.id), |ui| {
                ui.label(egui::RichText::new(&r.message).strong());
                ui.label(egui::RichText::new(&r.action).small());
                if let Some(uid) = r.selected {
                    let name = r
                        .identities
                        .iter()
                        .find(|i| i.uid == uid)
                        .map_or_else(|| format!("User {uid}"), |i| i.name.clone());
                    ui.label(format!("Authenticate as {name}"));
                }
                if let Some(info) = &r.info {
                    ui.label(info);
                }
                match &r.phase {
                    Phase::Choose => {
                        ui.label("Choose an account:");
                        for identity in &r.identities {
                            if button(ui, &identity.name) {
                                send(
                                    commands,
                                    Action::Service(UiAction::Select {
                                        request: r.id,
                                        uid: identity.uid,
                                    }),
                                );
                            }
                        }
                    }
                    Phase::Starting => {
                        ui.label("Preparing authentication…");
                    }
                    Phase::Checking => {
                        ui.label("Checking…");
                    }
                    Phase::Question(question) => {
                        ui.push_id(("authentication_question", question.id), |ui| {
                            ui.label(&question.prompt);
                            let text = if state.input.expose().is_empty() {
                                "Enter response".into()
                            } else if question.echo {
                                state.input.expose().to_owned()
                            } else {
                                "•".repeat(state.input.expose().chars().count().min(24))
                            };
                            if button(ui, &text) {
                                send(commands, Action::Keyboard(true));
                            }
                            if state.input_error {
                                ui.label("The response is too long.");
                            }
                            if state.keyboard {
                                for event in ui.input(|i| i.events.clone()) {
                                    match event {
                                        egui::Event::Text(text) => {
                                            key(model, KeyboardAction::Text(text), commands)
                                        }
                                        egui::Event::Key {
                                            key: egui::Key::Backspace,
                                            pressed: true,
                                            ..
                                        } => key(model, KeyboardAction::Backspace, commands),
                                        egui::Event::Key {
                                            key: egui::Key::Enter,
                                            pressed: true,
                                            ..
                                        } => key(model, KeyboardAction::Enter, commands),
                                        _ => {}
                                    }
                                }
                            }
                            if button(ui, "Authenticate") {
                                send(
                                    commands,
                                    Action::Submit {
                                        request: r.id,
                                        turn: question.id,
                                    },
                                );
                            }
                        });
                    }
                }
                if button(ui, "Cancel") {
                    commands.push(Command::SystemMenuOpen(false));
                }
            });
        });
}
