//! Synthetic text-input/input-method protocol fixture. Never use with private text.
use anyhow::{Context, Result, ensure};
use clap::{Parser, ValueEnum};
use serde_json::{Value, json};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::fd::AsFd,
    path::PathBuf,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle, delegate_noop,
    protocol::{
        wl_buffer, wl_compositor, wl_keyboard, wl_registry, wl_seat, wl_shm, wl_shm_pool,
        wl_surface,
    },
};
use wayland_protocols::{
    wp::text_input::zv3::client::{zwp_text_input_manager_v3, zwp_text_input_v3},
    xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base},
};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_manager_v2, zwp_input_method_v2,
};
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1, zwp_virtual_keyboard_v1,
};

#[derive(Clone, Copy, ValueEnum, PartialEq)]
enum Role {
    App,
    Method,
    Inspect,
}

#[derive(Parser)]
struct Options {
    #[arg(long, value_enum)]
    role: Role,
    #[arg(long)]
    report: PathBuf,
    #[arg(long)]
    commands: PathBuf,
    #[arg(long)]
    try_bind: Option<u32>,
    #[arg(long, default_value_t = 90)]
    exit_after_seconds: u64,
}

struct Probe {
    role: Role,
    log: File,
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    seat: Option<wl_seat::WlSeat>,
    shell: Option<xdg_wm_base::XdgWmBase>,
    text_manager: Option<zwp_text_input_manager_v3::ZwpTextInputManagerV3>,
    method_manager: Option<zwp_input_method_manager_v2::ZwpInputMethodManagerV2>,
    text_input: Option<zwp_text_input_v3::ZwpTextInputV3>,
    method: Option<zwp_input_method_v2::ZwpInputMethodV2>,
    virtual_manager: Option<zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1>,
    virtual_keyboard: Option<zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1>,
    surface: Option<wl_surface::WlSurface>,
    buffer: Option<wl_buffer::WlBuffer>,
    _xdg: Option<xdg_surface::XdgSurface>,
    _top: Option<xdg_toplevel::XdgToplevel>,
    serial: u32,
    entered: bool,
    enabled: bool,
    text: String,
    cursor: usize,
    preedit: String,
    pending_preedit: Option<String>,
    pending_commit: Option<String>,
    pending_delete: Option<(usize, usize)>,
    closed: bool,
}

