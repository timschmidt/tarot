//! Ordinary Rust/egui Wayland application with cooperating GMenu exports.
//! Run in a desktop session; --report/--qa-stdin are synthetic QA only.
#[path = "native_menu_egui/export.rs"]
mod export;
#[path = "native_menu_egui/gtk.rs"]
mod gtk;
use anyhow::{Context, Result, ensure};
use clap::Parser;
use std::{
    collections::HashSet,
    io::BufRead,
    os::fd::AsRawFd,
    path::PathBuf,
    sync::{Arc, mpsc},
    time::Instant,
};
use tarot_wayland_egui::Graphics;
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop,
    protocol::{
        wl_callback, wl_compositor, wl_pointer, wl_registry, wl_seat, wl_surface, wl_touch,
    },
};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

#[derive(Parser)]
struct Options {
    #[arg(long)]
    report: Option<PathBuf>,
    /// Explicit fixture-only stdin mutations, never action injection.
    #[arg(long)]
    qa_stdin: bool,
    /// Keep this app's controls available without claiming shell integration.
    #[arg(long)]
    no_claim: bool,
    /// Fixture-only exporter in a separate process, without a Wayland surface.
    #[arg(long, requires = "qa_stdin")]
    export_only: bool,
}
struct App {
    graphics: Option<Graphics>,
    compositor: Option<wl_compositor::WlCompositor>,
    shell: Option<xdg_wm_base::XdgWmBase>,
    gtk: Option<gtk::gtk_shell1::GtkShell1>,
    seat: Option<wl_seat::WlSeat>,
    pointer: Option<wl_pointer::WlPointer>,
    touch: Option<wl_touch::WlTouch>,
    context: egui::Context,
    events: Vec<egui::Event>,
    position: egui::Pos2,
    contacts: HashSet<i32>,
    touch_owner: Option<i32>,
    width: u32,
    height: u32,
    configured: bool,
    dirty: bool,
    closed: bool,
    frame_sequence: u64,
    pending_frame: Option<u64>,
    observed_revision: u64,
    start: Instant,
}
impl App {
    fn button(&mut self, pressed: bool) {
        self.events.push(egui::Event::PointerButton {
            pos: self.position,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        self.dirty = true;
    }
    fn cancel(&mut self) {
        tarot_ui::cancel_pointer_input(&mut self.events);
        self.touch_owner = None;
        self.dirty = true;
    }
    fn paint(
        &mut self,
        connection: &Connection,
        surface: &wl_surface::WlSurface,
        qh: &QueueHandle<Self>,
        export: &export::Export,
    ) -> Result<()> {
        ensure!(
            self.width <= 4096 && self.height <= 4096,
            "sample window size bound"
        );
        let data = export.shared.0.lock().unwrap();
        let revision = data.revision;
        let chosen = data.chosen.clone();
        let flag = data.flag;
        let value = data.window_value;
        let enabled = data.choose_enabled;
        drop(data);
        self.dirty |= revision != self.observed_revision;
        if !self.configured
            || self.pending_frame.is_some()
            || !(self.dirty || self.context.has_requested_repaint())
        {
            return Ok(());
        }
        if self.graphics.is_none() {
            self.graphics = Some(Graphics::new(connection, surface, self.width, self.height)?);
        }
        let graphics = self.graphics.as_mut().unwrap();
        graphics.resize(self.width, self.height);
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.width as f32, self.height as f32),
            )),
            time: Some(self.start.elapsed().as_secs_f64()),
            events: std::mem::take(&mut self.events),
            ..Default::default()
        };
        let mut action = None;
        let output = self.context.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.heading("Rust egui menu application");
                ui.label("App-owned actions in Tarot's activity menu");
                ui.add_space(12.0);
                ui.label(format!("Chosen theme: {chosen}"));
                ui.label(format!("Flag: {}", if flag { "on" } else { "off" }));
                ui.label(format!("Window value: {value}"));
                ui.add_space(12.0);
                if ui
                    .add_enabled(
                        enabled,
                        egui::Button::new("Choose night").min_size(egui::vec2(180.0, 48.0)),
                    )
                    .clicked()
                {
                    action = Some("choose");
                }
                if ui
                    .add(egui::Button::new("Toggle flag").min_size(egui::vec2(180.0, 48.0)))
                    .clicked()
                {
                    action = Some("toggle");
                }
                ui.label("These controls remain usable without shell exports.");
            });
        });
        self.frame_sequence += 1;
        let sequence = self.frame_sequence;
        surface.frame(qh, sequence);
        self.pending_frame = Some(sequence);
        graphics.paint(&self.context, output)?;
        self.dirty = false;
        self.observed_revision = revision;
        if let Some(action) = action {
            export.local(action)?;
            self.dirty = true;
        }
        Ok(())
    }
}
impl Dispatch<wl_registry::WlRegistry, ()> for App {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(4), qh, ()))
                }
                "xdg_wm_base" => state.shell = Some(registry.bind(name, version.min(6), qh, ())),
                "gtk_shell1" if version >= 4 => state.gtk = Some(registry.bind(name, 4, qh, ())),
                "wl_seat" => state.seat = Some(registry.bind(name, version.min(7), qh, ())),
                _ => {}
            }
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase, ()> for App {
    fn event(
        _: &mut Self,
        shell: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            shell.pong(serial);
        }
    }
}
impl Dispatch<xdg_surface::XdgSurface, ()> for App {
    fn event(
        state: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            state.configured = true;
            state.pending_frame = None;
            state.dirty = true;
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel, ()> for App {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if width > 0 {
                    state.width = width as u32;
                }
                if height > 0 {
                    state.height = height as u32;
                }
                state.dirty = true;
            }
            xdg_toplevel::Event::Close => state.closed = true,
            _ => {}
        }
    }
}
impl Dispatch<wl_callback::WlCallback, u64> for App {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        ticket: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if state.pending_frame == Some(*ticket) {
            state.pending_frame = None;
        }
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for App {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            if capabilities.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qh, ()));
            }
            if capabilities.contains(wl_seat::Capability::Touch) && state.touch.is_none() {
                state.touch = Some(seat.get_touch(qh, ()));
            }
            if !capabilities.contains(wl_seat::Capability::Pointer)
                && let Some(pointer) = state.pointer.take()
            {
                pointer.release();
                state.cancel();
            }
            if !capabilities.contains(wl_seat::Capability::Touch)
                && let Some(touch) = state.touch.take()
            {
                touch.release();
                state.contacts.clear();
                state.cancel();
            }
        }
    }
}
impl Dispatch<wl_pointer::WlPointer, ()> for App {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface_x,
                surface_y,
                ..
            }
            | wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                state.position = egui::pos2(surface_x as f32, surface_y as f32);
                state.events.push(egui::Event::PointerMoved(state.position));
                state.dirty = true;
            }
            wl_pointer::Event::Button {
                button: 0x110,
                state: WEnum::Value(button),
                ..
            } => state.button(button == wl_pointer::ButtonState::Pressed),
            wl_pointer::Event::Leave { .. } => state.cancel(),
            _ => {}
        }
    }
}
impl Dispatch<wl_touch::WlTouch, ()> for App {
    fn event(
        state: &mut Self,
        _: &wl_touch::WlTouch,
        event: wl_touch::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_touch::Event::Down { id, x, y, .. } => {
                state.contacts.insert(id);
                if state.contacts.len() == 1 {
                    state.touch_owner = Some(id);
                    state.position = egui::pos2(x as f32, y as f32);
                    state.events.push(egui::Event::PointerMoved(state.position));
                    state.button(true);
                } else {
                    state.cancel();
                }
            }
            wl_touch::Event::Motion { id, x, y, .. } if state.touch_owner == Some(id) => {
                state.position = egui::pos2(x as f32, y as f32);
                state.events.push(egui::Event::PointerMoved(state.position));
                state.dirty = true;
            }
            wl_touch::Event::Up { id, .. } => {
                state.contacts.remove(&id);
                if state.touch_owner == Some(id) {
                    state.button(false);
                    state.touch_owner = None;
                }
            }
            wl_touch::Event::Cancel => {
                state.contacts.clear();
                state.cancel();
            }
            _ => {}
        }
    }
}
delegate_noop!(App:ignore wl_compositor::WlCompositor);
delegate_noop!(App:ignore wl_surface::WlSurface);
delegate_noop!(App:ignore gtk::gtk_shell1::GtkShell1);
delegate_noop!(App:ignore gtk::gtk_surface1::GtkSurface1);

