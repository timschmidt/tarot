use egui::{Color32, Vec2};
use tarot_core::{
    Command, Layout, ShellModel, SystemPage,
    network::{Pending, Permission, State, UiAction},
};
fn action(commands: &mut Vec<Command>, action: UiAction) {
    commands.push(Command::Network(action));
}
pub fn show(ui: &mut egui::Ui, model: &ShellModel, _layout: Layout, commands: &mut Vec<Command>) {
    let network = &model.network;
    if ui
        .add_sized(
            [ui.available_width(), 48.0],
            egui::Button::new("Back to system controls"),
        )
        .clicked()
    {
        commands.push(Command::SystemPage(SystemPage::Power));
    }
    if !network.enabled {
        ui.label("System services are disabled.");
    }
    if let Some(error) = &network.error {
        ui.colored_label(Color32::LIGHT_RED, error);
    }
    let mut radio = network.radio;
    if ui
        .add_enabled(
            network.available
                && !network.busy
                && network.radio_permission.allowed()
                && (radio || network.hardware),
            egui::Checkbox::new(&mut radio, "Wi-Fi enabled"),
        )
        .changed()
    {
        action(commands, UiAction::Radio { enabled: radio });
    }
    if !network.hardware && network.available {
        ui.label("Wi-Fi hardware is blocked or unavailable.");
    }
    ui.label(network.state.label());
    ui.label(match network.connectivity {
        4 => "System internet available",
        3 => "System connectivity limited",
        2 => "System captive portal",
        1 => "No system internet",
        _ => "System connectivity unknown",
    });
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                network.available
                    && network.radio
                    && network.hardware
                    && !network.busy
                    && network.scan_permission.allowed(),
                egui::Button::new("Scan").min_size(Vec2::new(96.0, 48.0)),
            )
            .clicked()
        {
            action(commands, UiAction::Scan);
        }
        if ui
            .add_enabled(
                network.state == State::Connected
                    && !network.busy
                    && network.control_permission.allowed(),
                egui::Button::new("Disconnect").min_size(Vec2::new(144.0, 48.0)),
            )
            .clicked()
        {
            action(commands, UiAction::Disconnect);
        }
    });
    if [
        network.scan_permission,
        network.control_permission,
        network.radio_permission,
    ]
    .contains(&Permission::Authentication)
    {
        ui.label("Some actions need system authorization.");
    }
    if network.busy {
        ui.label(match network.pending {
            Some(Pending::Scan) => "Scanning…",
            Some(Pending::Connect(_)) => "Waiting for connection…",
            Some(Pending::Radio(_)) => "Changing Wi-Fi state…",
            Some(Pending::Disconnect) => "Disconnecting…",
            None => "Working…",
        });
    }
    if let Some(error) = &network.action_error {
        ui.colored_label(Color32::LIGHT_RED, error);
    }
    egui::ScrollArea::vertical()
        .id_salt("wifi_networks")
        .max_height(ui.available_height().max(48.0))
        .show(ui, |ui| {
            if network.networks.is_empty() {
                ui.label("No visible networks");
            }
            for row in &network.networks {
                ui.push_id(row.id, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(35, 40, 44))
                        .inner_margin(8.0)
                        .corner_radius(8.0)
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label(egui::RichText::new(&row.label).strong());
                            ui.label(format!(
                                "{} · {}%{}",
                                row.security.label(),
                                row.strength,
                                if row.connected { " · Connected" } else { "" }
                            ));
                            if row.profiles.is_empty() {
                                if row.password_kind.is_some() {
                                    if ui
                                        .add_enabled(
                                            network.available
                                                && network.radio
                                                && network.hardware
                                                && !network.busy
                                                && network.create_permission.allowed()
                                                && network.control_permission.allowed(),
                                            egui::Button::new("Connect to network…")
                                                .min_size(Vec2::new(ui.available_width(), 48.0)),
                                        )
                                        .clicked()
                                    {
                                        commands.push(Command::NetworkSetup(row.id));
                                    }
                                } else {
                                    ui.label("This network needs unsupported security settings.");
                                }
                            }
                            for profile in &row.profiles {
                                let text = if row.profiles.len() == 1 {
                                    "Connect saved network".into()
                                } else {
                                    format!("Connect: {}", profile.label)
                                };
                                if ui
                                    .add_enabled(
                                        network.available
                                            && network.radio
                                            && network.hardware
                                            && !network.busy
                                            && network.control_permission.allowed(),
                                        egui::Button::new(text)
                                            .min_size(Vec2::new(ui.available_width(), 48.0)),
                                    )
                                    .clicked()
                                {
                                    action(
                                        commands,
                                        UiAction::Connect {
                                            network: row.id,
                                            profile: profile.id,
                                        },
                                    );
                                }
                            }
                        });
                    ui.add_space(6.0);
                });
            }
        });
}
