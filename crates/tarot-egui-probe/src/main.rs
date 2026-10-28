//! Ordinary eframe/xdg-shell compatibility fixture. Use synthetic text only.
use anyhow::{Context, Result};
use clap::Parser;
use eframe::egui;
use eframe::glow::HasContext;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::PathBuf,
    sync::mpsc::{self, SyncSender},
    thread::JoinHandle,
    time::Duration,
};

#[derive(Parser)]
#[command(about = "Ordinary native egui compatibility fixture; use synthetic text only")]
struct Options {
    /// Optional new JSON-lines file containing synthetic text and widget geometry.
    #[arg(long)]
    report: Option<PathBuf>,
    /// Open both native windows immediately for a noninteractive lifecycle check.
    #[arg(long)]
    second_window: bool,
}

#[derive(Clone, Default, PartialEq, Serialize)]
struct WindowState {
    size: [f32; 2],
    focused: bool,
    focused_widget: Option<String>,
    widgets: BTreeMap<String, [f32; 4]>,
}

#[derive(Clone, Default, PartialEq, Serialize)]
struct Snapshot {
    primary_text: String,
    secondary_text: String,
    password_characters: usize,
    second_open: bool,
    primary: WindowState,
    secondary: WindowState,
}

struct Reporter {
    sender: Option<SyncSender<Snapshot>>,
    worker: Option<JoinHandle<Result<()>>>,
    last: Option<Snapshot>,
}

impl Reporter {
    fn new(path: Option<PathBuf>) -> Result<Self> {
        let mut reporter = Self {
            sender: None,
            worker: None,
            last: None,
        };
        if let Some(path) = path {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)
                .with_context(|| format!("Creating new synthetic report {}", path.display()))?;
            let (sender, receiver) = mpsc::sync_channel::<Snapshot>(8);
            reporter.worker = Some(
                std::thread::Builder::new()
                    .name("egui-probe-report".into())
                    .spawn(move || {
                        for snapshot in receiver {
                            serde_json::to_writer(&mut file, &snapshot)?;
                            file.write_all(b"\n")?;
                            file.flush()?;
                        }
                        Ok(())
                    })?,
            );
            reporter.sender = Some(sender);
        }
        Ok(reporter)
    }

    fn record(&mut self, snapshot: &Snapshot, ctx: &egui::Context) {
        let Some(sender) = &self.sender else { return };
        if self.last.as_ref() == Some(snapshot) {
            return;
        }
        match sender.try_send(snapshot.clone()) {
            Ok(()) => self.last = Some(snapshot.clone()),
            Err(mpsc::TrySendError::Full(_)) => {
                ctx.request_repaint_after(Duration::from_millis(16))
            }
            Err(mpsc::TrySendError::Disconnected(_)) => self.sender = None,
        }
    }
}

impl Drop for Reporter {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            match worker.join() {
                Ok(Ok(())) => {}
                result => eprintln!("Synthetic report worker failed: {result:?}"),
            }
        }
    }
}

struct Probe {
    state: Snapshot,
    password: String,
    reporter: Reporter,
    private_purpose: Option<bool>,
}

fn widget(window: &mut WindowState, name: &str, response: &egui::Response) {
    if response.has_focus() {
        window.focused_widget = Some(name.into());
    }
    let r = response.rect;
    window
        .widgets
        .insert(name.into(), [r.min.x, r.min.y, r.width(), r.height()]);
}

fn window_state(ctx: &egui::Context) -> WindowState {
    let rect = ctx.viewport_rect();
    WindowState {
        size: [rect.width(), rect.height()],
        focused: ctx.input(|i| i.focused),
        ..Default::default()
    }
}

fn entry(ui: &mut egui::Ui, text: &mut String, id: &str, password: bool) -> egui::Response {
    ui.add_sized(
        [ui.available_width(), 48.0],
        egui::TextEdit::singleline(text)
            .id_salt(id)
            .password(password)
            .font(egui::TextStyle::Body),
    )
}

