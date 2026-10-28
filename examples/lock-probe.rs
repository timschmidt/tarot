//! Synthetic session-lock/clipboard/input fixture. Never an authentication UI.
use anyhow::{Context, Result};
use clap::Parser;
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::{fd::AsFd, unix::net::UnixStream},
    path::PathBuf,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle, WEnum, delegate_noop, event_created_child,
    protocol::{
        wl_buffer, wl_callback, wl_compositor, wl_data_device, wl_data_device_manager,
        wl_data_offer, wl_data_source, wl_keyboard, wl_output, wl_pointer, wl_region, wl_registry,
        wl_seat, wl_shm, wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface, wl_touch,
    },
};
use wayland_protocols::{
    ext::session_lock::v1::client::{
        ext_session_lock_manager_v1, ext_session_lock_surface_v1, ext_session_lock_v1,
    },
    wp::idle_inhibit::zv1::client::{zwp_idle_inhibit_manager_v1, zwp_idle_inhibitor_v1},
    xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base},
};
const CLIPBOARD: &[u8] = b"Tarot synthetic clipboard fixture";
#[derive(Parser)]
struct Options {
    #[arg(long)]
    role: String,
    #[arg(long)]
    no_lock_buffer: bool,
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    commands: Option<PathBuf>,
    #[arg(long)]
    lock_global_id: Option<u32>,
    #[arg(long, default_value_t = 180)]
    exit_after_seconds: u64,
}
struct State {
    no_lock_buffer: bool,
    role: String,
    wait_resize_frame: bool,
    pending_resize: Option<(u32, u32)>,
    resize_ready: bool,
    report: File,
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    output: Option<wl_output::WlOutput>,
    manager: Option<ext_session_lock_manager_v1::ExtSessionLockManagerV1>,
    manager_id: Option<u32>,
    lock: Option<ext_session_lock_v1::ExtSessionLockV1>,
    lock_surface: Option<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1>,
    surface: Option<wl_surface::WlSurface>,
    seat: Option<wl_seat::WlSeat>,
    data_manager: Option<wl_data_device_manager::WlDataDeviceManager>,
    data_device: Option<wl_data_device::WlDataDevice>,
    source: Option<wl_data_source::WlDataSource>,
    old_offer: Option<wl_data_offer::WlDataOffer>,
    selection_sent: bool,
    wm: Option<xdg_wm_base::XdgWmBase>,
    xdg: Option<xdg_surface::XdgSurface>,
    top: Option<xdg_toplevel::XdgToplevel>,
    buffers: Vec<(File, wl_buffer::WlBuffer)>,
    expect_error: bool,
    idle_manager: Option<zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1>,
    inhibitors: Vec<zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1>,
    subcompositor: Option<wl_subcompositor::WlSubcompositor>,
    cover: Option<(wl_surface::WlSurface, wl_subsurface::WlSubsurface)>,
}
impl State {
    fn record(&mut self, event: &str, fields: Value) {
        let _ = writeln!(
            self.report,
            "{}",
            json!({"pid":std::process::id(),"role":self.role,"event":event,"fields":fields})
        );
        let _ = self.report.flush();
    }
    fn buffer(
        &mut self,
        qh: &QueueHandle<Self>,
        w: u32,
        h: u32,
        color: [u8; 4],
    ) -> wl_buffer::WlBuffer {
        assert!((1..=4096).contains(&w) && (1..=4096).contains(&h));
        let mut file = tempfile::tempfile().unwrap();
        let pixels = color.repeat(w as usize * h as usize);
        file.write_all(&pixels).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        let pool =
            self.shm
                .as_ref()
                .unwrap()
                .create_pool(file.as_fd(), pixels.len() as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            w as i32,
            h as i32,
            w as i32 * 4,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );
        pool.destroy();
        self.buffers.push((file, buffer.clone()));
        buffer
    }
    fn start_lock(&mut self, qh: &QueueHandle<Self>) {
        let lock = self.manager.as_ref().unwrap().lock(qh, ());
        let surface = self.compositor.as_ref().unwrap().create_surface(qh, ());
        self.lock_surface =
            Some(lock.get_lock_surface(&surface, self.output.as_ref().unwrap(), qh, ()));
        self.surface = Some(surface);
        self.lock = Some(lock);
        self.record("lock-requested", json!({}));
    }
    fn command(&mut self, action: &str, qh: &QueueHandle<Self>) -> Option<UnixStream> {
        match action {
            "checkpoint" => self.record("checkpoint", json!({})),
            "defer-next-configure" => {
                self.wait_resize_frame = true;
                self.resize_ready = false;
                self.record("defer-armed", json!({}));
            }
            "finish-resize" => {
                assert!(self.resize_ready);
                let (width, height) = self.pending_resize.take().unwrap();
                self.paint_lock(qh, width, height);
            }
            "cover" => {
                let child = self.compositor.as_ref().unwrap().create_surface(qh, ());
                let sub = self.subcompositor.as_ref().unwrap().get_subsurface(
                    &child,
                    self.surface.as_ref().unwrap(),
                    qh,
                    (),
                );
                let region = self.compositor.as_ref().unwrap().create_region(qh, ());
                region.add(0, 0, 160, 240);
                child.set_opaque_region(Some(&region));
                region.destroy();
                let buffer = self.buffer(qh, 160, 240, [0x55, 0x55, 0x55, 0xff]);
                child.attach(Some(&buffer), 0, 0);
                child.damage(0, 0, 160, 240);
                child.commit();
                self.surface.as_ref().unwrap().commit();
                self.cover = Some((child, sub));
                self.record("covered", json!({}));
            }
            "uncover" => {
                let (child, sub) = self.cover.take().unwrap();
                sub.destroy();
                child.destroy();
                self.surface.as_ref().unwrap().commit();
                self.record("uncovered", json!({}));
            }
            "inhibit" => {
                let inhibitor = self.idle_manager.as_ref().unwrap().create_inhibitor(
                    self.surface.as_ref().unwrap(),
                    qh,
                    (),
                );
                self.inhibitors.push(inhibitor);
                self.record("inhibited", json!({"count":self.inhibitors.len()}));
            }
            "uninhibit" => {
                self.inhibitors.pop().unwrap().destroy();
                self.record("uninhibited", json!({"count":self.inhibitors.len()}));
            }
            "unmap" => {
                let surface = self.surface.as_ref().unwrap();
                surface.attach(None, 0, 0);
                surface.commit();
                self.record("unmapped", json!({}));
            }
            "lock" => self.start_lock(qh),
            "unlock" => {
                self.lock.take().unwrap().unlock_and_destroy();
                self.record("unlock-requested", json!({}));
            }
            "invalid-second-unlock" => {
                let bad = self.manager.as_ref().unwrap().lock(qh, ());
                bad.unlock_and_destroy();
                self.expect_error = true;
            }
            "destroy-lock-surface" => {
                self.lock_surface.take().unwrap().destroy();
                self.record("lock-surface-destroyed", json!({}));
            }
            "read-clipboard" => {
                let (read, write) = UnixStream::pair().unwrap();
                read.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                self.old_offer
                    .as_ref()
                    .unwrap()
                    .receive("text/plain".into(), write.as_fd());
                return Some(read);
            }
            _ => panic!("Unknown synthetic command"),
        }
        None
    }
    fn paint_lock(&mut self, qh: &QueueHandle<Self>, width: u32, height: u32) {
        let buffer = self.buffer(qh, width, height, [0x99, 0x66, 0x33, 0xff]);
        let surface = self.surface.as_ref().unwrap();
        surface.attach(Some(&buffer), 0, 0);
        surface.damage(0, 0, width as i32, height as i32);
        surface.commit();
        self.record("configured", json!({"width":width,"height":height}));
    }
}
impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        reg: &wl_registry::WlRegistry,
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
                "wl_compositor" => state.compositor = Some(reg.bind(name, version.min(4), qh, ())),
                "wl_shm" => state.shm = Some(reg.bind(name, 1, qh, ())),
                "wl_subcompositor" => state.subcompositor = Some(reg.bind(name, 1, qh, ())),
                "wl_output" if state.output.is_none() => {
                    state.output = Some(reg.bind(name, version.min(3), qh, ()))
                }
                "wl_seat" => state.seat = Some(reg.bind(name, version.min(5), qh, ())),
                "wl_data_device_manager" => {
                    state.data_manager = Some(reg.bind(name, version.min(3), qh, ()))
                }
                "xdg_wm_base" => state.wm = Some(reg.bind(name, version.min(3), qh, ())),
                "zwp_idle_inhibit_manager_v1" => {
                    state.idle_manager = Some(reg.bind(name, 1, qh, ()))
                }
                "ext_session_lock_manager_v1" => {
                    state.manager_id = Some(name);
                    state.manager = Some(reg.bind(name, 1, qh, ()));
                }
                _ => {}
            }
        }
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(c),
        } = event
        {
            if c.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(qh, ());
            }
            if c.contains(wl_seat::Capability::Pointer) {
                seat.get_pointer(qh, ());
            }
            if c.contains(wl_seat::Capability::Touch) {
                seat.get_touch(qh, ());
            }
        }
    }
}
impl Dispatch<wl_keyboard::WlKeyboard, ()> for State {
    fn event(
        s: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { serial, keys, .. } => {
                if !s.selection_sent
                    && let (Some(d), Some(source)) = (&s.data_device, &s.source)
                {
                    d.set_selection(Some(source), serial);
                    s.selection_sent = true;
                }
                s.record("keyboard-enter", json!({"held_count":keys.len()/4}));
            }
            wl_keyboard::Event::Leave { .. } => s.record("keyboard-leave", json!({})),
            wl_keyboard::Event::Key { key, state, .. } => {
                s.record("key", json!({"code":key,"state":format!("{state:?}")}))
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                ..
            } => {
                s.record(
                    "modifiers",
                    json!({"depressed":mods_depressed,"latched":mods_latched,"locked":mods_locked}),
                );
            }
            _ => {}
        }
    }
}
impl Dispatch<wl_pointer::WlPointer, ()> for State {
    fn event(
        s: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter { .. } => s.record("pointer-enter", json!({})),
            wl_pointer::Event::Leave { .. } => s.record("pointer-leave", json!({})),
            wl_pointer::Event::Button { button, state, .. } => s.record(
                "button",
                json!({"button":button,"state":format!("{state:?}")}),
            ),
            _ => {}
        }
    }
}
impl Dispatch<wl_touch::WlTouch, ()> for State {
    fn event(
        s: &mut Self,
        _: &wl_touch::WlTouch,
        event: wl_touch::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_touch::Event::Down { id, .. } => s.record("touch-down", json!({"id":id})),
            wl_touch::Event::Up { id, .. } => s.record("touch-up", json!({"id":id})),
            wl_touch::Event::Cancel => s.record("touch-cancel", json!({})),
            _ => {}
        }
    }
}
impl Dispatch<wl_data_device::WlDataDevice, ()> for State {
    fn event(
        s: &mut Self,
        _: &wl_data_device::WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_device::Event::Selection { id: Some(offer) } = event
            && s.old_offer.is_none()
        {
            s.old_offer = Some(offer);
            s.record("clipboard-offered", json!({}));
        }
    }
    event_created_child!(State,wl_data_device::WlDataDevice,[0=>(wl_data_offer::WlDataOffer,())]);
}
impl Dispatch<wl_data_source::WlDataSource, ()> for State {
    fn event(
        s: &mut Self,
        _: &wl_data_source::WlDataSource,
        event: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_source::Event::Send { fd, .. } = event {
            let mut file = File::from(fd);
            file.write_all(CLIPBOARD).unwrap();
            s.record("clipboard-sent", json!({}));
        }
    }
}
impl Dispatch<ext_session_lock_v1::ExtSessionLockV1, ()> for State {
    fn event(
        s: &mut Self,
        _: &ext_session_lock_v1::ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_session_lock_v1::Event::Locked => s.record("locked", json!({})),
            ext_session_lock_v1::Event::Finished => s.record("finished", json!({})),
            _ => {}
        }
    }
}
impl Dispatch<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1, ()> for State {
    fn event(
        s: &mut Self,
        lock: &ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            lock.ack_configure(serial);
            if s.no_lock_buffer {
                s.record("configure-without-buffer", json!({}));
                return;
            }
            if s.wait_resize_frame {
                s.wait_resize_frame = false;
                s.pending_resize = Some((width, height));
                let surface = s.surface.as_ref().unwrap();
                surface.frame(qh, ());
                surface.commit();
                s.record(
                    "configure-awaiting-frame",
                    json!({"width":width,"height":height}),
                );
                return;
            }
            s.paint_lock(qh, width, height);
        }
    }
}
impl Dispatch<wl_callback::WlCallback, ()> for State {
    fn event(
        s: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if s.pending_resize.is_some() {
            s.resize_ready = true;
            s.record("resize-frame-done", json!({}));
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
    fn event(
        _: &mut Self,
        base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            base.pong(serial);
        }
    }
}
impl Dispatch<xdg_surface::XdgSurface, ()> for State {
    fn event(
        s: &mut Self,
        xdg: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg.ack_configure(serial);
            if s.buffers.is_empty() {
                let buffer = s.buffer(qh, 160, 240, [0x20, 0x60, 0xb0, 0xff]);
                let surface = s.surface.as_ref().unwrap();
                surface.attach(Some(&buffer), 0, 0);
                surface.damage(0, 0, 160, 240);
                surface.commit();
                s.record("app-mapped", json!({}));
            }
        }
    }
}
delegate_noop!(State:ignore wl_compositor::WlCompositor);
delegate_noop!(State:ignore wl_surface::WlSurface);
delegate_noop!(State:ignore wl_shm::WlShm);
delegate_noop!(State:ignore wl_shm_pool::WlShmPool);
delegate_noop!(State:ignore wl_buffer::WlBuffer);
delegate_noop!(State:ignore wl_output::WlOutput);
delegate_noop!(State:ignore wl_data_device_manager::WlDataDeviceManager);
delegate_noop!(State:ignore wl_data_offer::WlDataOffer);
delegate_noop!(State:ignore ext_session_lock_manager_v1::ExtSessionLockManagerV1);
delegate_noop!(State:ignore xdg_toplevel::XdgToplevel);
fn main() -> Result<()> {
    let options = Options::parse();
    let report = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&options.report)?;
    let connection = Connection::connect_to_env()?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let registry = connection.display().get_registry(&qh, ());
    let mut state = State {
        no_lock_buffer: options.no_lock_buffer,
        role: options.role.clone(),
        wait_resize_frame: false,
        pending_resize: None,
        resize_ready: false,
        report,
        compositor: None,
        shm: None,
        output: None,
        manager: None,
        manager_id: None,
        lock: None,
        lock_surface: None,
        surface: None,
        seat: None,
        data_manager: None,
        data_device: None,
        source: None,
        old_offer: None,
        selection_sent: false,
        wm: None,
        xdg: None,
        top: None,
        buffers: Vec::new(),
        expect_error: false,
        idle_manager: None,
        inhibitors: Vec::new(),
        subcompositor: None,
        cover: None,
    };
    queue.roundtrip(&mut state)?;
    if state.role == "denied" {
        anyhow::ensure!(
            state.manager.is_none(),
            "Ordinary client saw the lock manager"
        );
        let _: ext_session_lock_manager_v1::ExtSessionLockManagerV1 = registry.bind(
            options
                .lock_global_id
                .context("Need the known hidden global ID")?,
            1,
            &qh,
            (),
        );
        anyhow::ensure!(
            queue.roundtrip(&mut state).is_err(),
            "Hidden manager bind succeeded"
        );
        state.record("bind-denied", json!({}));
        return Ok(());
    }
    if state.role == "app" {
        let device = state.data_manager.as_ref().unwrap().get_data_device(
            state.seat.as_ref().unwrap(),
            &qh,
            (),
        );
        let source = state
            .data_manager
            .as_ref()
            .unwrap()
            .create_data_source(&qh, ());
        source.offer("text/plain".into());
        state.data_device = Some(device);
        state.source = Some(source);
        let surface = state.compositor.as_ref().unwrap().create_surface(&qh, ());
        let xdg = state
            .wm
            .as_ref()
            .unwrap()
            .get_xdg_surface(&surface, &qh, ());
        let top = xdg.get_toplevel(&qh, ());
        top.set_title("Tarot synthetic private app".into());
        top.set_app_id("tarot.lock-probe".into());
        surface.commit();
        state.surface = Some(surface);
        state.xdg = Some(xdg);
        state.top = Some(top);
    } else {
        anyhow::ensure!(
            state.role == "locker" && state.manager.is_some(),
            "Locker requires a private trusted connection"
        );
    }
    let mut commands_seen = options
        .commands
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map_or(0, |s| s.lines().count());
    // Readiness includes the command baseline: a controller may append its
    // first command immediately after observing this event.
    state.record("ready", json!({"manager_id":state.manager_id}));
    let deadline = Instant::now() + Duration::from_secs(options.exit_after_seconds);
    while Instant::now() < deadline {
        if let Err(error) = queue.roundtrip(&mut state) {
            if state.expect_error {
                state.record("invalid-unlock-rejected", json!({}));
                return Ok(());
            }
            return Err(error.into());
        }
        if let Some(path) = &options.commands
            && let Ok(text) = std::fs::read_to_string(path)
        {
            let lines: Vec<_> = text.lines().collect();
            for line in lines.iter().skip(commands_seen) {
                let cmd: Value = serde_json::from_str(line)?;
                let action = cmd["action"].as_str().context("Need synthetic action")?;
                if action == "exit" {
                    return Ok(());
                }
                let read = state.command(action, &qh);
                if let Err(error) = queue.roundtrip(&mut state) {
                    if state.expect_error {
                        state.record("invalid-unlock-rejected", json!({}));
                        return Ok(());
                    }
                    return Err(error.into());
                }
                if let Some(mut read) = read {
                    let mut bytes = Vec::new();
                    read.read_to_end(&mut bytes)?;
                    state.record(
                        "clipboard-read",
                        json!({"bytes":bytes.len(),"matches":bytes==CLIPBOARD}),
                    );
                }
            }
            commands_seen = lines.len();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}

delegate_noop!(State: ignore zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1);
delegate_noop!(State: ignore zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1);

delegate_noop!(State: ignore wl_subcompositor::WlSubcompositor);
delegate_noop!(State: ignore wl_subsurface::WlSubsurface);
delegate_noop!(State: ignore wl_region::WlRegion);
