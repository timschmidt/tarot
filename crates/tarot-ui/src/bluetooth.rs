//! Bluetooth controls and transient pairing prompts. No pairing input is stored
//! in egui TextEdit state, undo history, accessibility values or diagnostics.
use egui::{Color32, Vec2};
use tarot_core::{
    Command, KeyboardAction, ShellModel,
    bluetooth::{Action, Answer, PromptKind, UiAction},
    network::Secret,
};
fn send(commands: &mut Vec<Command>, action: Action) {
    commands.push(Command::Bluetooth(action));
}
fn service(commands: &mut Vec<Command>, action: UiAction) {
    send(commands, Action::Service(action));
}
fn button(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(label).min_size(Vec2::new(ui.available_width(), 48.0)),
    )
    .clicked()
}
pub fn key(model: &ShellModel, action: KeyboardAction, commands: &mut Vec<Command>) {
    let Some(prompt) = &model.bluetooth.prompt else {
        return;
    };
    let id = prompt.id;
    send(
        commands,
        match action {
            KeyboardAction::Text(text) => Action::Text {
                request: id,
                value: Secret::new(text),
            },
            KeyboardAction::Backspace => Action::Backspace { request: id },
            KeyboardAction::Enter => Action::Submit { request: id },
            KeyboardAction::Hide => Action::Keyboard(false),
        },
    );
}
pub fn show(ui: &mut egui::Ui, model: &ShellModel, commands: &mut Vec<Command>) {
    egui::ScrollArea::vertical().id_salt("bluetooth_controls").max_height(ui.available_height().max(1.0)).show(ui,|ui|{
        let bt=&model.bluetooth;
        if let Some(p)=&bt.prompt {
            ui.push_id(("bluetooth_prompt",p.id),|ui|{
                ui.label(egui::RichText::new(&p.label).strong());
                ui.label(match p.kind {
                    PromptKind::Pin=>"Enter this device’s PIN.",
                    PromptKind::Passkey=>"Enter its passkey (up to six digits).",
                    PromptKind::Confirm=>"Does this code match the other device?",
                    PromptKind::DisplayPin=>"Type this PIN on the other device.",
                    PromptKind::DisplayPasskey=>"Type this passkey on the other device, then press Enter.",
                    PromptKind::AuthorizePairing=>"Allow this device to pair?",
                    PromptKind::AuthorizeService=>"Allow this device to use a Bluetooth service?",
                });
                if let Some(code)=&p.code {ui.label(egui::RichText::new(code.expose()).size(28.0).strong());}
                if p.kind==PromptKind::DisplayPasskey {ui.label(format!("{} of 6 digits entered",p.entered.min(6)));}
                if let Some(label)=&p.service {ui.label(label);}
                if p.kind.needs_code() {
                    let text=if model.bluetooth_ui.input.expose().is_empty(){"PIN or passkey".into()}else{"•".repeat(model.bluetooth_ui.input.expose().chars().count().min(16))};
                    if button(ui,&text,true) {send(commands,Action::Keyboard(true));}
                    if model.bluetooth_ui.keyboard {
                        for event in ui.input(|i|i.events.clone()) {
                            match event {
                                egui::Event::Text(text)=>key(model,KeyboardAction::Text(text),commands),
                                egui::Event::Key{key:egui::Key::Backspace,pressed:true,..}=>key(model,KeyboardAction::Backspace,commands),
                                egui::Event::Key{key:egui::Key::Enter,pressed:true,..}=>key(model,KeyboardAction::Enter,commands),
                                _=>{}
                            }
                        }
                    }
                    if button(ui,"Continue",p.kind.accepts(&model.bluetooth_ui.input)) {send(commands,Action::Submit{request:p.id});}
                } else if !p.kind.display_only() && button(ui,"Allow",true) {service(commands,UiAction::Reply{request:p.id,answer:Answer::Accept});}
                if button(ui,"Cancel",true) {service(commands,UiAction::Reply{request:p.id,answer:Answer::Reject});}
            });
            return;
        }
        if let Some(error)=&bt.error {ui.colored_label(Color32::LIGHT_RED,error);}
        if let Some(error)=&bt.action_error {ui.colored_label(Color32::LIGHT_RED,error);}
        if !bt.enabled {ui.label("System services are disabled.");}
        if let Some(pending)=bt.pending {
            ui.label(format!("{}…",match pending {
                tarot_core::bluetooth::Pending::Pair(_)=>"Pairing", tarot_core::bluetooth::Pending::Connect(_)=>"Connecting",
                tarot_core::bluetooth::Pending::Disconnect(_)=>"Disconnecting",tarot_core::bluetooth::Pending::Forget(_)=>"Removing pairing",
                _=>"Updating Bluetooth",
            }));
            if matches!(pending,tarot_core::bluetooth::Pending::Pair(_)) && button(ui,"Cancel pairing",true) {service(commands,UiAction::CancelPairing);}
        }
        let busy=bt.pending.is_some() || bt.busy;
        if let Some(id)=model.bluetooth_ui.selected {
            if let Some(device)=bt.devices.iter().find(|d|d.id==id) {
                ui.label(egui::RichText::new(&device.label).strong());
                if button(ui,"Back to devices",true) {send(commands,Action::Back);}
                if model.bluetooth_ui.forget==Some(id) {
                    ui.label("Forget this device? Its saved pairing information will be removed.");
                    if button(ui,"Forget device",!busy) {send(commands,Action::ConfirmForget);}
                    if button(ui,"Keep device",true) {send(commands,Action::Select(id));}
                    return;
                }
                ui.push_id((id,device.paired,device.connected,device.trusted,device.blocked),|ui|{
                    if device.blocked {ui.label("This device is blocked by the Bluetooth service.");}
                    let ready=bt.available && bt.adapter().is_some_and(|a|a.powered) && !busy && !device.blocked;
                    if !device.paired {
                        if button(ui,"Pair",ready && bt.agent_available) {service(commands,UiAction::Pair{device:id});}
                    } else {
                        if device.connected {
                            if button(ui,"Disconnect",ready) {service(commands,UiAction::Disconnect{device:id});}
                        } else if button(ui,"Connect",ready) {service(commands,UiAction::Connect{device:id});}
                        let mut trusted=device.trusted;
                        if ui.add_enabled(ready,egui::Checkbox::new(&mut trusted,"Trust this device")).changed() {service(commands,UiAction::Trust{device:id,trusted});}
                        ui.label("Trusted devices can use Bluetooth services without asking each time.");
                        if button(ui,"Forget device…",!busy) {send(commands,Action::AskForget(id));}
                    }
                });
            }
            return;
        }
        if bt.adapters.len()>1 {
            for adapter in &bt.adapters {
                if ui.add_enabled(!busy,egui::Button::new(&adapter.label).selected(Some(adapter.id)==bt.selected_adapter)).clicked() {service(commands,UiAction::Adapter{id:adapter.id});}
            }
        }
        if let Some(adapter)=bt.adapter() {
            ui.push_id((adapter.id,adapter.powered),|ui|{
                let mut powered=adapter.powered;
                if ui.add_enabled(bt.available && !busy,egui::Checkbox::new(&mut powered,"Bluetooth enabled")).changed() {service(commands,UiAction::Radio{adapter:adapter.id,enabled:powered});}
            });
            let ready=bt.available && adapter.powered && !busy;
            if button(ui,if bt.scanning {"Stop searching"}else{"Find devices"},ready) {service(commands,UiAction::Scan{adapter:adapter.id,enabled:!bt.scanning});}
            let mut visible=bt.visible;
            if ui.add_enabled(ready && bt.default_agent,egui::Checkbox::new(&mut visible,"Allow new pairings for 2 minutes")).changed() {service(commands,UiAction::Visible{adapter:adapter.id,enabled:visible});}
            if !bt.agent_available {ui.label("Pairing agent unavailable.");}
            else if !bt.default_agent {ui.label("Incoming pairing needs system authorization. You can still pair with a device below.");}
            ui.label("Keep this page open while pairing.");
        } else {ui.label("No Bluetooth adapter available.");}
        ui.separator();
        for device in &bt.devices {
            ui.push_id(("bluetooth_device",device.id),|ui|{
                let detail=if device.connected {"Connected"}else if device.paired {"Paired"}else{"Available"};
                if button(ui,&format!("{} · {}",device.label,detail),true) {send(commands,Action::Select(device.id));}
            });
        }
        if bt.devices.is_empty() {ui.label("No devices found. Put the other device in pairing mode, then choose Find devices.");}
    });
}
