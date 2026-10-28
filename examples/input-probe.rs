//! Native Wayland test client with a deliberately oversized window and a subsurface.
//! It records what the client actually receives, independently of shell diagnostics.
use anyhow::Result;
use clap::Parser;
use serde_json::{Value, json};
use std::{collections::HashMap, fs::File, io::Write, os::fd::AsFd, path::PathBuf};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, WEnum, delegate_noop,
    protocol::{
        wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat,
        wl_shm, wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface, wl_touch,
    },
};
use wayland_protocols::xdg::shell::client::{
    xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base,
};

#[derive(Parser)]
struct Options {
    #[arg(long)]
    report: PathBuf,
    #[arg(long, default_value_t = 60)]
    exit_after_seconds: u64,
    #[arg(long)]
    popups: bool,
    #[arg(long)]
    move_child: bool,
    #[arg(long, value_parser=parse_color)]
    solid_color: Option<[u8; 4]>,
    #[arg(long)]
    fit_output: bool,
    #[arg(long)]
    corner_child: bool,
    #[arg(long)]
    no_child: bool,
    #[arg(long)]
    opaque: bool,
    #[arg(long)]
    refuse_close: bool,
}

fn parse_color(value: &str) -> std::result::Result<[u8; 4], String> {
    if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Expected an RRGGBB color".into());
    }
    let r = u8::from_str_radix(&value[0..2], 16).unwrap();
    let g = u8::from_str_radix(&value[2..4], 16).unwrap();
    let b = u8::from_str_radix(&value[4..6], 16).unwrap();
    Ok([b, g, r, 255])
}

#[derive(Clone, Copy, Debug)]
enum Tag {
    Root,
    Child,
    Popup(u32),
}
impl Tag {
    fn name(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Child => "child",
            Self::Popup(1) => "popup1",
            Self::Popup(_) => "popup2",
        }
    }
}

struct Popup {
    level: u32,
    initiating_serial: u32,
    late_grab: bool,
    surface: wl_surface::WlSurface,
    xdg: xdg_surface::XdgSurface,
    popup: xdg_popup::XdgPopup,
    buffer: wl_buffer::WlBuffer,
}

struct Window {
    surface: wl_surface::WlSurface,
    buffer: wl_buffer::WlBuffer,
    _xdg: xdg_surface::XdgSurface,
    _toplevel: xdg_toplevel::XdgToplevel,
    _child: wl_surface::WlSurface,
    _subsurface: wl_subsurface::WlSubsurface,
    _child_buffer: wl_buffer::WlBuffer,
}

struct Probe {
    refuse_close: bool,
    opaque: bool,
    solid_color: Option<[u8; 4]>,
    fit_output: bool,
    configured_size: (i32, i32),
    corner_child: bool,
    no_child: bool,
    compositor: Option<wl_compositor::WlCompositor>,
    subcompositor: Option<wl_subcompositor::WlSubcompositor>,
    shm: Option<wl_shm::WlShm>,
    shell: Option<xdg_wm_base::XdgWmBase>,
    window: Option<Window>,
    mapped: bool,
    closed: bool,
    touch: Option<wl_touch::WlTouch>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    seat: Option<wl_seat::WlSeat>,
    popup_mode: bool,
    popups: Vec<Popup>,
    pointer_position: (f64, f64),
    moving_child: bool,
    child_restore: bool,
    touch_targets: HashMap<i32, Tag>,
    pointer_target: Option<Tag>,
    report: File,
}

impl Probe {
    fn record(&mut self, event: Value) {
        serde_json::to_writer(&mut self.report, &event).unwrap();
        self.report.write_all(b"\n").unwrap();
        self.report.flush().unwrap();
    }

    fn popup_action(&mut self, target: Tag, serial: u32, x: f64, y: f64, qh: &QueueHandle<Self>) {
        if !self.popup_mode {
            return;
        }
        match target {
            Tag::Root if !self.popups.is_empty() => {
                self.destroy_popups(1);
            }
            Tag::Root => {
                let invalid = x > 500.0 && y > 800.0;
                let corner = x > 500.0 && y < 200.0;
                let late = x < 100.0 && y > 800.0;
                self.open_popup(
                    1,
                    if invalid {
                        serial.wrapping_add(0x4000_0000)
                    } else {
                        serial
                    },
                    corner,
                    late,
                    qh,
                );
            }
            Tag::Popup(1) if self.popups.len() == 1 => {
                self.open_popup(2, self.popups[0].initiating_serial, false, false, qh)
            }
            Tag::Popup(2) => self.destroy_popups(2),
            _ => {}
        }
    }