fn main() -> Result<()> {
    let options = Options::parse();
    let mut export = Arc::new(export::Export::start(options.report.as_deref())?);
    let (commands, pending) = mpsc::sync_channel(16);
    if options.qa_stdin {
        std::thread::Builder::new()
            .name("menu-egui-qa".into())
            .spawn(move || {
                for line in std::io::stdin().lock().lines() {
                    let Ok(line) = line else { break };
                    if line.len() > 1024 || commands.send(line).is_err() {
                        break;
                    }
                }
            })?;
    }
    if options.export_only {
        while let Ok(command) = pending.recv() {
            if command == "exit" {
                break;
            }
            export.mutate(&command)?;
        }
        return Ok(());
    }
    let connection = Connection::connect_to_env()?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut app = App {
        graphics: None,
        compositor: None,
        shell: None,
        gtk: None,
        seat: None,
        pointer: None,
        touch: None,
        context: egui::Context::default(),
        events: vec![],
        position: egui::Pos2::ZERO,
        contacts: HashSet::new(),
        touch_owner: None,
        width: 360,
        height: 652,
        configured: false,
        dirty: true,
        closed: false,
        frame_sequence: 0,
        pending_frame: None,
        observed_revision: 0,
        start: Instant::now(),
    };
    tarot_ui::theme::install(&app.context);
    connection.display().get_registry(&qh, ());
    queue.roundtrip(&mut app)?;
    let surface = app
        .compositor
        .as_ref()
        .context("wl_compositor missing")?
        .create_surface(&qh, ());
    let xdg = app
        .shell
        .as_ref()
        .context("xdg_wm_base missing")?
        .get_xdg_surface(&surface, &qh, ());
    let toplevel = xdg.get_toplevel(&qh, ());
    toplevel.set_title("Rust egui menus".into());
    toplevel.set_app_id("org.tarot.MenuEgui".into());
    let mut extension = if options.no_claim {
        None
    } else if let Some(gtk) = &app.gtk {
        let extension = gtk.get_gtk_surface(&surface, &qh, ());
        extension.set_dbus_properties(
            Some("org.tarot.MenuEgui".into()),
            Some(export::MENU.into()),
            None,
            Some(export::WINDOW.into()),
            Some(export::APP.into()),
            Some(export.owner().into()),
        );
        Some(extension)
    } else {
        None
    };
    surface.commit();
    connection.flush()?;
    while !app.configured {
        queue.blocking_dispatch(&mut app)?;
    }
    app.paint(&connection, &surface, &qh, &export)?;
    export.shared.mapped();
    while !app.closed {
        queue.dispatch_pending(&mut app)?;
        for command in pending.try_iter().take(16) {
            match command.as_str() {
                "flip" | "disable" => export.mutate(&command)?,
                _ if command.starts_with("noise ") => export.mutate(&command)?,
                "release-claim" => {
                    if let Some(extension) = extension.take() {
                        extension.release();
                    }
                }
                "reconnect-export" => export = Arc::new(export.reconnect()?),
                "close-export" => export.connection.clone().close()?,
                "exit" => app.closed = true,
                _ => {
                    let owner = if command == "claim-self" {
                        Some(export.owner().to_string())
                    } else if command == "clear-claim" {
                        None
                    } else if let Some(owner) = command.strip_prefix("claim-other ") {
                        Some(owner.to_string())
                    } else {
                        anyhow::bail!("unknown QA claim command")
                    };
                    let extension = extension.get_or_insert_with(|| {
                        app.gtk.as_ref().unwrap().get_gtk_surface(&surface, &qh, ())
                    });
                    extension.set_dbus_properties(
                        Some("org.tarot.MenuEgui".into()),
                        Some(export::MENU.into()),
                        None,
                        Some(export::WINDOW.into()),
                        Some(export::APP.into()),
                        owner,
                    );
                }
            }
            // Acknowledgement observes server handling, not just queued bytes.
            queue.roundtrip(&mut app)?;
            export.shared.claim_mutation(&command);
        }
        app.paint(&connection, &surface, &qh, &export)?;
        queue.flush()?;
        if let Some(guard) = queue.prepare_read() {
            let mut poll = libc::pollfd {
                fd: guard.connection_fd().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            // SAFETY: one live initialized pollfd, read guard retains its socket.
            let result = unsafe { libc::poll(&mut poll, 1, 16) };
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(error.into());
            }
            if poll.revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) != 0 {
                guard.read()?;
            }
        }
        ensure!(
            app.width <= 4096 && app.height <= 4096,
            "sample window size bound"
        );
    }
    app.graphics.take();
    if let Some(extension) = extension {
        extension.release();
    }
    toplevel.destroy();
    xdg.destroy();
    surface.destroy();
    connection.flush()?;
    Ok(())
}