impl Probe {
    fn record(&mut self, event: Value) {
        writeln!(self.log, "{event}").unwrap();
        self.log.flush().unwrap();
    }
    fn context(&mut self) {
        if !self.entered {
            return;
        }
        let input = self.text_input.as_ref().unwrap();
        input.set_surrounding_text(self.text.clone(), self.cursor as i32, self.cursor as i32);
        input.set_text_change_cause(zwp_text_input_v3::ChangeCause::Other);
        input.set_content_type(
            zwp_text_input_v3::ContentHint::None,
            zwp_text_input_v3::ContentPurpose::Normal,
        );
        input.set_cursor_rectangle(30, 100, 2, 24);
        self.commit_context();
    }
    fn commit_context(&mut self) {
        self.text_input.as_ref().unwrap().commit();
        self.serial = self.serial.wrapping_add(1);
    }
    fn command(&mut self, value: Value) -> Result<()> {
        let command = value["command"].as_str().context("missing command")?;
        if command == "quit" {
            self.closed = true;
            return Ok(());
        }
        if self.role == Role::Method {
            if command == "virtual-key" {
                let keyboard = self.virtual_keyboard.as_ref().unwrap();
                let raw = value["raw"].as_bool().unwrap_or(false);
                if !raw {
                    keyboard.modifiers(value["mods"].as_u64().unwrap_or(0) as u32, 0, 0, 0);
                }
                let key = value["key"].as_u64().unwrap() as u32;
                keyboard.key(0, key, 1);
                keyboard.key(0, key, 0);
                if !raw && value["reset"].as_bool().unwrap_or(true) {
                    keyboard.modifiers(0, 0, 0, 0);
                }
                self.record(json!({"event":"method_sent","command":command}));
                return Ok(());
            }
            let method = self.method.as_ref().unwrap();
            match command {
                "preedit" | "stale" => {
                    let text = value["text"].as_str().unwrap();
                    method.set_preedit_string(text.into(), text.len() as i32, text.len() as i32);
                }
                "commit" => {
                    method.set_preedit_string(String::new(), 0, 0);
                    method.commit_string(value["text"].as_str().unwrap().into());
                }
                "delete" => method.delete_surrounding_text(
                    value["before"].as_u64().unwrap() as u32,
                    value["after"].as_u64().unwrap() as u32,
                ),
                _ => anyhow::bail!("unknown method command"),
            }
            method.commit(if command == "stale" {
                self.serial.wrapping_sub(1)
            } else {
                self.serial
            });
            self.record(json!({"event":"method_sent","command":command,"serial":self.serial}));
        } else {
            match command {
                "disable" => {
                    self.enabled = false;
                    self.text_input.as_ref().unwrap().disable();
                    self.commit_context();
                }
                "enable" => {
                    self.enabled = true;
                    self.text_input.as_ref().unwrap().enable();
                    self.context();
                }
                "context" => {
                    self.text = value["text"].as_str().unwrap().into();
                    self.cursor = value["cursor"].as_u64().unwrap() as usize;
                    ensure!(
                        self.text.is_char_boundary(self.cursor),
                        "cursor must be a UTF-8 boundary"
                    );
                    self.context();
                }
                _ => anyhow::bail!("unknown app command"),
            }
        }
        Ok(())
    }
    fn apply(&mut self, serial: u32) {
        if serial == self.serial {
            if let Some((before, after)) = self.pending_delete.take() {
                let start = self.cursor.saturating_sub(before);
                let end = (self.cursor + after).min(self.text.len());
                assert!(self.text.is_char_boundary(start) && self.text.is_char_boundary(end));
                self.text.replace_range(start..end, "");
                self.cursor = start;
            }
            if let Some(text) = self.pending_commit.take() {
                self.text.insert_str(self.cursor, &text);
                self.cursor += text.len();
            }
            if let Some(text) = self.pending_preedit.take() {
                self.preedit = text;
            }
            self.record(json!({"event":"applied","serial":serial,"text":self.text,"cursor":self.cursor,"preedit":self.preedit}));
            self.context();
        } else {
            self.pending_preedit = None;
            self.pending_commit = None;
            self.pending_delete = None;
            self.record(json!({"event":"discarded","serial":serial,"expected":self.serial,"text":self.text}));
        }
    }
}