    fn open_popup(
        &mut self,
        level: u32,
        serial: u32,
        corner: bool,
        late: bool,
        qh: &QueueHandle<Self>,
    ) {
        let surface = self
            .compositor
            .as_ref()
            .unwrap()
            .create_surface(qh, Tag::Popup(level));
        let xdg = self
            .shell
            .as_ref()
            .unwrap()
            .get_xdg_surface(&surface, qh, level);
        let positioner = self.shell.as_ref().unwrap().create_positioner(qh, ());
        positioner.set_size(180, 120);
        let (x, y) = if corner {
            (590, 990)
        } else if level == 1 {
            (200, 200)
        } else {
            (40, 40)
        };
        positioner.set_anchor_rect(x, y, 1, 1);
        positioner.set_anchor(xdg_positioner::Anchor::BottomRight);
        positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
        positioner.set_constraint_adjustment(
            xdg_positioner::ConstraintAdjustment::SlideX
                | xdg_positioner::ConstraintAdjustment::SlideY
                | xdg_positioner::ConstraintAdjustment::FlipX
                | xdg_positioner::ConstraintAdjustment::FlipY,
        );
        let parent = if level == 1 {
            &self.window.as_ref().unwrap()._xdg
        } else {
            &self.popups.last().unwrap().xdg
        };
        let popup = xdg.get_popup(Some(parent), &positioner, qh, level);
        positioner.destroy();
        if !late {
            popup.grab(self.seat.as_ref().unwrap(), serial);
        }
        let buffer = self.buffer(qh, 180, 120, level == 1);
        self.popups.push(Popup {
            level,
            initiating_serial: serial,
            late_grab: late,
            surface,
            xdg,
            popup,
            buffer,
        });
        self.popups.last().unwrap().surface.commit();
        self.record(
            json!({"event":"popup_requested","level":level,"serial":serial,"corner":corner,"late":late}),
        );
    }

    fn destroy_popups(&mut self, level: u32) {
        while self.popups.last().is_some_and(|popup| popup.level >= level) {
            let popup = self.popups.pop().unwrap();
            popup.popup.destroy();
            popup.xdg.destroy();
            popup.surface.destroy();
            popup.buffer.destroy();
            self.record(json!({"event":"popup_destroyed","level":popup.level}));
        }
    }

    fn buffer(
        &self,
        qh: &QueueHandle<Self>,
        width: i32,
        height: i32,
        child: bool,
    ) -> wl_buffer::WlBuffer {
        let mut bytes = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let color = if !child && let Some(color) = self.solid_color {
                    color
                } else if x % 50 < 2 || y % 50 < 2 {
                    [30, 30, 30, 255]
                } else if child {
                    [210, 120, 40, 255]
                } else {
                    [70, (y / 5) as u8, (x / 3) as u8, 255]
                };
                bytes.extend_from_slice(&color);
            }
        }
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&bytes).unwrap();
        let pool = self
            .shm
            .as_ref()
            .unwrap()
            .create_pool(file.as_fd(), width * height * 4, qh, ());
        let buffer = pool.create_buffer(
            0,
            width,
            height,
            width * 4,
            if self.opaque {
                wl_shm::Format::Xrgb8888
            } else {
                wl_shm::Format::Argb8888
            },
            qh,
            (),
        );
        pool.destroy();
        buffer
    }

    fn create_window(&mut self, qh: &QueueHandle<Self>) {
        let compositor = self.compositor.as_ref().unwrap();
        let surface = compositor.create_surface(qh, Tag::Root);
        let child = compositor.create_surface(qh, Tag::Child);
        let subsurface =
            self.subcompositor
                .as_ref()
                .unwrap()
                .get_subsurface(&child, &surface, qh, ());
        let (x, y) = if self.corner_child {
            (-30, -30)
        } else {
            (100, 150)
        };
        subsurface.set_position(x, y);
        subsurface.set_desync();
        let child_buffer = self.buffer(qh, 200, 200, true);
        if !self.no_child {
            child.attach(Some(&child_buffer), 0, 0);
            child.damage(0, 0, 200, 200);
            child.commit();
        }
        let xdg = self
            .shell
            .as_ref()
            .unwrap()
            .get_xdg_surface(&surface, qh, 0_u32);
        let toplevel = xdg.get_toplevel(qh, ());
        toplevel.set_title("Tarot native input probe".into());
        toplevel.set_app_id("org.tarot.InputProbe".into());
        // The compositor must fit this geometry, rather than assume configured == actual size.
        if !self.fit_output {
            xdg.set_window_geometry(20, 30, 600, 1000);
        }
        let buffer = self.buffer(qh, 640, 1060, false);
        self.window = Some(Window {
            surface,
            buffer,
            _xdg: xdg,
            _toplevel: toplevel,
            _child: child,
            _subsurface: subsurface,
            _child_buffer: child_buffer,
        });
        self.window.as_ref().unwrap().surface.commit();
    }
}