impl eframe::App for Probe {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let secondary = egui::ViewportId::from_hash_of("egui-probe-secondary");
        if ctx.input(|input| {
            input
                .raw
                .viewports
                .get(&secondary)
                .is_some_and(|v| v.close_requested())
        }) {
            self.state.second_open = false;
        }
        self.state.primary = window_state(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Native egui application");
            ui.label("Synthetic compatibility fixture");
            ui.add_space(12.0);
            ui.label("Text entry");
            let response = entry(ui, &mut self.state.primary_text, "primary-text", false);
            widget(&mut self.state.primary, "text", &response);
            ui.add_space(8.0);
            ui.label("Password entry (synthetic only)");
            let response = entry(ui, &mut self.password, "private-text", true);
            widget(&mut self.state.primary, "password", &response);
            let private = response.has_focus();
            if self.private_purpose != Some(private) {
                ctx.send_viewport_cmd(egui::ViewportCommand::IMEPurpose(if private {
                    egui::viewport::IMEPurpose::Password
                } else {
                    egui::viewport::IMEPurpose::Normal
                }));
                self.private_purpose = Some(private);
            }
            self.state.password_characters = self.password.chars().count();
            ui.add_space(12.0);
            let response = ui.add_sized(
                [ui.available_width(), 48.0],
                egui::Button::new("Open second window"),
            );
            widget(&mut self.state.primary, "open-second", &response);
            if response.clicked() {
                self.state.second_open = true;
            }
            ui.label("Each native window should be an independent card.");
        });
        if self.state.second_open {
            ctx.show_viewport_immediate(
                secondary,
                egui::ViewportBuilder::default()
                    .with_app_id("org.tarot.EguiProbe")
                    .with_title("Tarot egui — second window")
                    .with_decorations(false)
                    .with_inner_size([360.0, 480.0]),
                |ctx, _class| {
                    self.state.secondary = window_state(ctx);
                    if ctx.input(|i| i.viewport().close_requested()) {
                        self.state.second_open = false;
                        ctx.request_repaint_of(egui::ViewportId::ROOT);
                    }
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.heading("Second native window");
                        ui.label("Independent text; shared application process.");
                        ui.add_space(12.0);
                        let response =
                            entry(ui, &mut self.state.secondary_text, "secondary-text", false);
                        widget(&mut self.state.secondary, "text", &response);
                        ui.add_space(12.0);
                        ui.label(format!("First window text: {}", self.state.primary_text));
                    });
                },
            );
        } else {
            self.state.secondary = WindowState::default();
        }
        self.reporter.record(&self.state, ctx);
    }
}

fn main() -> Result<()> {
    let options = Options::parse();
    let reporter = Reporter::new(options.report)?;
    let native = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_app_id("org.tarot.EguiProbe")
            .with_title("Tarot egui — primary window")
            .with_decorations(false)
            .with_inner_size([360.0, 664.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tarot egui compatibility",
        native,
        Box::new(move |cc| {
            tarot_ui::native_text::install(&cc.egui_ctx);
            if let Some(gl) = &cc.gl {
                // SAFETY: eframe calls its app creator with this GL context current.
                let renderer = unsafe { gl.get_parameter_string(eframe::glow::RENDERER) };
                eprintln!("egui fixture renderer: {renderer}");
            }
            cc.egui_ctx.style_mut(|style| {
                style.spacing.item_spacing = egui::vec2(8.0, 8.0);
                style
                    .text_styles
                    .insert(egui::TextStyle::Body, egui::FontId::proportional(18.0));
            });
            Ok(Box::new(Probe {
                state: Snapshot {
                    second_open: options.second_window,
                    ..Default::default()
                },
                password: String::new(),
                reporter,
                private_purpose: None,
            }))
        }),
    )
    .map_err(|error| anyhow::anyhow!(error.to_string()))
}
