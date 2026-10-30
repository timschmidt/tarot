//! The password field renders only bullets. It has no TextEdit undo/history
//! state, and never copies its value into egui memory or accessibility text.
use egui::{Color32, Pos2, Rect, Vec2};
use tarot_core::{
    Command, KeyboardAction, Layout, ShellModel,
    network::{InputTarget, PasswordKind, Secret},
};
fn key(action: KeyboardAction, commands: &mut Vec<Command>) {
    commands.push(match action {
        KeyboardAction::Text(text) => Command::CredentialText(Secret::new(text)),
        KeyboardAction::Backspace => Command::CredentialBackspace,
        KeyboardAction::Enter => Command::CredentialSubmit,
        KeyboardAction::Hide => Command::CredentialKeyboard(false),
    });
}
pub fn show(ui: &mut egui::Ui, model: &ShellModel, commands: &mut Vec<Command>) {
    let Some(input) = &model.network_input else {
        return;
    };
    ui.label(egui::RichText::new(&input.label).strong());
    if matches!(input.target, InputTarget::Agent(_)) {
        ui.label("Enter the password to continue connecting.");
    }
    ui.label(input.kind.guidance());
    if input.kind != PasswordKind::Open {
        let bullets = "•".repeat(input.password.expose().chars().count().min(24));
        let label = if bullets.is_empty() {
            "Password".to_owned()
        } else {
            bullets
        };
        if ui
            .add_sized([ui.available_width(), 48.0], egui::Button::new(label))
            .clicked()
        {
            commands.push(Command::CredentialKeyboard(true));
        }
        if !input.keyboard {
            ui.label("Tap the password field to open the keyboard.");
        }
        // The owned buffer stays in the model. Raw input is consumed here and
        // emitted as redacted commands; native app focus remains suspended.
        let events = ui.input(|i| i.events.clone());
        for event in events {
            match event {
                egui::Event::Text(text) if input.keyboard => {
                    key(KeyboardAction::Text(text), commands)
                }
                egui::Event::Key {
                    key: egui::Key::Backspace,
                    pressed: true,
                    ..
                } => key(KeyboardAction::Backspace, commands),
                egui::Event::Key {
                    key: egui::Key::Enter,
                    pressed: true,
                    ..
                } => key(KeyboardAction::Enter, commands),
                _ => {}
            }
        }
    }
    if matches!(input.target, InputTarget::New(_)) {
        let mut remember = input.remember;
        if ui
            .checkbox(&mut remember, "Remember this network")
            .changed()
        {
            commands.push(Command::CredentialRemember(remember));
        }
        ui.label(if remember {
            "Save this network and password for future connections."
        } else {
            "Use a temporary profile; remove it after disconnection."
        });
    }
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                input.kind.accepts(&input.password)
                    && (matches!(input.target, InputTarget::Agent(_))
                        || model.can_request_new_network()),
                egui::Button::new("Connect").min_size(Vec2::new(120.0, 48.0)),
            )
            .clicked()
        {
            commands.push(Command::CredentialSubmit);
        }
        if ui
            .add_sized([120.0, 48.0], egui::Button::new("Cancel"))
            .clicked()
        {
            commands.push(Command::CredentialCancel);
        }
    });
}
pub fn keyboard(
    ctx: &egui::Context,
    model: &ShellModel,
    layout: Layout,
    commands: &mut Vec<Command>,
) {
    let id = egui::Id::new("credential_keyboard_state");
    if !model.credential_keyboard_visible() {
        ctx.data_mut(|d| d.remove::<super::keyboard::KeyboardUi>(id));
        return;
    }
    let identity = if model.system_page == tarot_core::SystemPage::Authorization {
        let request = model
            .authorization
            .snapshot
            .request
            .as_ref()
            .map_or(0, |r| r.id);
        let turn = model.authorization.question().map_or(0, |(_, q)| q.id);
        (3u8, request, turn)
    } else if model.system_page == tarot_core::SystemPage::Bluetooth {
        (2u8, model.bluetooth.prompt.as_ref().map_or(0, |p| p.id), 0)
    } else {
        match model.network_input.as_ref().map(|i| i.target) {
            Some(InputTarget::New(id)) => (0, id, 0),
            Some(InputTarget::Agent(id)) => (1, id, 0),
            None => (4, 0, 0),
        }
    };
    let context_id = egui::Id::new("credential_keyboard_context");
    if ctx.data_mut(|d| d.get_temp::<(u8, u64, u64)>(context_id)) != Some(identity) {
        ctx.data_mut(|d| {
            d.remove::<super::keyboard::KeyboardUi>(id);
            d.insert_temp(context_id, identity);
        });
    }
    let rect = layout.keyboard();
    let mut keyboard = ctx
        .data_mut(|d| d.get_temp::<super::keyboard::KeyboardUi>(id))
        .unwrap_or_default();
    egui::Area::new("credential_keyboard".into())
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::new(rect.x, rect.y))
        .movable(false)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.set_clip_rect(Rect::from_min_size(
                Pos2::new(rect.x, rect.y),
                Vec2::new(rect.width, rect.height),
            ));
            ui.set_min_size(Vec2::new(rect.width, rect.height));
            ui.set_max_size(Vec2::new(rect.width, rect.height));
            egui::Frame::new()
                .fill(Color32::from_rgb(38, 40, 44))
                .inner_margin(5.0)
                .show(ui, |ui| {
                    for action in
                        keyboard.show_in_ui(
                            ui,
                            model.bluetooth.prompt.as_ref().is_some_and(|p| {
                                p.kind == tarot_core::bluetooth::PromptKind::Passkey
                            }) && model.system_page == tarot_core::SystemPage::Bluetooth,
                            true,
                        )
                    {
                        if model.system_page == tarot_core::SystemPage::Authorization {
                            super::authorization::key(model, action, commands);
                        } else if model.system_page == tarot_core::SystemPage::Bluetooth {
                            super::bluetooth::key(model, action, commands);
                        } else {
                            key(action, commands);
                        }
                    }
                });
        });
    ctx.data_mut(|d| d.insert_temp(id, keyboard));
}