fn main() -> Result<()> {
    let options = Options::parse();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(options.exit_after_seconds));
        std::process::exit(0);
    });
    let connection = Connection::connect_to_env()?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut probe = Probe {
        refuse_close: options.refuse_close,
        opaque: options.opaque,
        solid_color: options.solid_color,
        fit_output: options.fit_output,
        configured_size: (360, 652),
        corner_child: options.corner_child,
        no_child: options.no_child,
        compositor: None,
        subcompositor: None,
        shm: None,
        shell: None,
        window: None,
        mapped: false,
        closed: false,
        touch: None,
        pointer: None,
        keyboard: None,
        seat: None,
        popup_mode: options.popups,
        popups: Vec::new(),
        pointer_position: (0.0, 0.0),
        moving_child: options.move_child,
        child_restore: false,
        touch_targets: HashMap::new(),
        pointer_target: None,
        report: File::create(options.report)?,
    };
    connection.display().get_registry(&qh, ());
    queue.roundtrip(&mut probe)?;
    probe.create_window(&qh);
    while !probe.closed {
        queue.blocking_dispatch(&mut probe)?;
    }
    Ok(())
}

impl Dispatch<wl_registry::WlRegistry, ()> for Probe {
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
                "wl_compositor" => state.compositor = Some(registry.bind(name, 1, qh, ())),
                "wl_subcompositor" => state.subcompositor = Some(registry.bind(name, 1, qh, ())),
                "wl_shm" => state.shm = Some(registry.bind(name, 1, qh, ())),
                "xdg_wm_base" => state.shell = Some(registry.bind(name, version.min(3), qh, ())),
                "wl_seat" => {
                    state.seat = Some(registry.bind(name, version.min(7), qh, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Probe {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        {
            if caps.contains(wl_seat::Capability::Touch) && state.touch.is_none() {
                state.touch = Some(seat.get_touch(qh, ()));
            }
            if caps.contains(wl_seat::Capability::Pointer) && state.pointer.is_none() {
                state.pointer = Some(seat.get_pointer(qh, ()));
            }
            if caps.contains(wl_seat::Capability::Keyboard) && state.keyboard.is_none() {
                state.keyboard = Some(seat.get_keyboard(qh, ()));
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for Probe {
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
impl Dispatch<xdg_surface::XdgSurface, u32> for Probe {
    fn event(
        state: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        level: &u32,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
            if *level > 0 {
                if let Some(popup) = state.popups.iter().find(|popup| popup.level == *level) {
                    popup.surface.attach(Some(&popup.buffer), 0, 0);
                    popup.surface.damage(0, 0, 180, 120);
                    popup.surface.commit();
                    if popup.late_grab {
                        popup
                            .popup
                            .grab(state.seat.as_ref().unwrap(), popup.initiating_serial);
                    }
                }
            } else if state.fit_output {
                let (width, height) = state.configured_size;
                let buffer = state.buffer(qh, width, height, false);
                let window = state.window.as_mut().unwrap();
                window._xdg.set_window_geometry(0, 0, width, height);
                let previous = std::mem::replace(&mut window.buffer, buffer);
                window.surface.attach(Some(&window.buffer), 0, 0);
                window.surface.damage(0, 0, width, height);
                window.surface.commit();
                previous.destroy();
                state.mapped = true;
                state.record(json!({"event":"fit-configured","width":width,"height":height}));
            } else if !state.mapped {
                let window = state.window.as_ref().unwrap();
                window.surface.attach(Some(&window.buffer), 0, 0);
                window.surface.damage(0, 0, 640, 1060);
                window.surface.commit();
                state.mapped = true;
                state.record(json!({"event":"mapped"}));
            }
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel, ()> for Probe {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Close => {
                state.record(json!({"event":"close_requested","refused":state.refuse_close}));
                state.closed = !state.refuse_close;
            }
            xdg_toplevel::Event::Configure { width, height, .. }
                if state.fit_output && width > 0 && height > 0 =>
            {
                state.configured_size = (width, height);
            }
            _ => {}
        }
    }
}
impl Dispatch<wl_touch::WlTouch, ()> for Probe {
    fn event(
        state: &mut Self,
        _: &wl_touch::WlTouch,
        event: wl_touch::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_touch::Event::Down {
                id,
                surface,
                x,
                y,
                time,
                serial,
            } => {
                let target = *surface.data::<Tag>().unwrap();
                state.touch_targets.insert(id, target);
                state.record(json!({"event":"touch_down","id":id,"target":target.name(),"x":x,"y":y,"time":time}));
                if matches!(target, Tag::Child) && state.moving_child {
                    let window = state.window.as_ref().unwrap();
                    window._subsurface.set_position(150, 200);
                    window._child.commit();
                    window.surface.frame(qh, "child_move_presented");
                    window.surface.commit();
                    state.moving_child = false;
                    state.child_restore = true;
                }
                state.popup_action(target, serial, x, y, qh);
            }
            wl_touch::Event::Motion { id, x, y, time } => {
                let target = state.touch_targets.get(&id).unwrap().name();
                state.record(
                    json!({"event":"touch_motion","id":id,"target":target,"x":x,"y":y,"time":time}),
                );
            }
            wl_touch::Event::Up { id, time, .. } => {
                let target = state.touch_targets.remove(&id).unwrap().name();
                state.record(json!({"event":"touch_up","id":id,"target":target,"time":time}));
                if state.child_restore {
                    let window = state.window.as_ref().unwrap();
                    window._subsurface.set_position(100, 150);
                    window._child.commit();
                    window.surface.frame(qh, "child_restore_presented");
                    window.surface.commit();
                    state.child_restore = false;
                }
            }
            wl_touch::Event::Cancel => {
                state.touch_targets.clear();
                state.record(json!({"event":"touch_cancel"}));
            }
            _ => {}
        }
    }
}
impl Dispatch<wl_pointer::WlPointer, ()> for Probe {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                let target = *surface.data::<Tag>().unwrap();
                state.pointer_target = Some(target);
                state.pointer_position = (surface_x, surface_y);
                state.record(json!({"event":"pointer_enter","target":target.name(),"x":surface_x,"y":surface_y}));
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                time,
            } => {
                state.pointer_position = (surface_x, surface_y);
                state.record(json!({"event":"pointer_motion","target":state.pointer_target.map(Tag::name),"x":surface_x,"y":surface_y,"time":time}));
            }
            wl_pointer::Event::Button {
                button,
                state: button_state,
                time,
                serial,
            } => {
                state.record(json!({"event":"pointer_button","button":button,"pressed":button_state==WEnum::Value(wl_pointer::ButtonState::Pressed),"time":time}));
                if button_state == WEnum::Value(wl_pointer::ButtonState::Pressed)
                    && let Some(target) = state.pointer_target
                {
                    state.popup_action(
                        target,
                        serial,
                        state.pointer_position.0,
                        state.pointer_position.1,
                        qh,
                    );
                }
            }
            wl_pointer::Event::Leave { .. } => {
                state.pointer_target = None;
                state.record(json!({"event":"pointer_leave"}));
            }
            _ => {}
        }
    }
}

impl Dispatch<xdg_popup::XdgPopup, u32> for Probe {
    fn event(
        state: &mut Self,
        _: &xdg_popup::XdgPopup,
        event: xdg_popup::Event,
        level: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_popup::Event::Configure {
                x,
                y,
                width,
                height,
            } => {
                state.record(json!({"event":"popup_configure","level":level,"x":x,"y":y,"width":width,"height":height}));
            }
            xdg_popup::Event::PopupDone => {
                state.record(json!({"event":"popup_done","level":level}));
                state.destroy_popups(*level);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Probe {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { surface, .. } => {
                state.record(json!({"event":"keyboard_enter","target":surface.data::<Tag>().unwrap().name()}));
            }
            wl_keyboard::Event::Leave { surface, .. } => {
                // A queued leave may refer to a popup the client has just
                // destroyed while handling popup_done. Its proxy data is gone.
                state.record(json!({"event":"keyboard_leave","target":surface.data::<Tag>().map(|tag| tag.name())}));
            }
            wl_keyboard::Event::Key {
                key: 1,
                state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => {
                if let Some(popup) = state.popups.last() {
                    state.destroy_popups(popup.level);
                }
            }
            _ => {}
        }
    }
}

delegate_noop!(Probe: ignore wl_compositor::WlCompositor);
delegate_noop!(Probe: ignore wl_subcompositor::WlSubcompositor);
delegate_noop!(Probe: ignore wl_shm::WlShm);
delegate_noop!(Probe: ignore wl_shm_pool::WlShmPool);
impl Dispatch<wl_surface::WlSurface, Tag> for Probe {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &Tag,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
delegate_noop!(Probe: ignore wl_subsurface::WlSubsurface);
delegate_noop!(Probe: ignore wl_buffer::WlBuffer);
delegate_noop!(Probe: ignore xdg_positioner::XdgPositioner);
impl Dispatch<wl_callback::WlCallback, &'static str> for Probe {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        label: &&'static str,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.record(json!({"event":label}));
    }
}