fn main() -> Result<()> {
    let options = Options::parse();
    let connection = Connection::connect_to_env()?;
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let registry = connection.display().get_registry(&qh, ());
    let mut state = Probe {
        role: options.role,
        log: File::create(options.report)?,
        compositor: None,
        shm: None,
        seat: None,
        shell: None,
        text_manager: None,
        method_manager: None,
        text_input: None,
        method: None,
        virtual_manager: None,
        virtual_keyboard: None,
        surface: None,
        buffer: None,
        _xdg: None,
        _top: None,
        serial: 0,
        entered: false,
        enabled: true,
        text: "hello ".into(),
        cursor: 6,
        preedit: String::new(),
        pending_preedit: None,
        pending_commit: None,
        pending_delete: None,
        closed: false,
    };
    queue.roundtrip(&mut state)?;
    state.record(json!({"event":"registry","input_method_visible":state.method_manager.is_some(),"text_input_visible":state.text_manager.is_some()}));
    if let Some(name) = options.try_bind {
        let _forbidden = registry
            .bind::<zwp_input_method_manager_v2::ZwpInputMethodManagerV2, _, _>(name, 1, &qh, ());
        queue.roundtrip(&mut state)?;
        anyhow::bail!("ordinary connection bound a privileged global");
    }
    match options.role {
        Role::Method => {
            state.method = Some(
                state
                    .method_manager
                    .as_ref()
                    .context("helper has no input-method global")?
                    .get_input_method(state.seat.as_ref().unwrap(), &qh, ()),
            );
            let keyboard = state
                .virtual_manager
                .as_ref()
                .context("no trusted virtual keyboard")?
                .create_virtual_keyboard(state.seat.as_ref().unwrap(), &qh, ());
            let context = xkbcommon::xkb::Context::new(xkbcommon::xkb::CONTEXT_NO_FLAGS);
            let keymap = xkbcommon::xkb::Keymap::new_from_names(
                &context,
                "",
                "",
                "us",
                "",
                None,
                xkbcommon::xkb::KEYMAP_COMPILE_NO_FLAGS,
            )
            .context("virtual keymap")?;
            let mut bytes = keymap
                .get_as_string(xkbcommon::xkb::KEYMAP_FORMAT_TEXT_V1)
                .into_bytes();
            bytes.push(0);
            let mut file = tempfile::tempfile()?;
            file.write_all(&bytes)?;
            keyboard.keymap(1, file.as_fd(), bytes.len() as u32);
            state.virtual_keyboard = Some(keyboard);
        }
        Role::Inspect => {
            ensure!(
                state.method_manager.is_none(),
                "ordinary app saw privileged global"
            );
            return Ok(());
        }
        Role::App => {
            state.seat.as_ref().unwrap().get_keyboard(&qh, ());
            ensure!(
                state.method_manager.is_none(),
                "ordinary app saw privileged global"
            );
            state.text_input = Some(state.text_manager.as_ref().unwrap().get_text_input(
                state.seat.as_ref().unwrap(),
                &qh,
                (),
            ));
            let surface = state.compositor.as_ref().unwrap().create_surface(&qh, ());
            let xdg = state
                .shell
                .as_ref()
                .unwrap()
                .get_xdg_surface(&surface, &qh, ());
            let top = xdg.get_toplevel(&qh, ());
            top.set_app_id("org.tarot.TextProbe".into());
            top.set_title("Tarot synthetic text fixture".into());
            let mut file = tempfile::tempfile()?;
            file.write_all(&[0x50, 0x80, 0x30, 0xff].repeat(320 * 520))?;
            let pool =
                state
                    .shm
                    .as_ref()
                    .unwrap()
                    .create_pool(file.as_fd(), 320 * 520 * 4, &qh, ());
            state.buffer =
                Some(pool.create_buffer(0, 320, 520, 320 * 4, wl_shm::Format::Argb8888, &qh, ()));
            pool.destroy();
            surface.commit();
            state.surface = Some(surface);
            state._xdg = Some(xdg);
            state._top = Some(top);
        }
    }
    queue.roundtrip(&mut state)?;
    state.record(json!({"event":"ready"}));
    let mut commands = BufReader::new(
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(options.commands)?,
    );
    let start = Instant::now();
    while !state.closed && start.elapsed() < Duration::from_secs(options.exit_after_seconds) {
        queue.roundtrip(&mut state)?;
        let mut line = String::new();
        while commands.read_line(&mut line)? > 0 {
            let value: Value = serde_json::from_str(&line)?;
            let virtual_key = value["command"] == "virtual-key";
            state.command(value)?;
            if virtual_key {
                queue.roundtrip(&mut state)?;
                state.record(json!({"event":"virtual-synced"}));
            }
            line.clear();
        }
        std::thread::sleep(Duration::from_millis(10));
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
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(4), qh, ()))
                }
                "wl_shm" => state.shm = Some(registry.bind(name, 1, qh, ())),
                "wl_seat" => state.seat = Some(registry.bind(name, version.min(7), qh, ())),
                "xdg_wm_base" => state.shell = Some(registry.bind(name, version.min(3), qh, ())),
                "zwp_text_input_manager_v3" => {
                    state.text_manager = Some(registry.bind(name, 1, qh, ()))
                }
                "zwp_input_method_manager_v2" => {
                    state.method_manager = Some(registry.bind(name, 1, qh, ()));
                    state.record(json!({"event":"method_global","name":name}));
                }
                "zwp_virtual_keyboard_manager_v1" => {
                    state.virtual_manager = Some(registry.bind(name, 1, qh, ()));
                }
                _ => {}
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
impl Dispatch<xdg_surface::XdgSurface, ()> for Probe {
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
            let root = state.surface.as_ref().unwrap();
            root.attach(state.buffer.as_ref(), 0, 0);
            root.damage(0, 0, 320, 520);
            root.commit();
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
        if let xdg_toplevel::Event::Close = event {
            state.closed = true;
        }
    }
}
impl Dispatch<zwp_text_input_v3::ZwpTextInputV3, ()> for Probe {
    fn event(
        state: &mut Self,
        input: &zwp_text_input_v3::ZwpTextInputV3,
        event: zwp_text_input_v3::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_text_input_v3::Event::Enter { .. } => {
                state.entered = true;
                state.record(json!({"event":"enter"}));
                if state.enabled {
                    input.enable();
                    state.context();
                }
            }
            zwp_text_input_v3::Event::Leave { .. } => {
                state.entered = false;
                state.pending_commit = None;
                state.pending_preedit = None;
                state.pending_delete = None;
                state.preedit.clear();
                state.record(json!({"event":"leave"}));
            }
            zwp_text_input_v3::Event::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                state.record(
                    json!({"event":"preedit","text":text,"begin":cursor_begin,"end":cursor_end}),
                );
                state.pending_preedit = Some(text.unwrap_or_default());
            }
            zwp_text_input_v3::Event::CommitString { text } => {
                state.pending_commit = Some(text.unwrap_or_default())
            }
            zwp_text_input_v3::Event::DeleteSurroundingText {
                before_length,
                after_length,
            } => state.pending_delete = Some((before_length as usize, after_length as usize)),
            zwp_text_input_v3::Event::Done { serial } => state.apply(serial),
            _ => {}
        }
    }
}
impl Dispatch<zwp_input_method_v2::ZwpInputMethodV2, ()> for Probe {
    fn event(
        state: &mut Self,
        _: &zwp_input_method_v2::ZwpInputMethodV2,
        event: zwp_input_method_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_input_method_v2::Event::Activate => state.record(json!({"event":"activate"})),
            zwp_input_method_v2::Event::Deactivate => state.record(json!({"event":"deactivate"})),
            zwp_input_method_v2::Event::Done => {
                state.serial = state.serial.wrapping_add(1);
                state.record(json!({"event":"method_done","serial":state.serial}));
            }
            zwp_input_method_v2::Event::SurroundingText {
                text,
                cursor,
                anchor,
            } => state
                .record(json!({"event":"surrounding","text":text,"cursor":cursor,"anchor":anchor})),
            zwp_input_method_v2::Event::ContentType { hint, purpose } => state.record(
                json!({"event":"content_type","hint":u32::from(hint),"purpose":u32::from(purpose)}),
            ),
            zwp_input_method_v2::Event::TextChangeCause { cause } => {
                state.record(json!({"event":"cause","value":u32::from(cause)}))
            }
            zwp_input_method_v2::Event::Unavailable => {
                state.record(json!({"event":"unavailable"}));
                state.closed = true;
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
            wl_keyboard::Event::Key {
                key,
                state: key_state,
                ..
            } => {
                state.record(json!({"event":"key","key":key,"state":u32::from(key_state)}));
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                ..
            } => {
                state.record(json!({"event":"modifiers","depressed":mods_depressed,"latched":mods_latched,"locked":mods_locked}));
            }
            _ => {}
        }
    }
}

delegate_noop!(Probe: ignore wl_compositor::WlCompositor);
delegate_noop!(Probe: ignore wl_shm::WlShm);
delegate_noop!(Probe: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Probe: ignore wl_buffer::WlBuffer);
delegate_noop!(Probe: ignore wl_surface::WlSurface);
delegate_noop!(Probe: ignore wl_seat::WlSeat);
delegate_noop!(Probe: ignore zwp_text_input_manager_v3::ZwpTextInputManagerV3);
delegate_noop!(Probe: ignore zwp_input_method_manager_v2::ZwpInputMethodManagerV2);
delegate_noop!(Probe: ignore zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1);
delegate_noop!(Probe: ignore zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1);
