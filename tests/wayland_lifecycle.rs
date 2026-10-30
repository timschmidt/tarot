//! Real protocol regression: run against an empty, isolated Tarot development session.
//! See BUILDING.md; this deliberately does not connect during ordinary unit tests.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    os::{fd::AsFd, unix::net::UnixStream},
    path::PathBuf,
    time::{Duration, Instant},
};

use serde_json::{Value, json};
#[path = "../examples/native_menu_egui/gtk.rs"]
mod gtk_protocol;
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, delegate_noop,
    protocol::{
        wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat,
        wl_shm, wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface, wl_touch,
    },
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3, zwp_text_input_v3,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols::xdg::dialog::v1::client::{xdg_dialog_v1, xdg_wm_dialog_v1};
use wayland_protocols::xdg::shell::client::{
    xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base,
};

#[derive(Default)]
struct Client {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    shell: Option<xdg_wm_base::XdgWmBase>,
    dialogs: Option<xdg_wm_dialog_v1::XdgWmDialogV1>,
    gtk: Option<gtk_protocol::gtk_shell1::GtkShell1>,
    gtk_capabilities: Option<u32>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    text_manager: Option<zwp_text_input_manager_v3::ZwpTextInputManagerV3>,
    text_enabled: bool,
    pending_commit: Option<String>,
    ime_commits: Vec<String>,
    subcompositor: Option<wl_subcompositor::WlSubcompositor>,
    configures: usize,
    sizes: Vec<(xdg_toplevel::XdgToplevel, i32, i32)>,
    states: Vec<(xdg_toplevel::XdgToplevel, Vec<u32>)>,
    capabilities: HashMap<xdg_toplevel::XdgToplevel, Vec<u32>>,
    close_requests: usize,
    close_targets: Vec<xdg_toplevel::XdgToplevel>,
    frames: Vec<u32>,
    focus: Option<wl_surface::WlSurface>,
    pointer_focus: Option<wl_surface::WlSurface>,
    pointer_position: (f64, f64),
    presses: Vec<(wl_surface::WlSurface, f64, f64)>,
    keys: Vec<wl_surface::WlSurface>,
    seat: Option<wl_seat::WlSeat>,
    press_serial: Option<u32>,
    popup_rects: HashMap<xdg_popup::XdgPopup, (i32, i32, i32, i32)>,
}

struct Window {
    surface: wl_surface::WlSurface,
    xdg: xdg_surface::XdgSurface,
    toplevel: xdg_toplevel::XdgToplevel,
    buffer: wl_buffer::WlBuffer,
}

struct Popup {
    surface: wl_surface::WlSurface,
    xdg: xdg_surface::XdgSurface,
    popup: xdg_popup::XdgPopup,
    buffer: wl_buffer::WlBuffer,
}

impl Popup {
    fn map(&self) {
        self.surface.attach(Some(&self.buffer), 0, 0);
        self.surface.damage(0, 0, 500, 500);
        self.surface.commit();
    }
    fn destroy(self) {
        self.popup.destroy();
        self.xdg.destroy();
        self.surface.destroy();
        self.buffer.destroy();
    }
}

impl Client {
    fn popup(&self, qh: &QueueHandle<Self>, parent: &Window, grab: bool) -> Popup {
        let surface = self.compositor.as_ref().unwrap().create_surface(qh, ());
        let xdg = self
            .shell
            .as_ref()
            .unwrap()
            .get_xdg_surface(&surface, qh, ());
        let positioner = self.shell.as_ref().unwrap().create_positioner(qh, ());
        positioner.set_size(140, 100);
        positioner.set_anchor_rect(70, 60, 1, 1);
        positioner.set_anchor(xdg_positioner::Anchor::BottomRight);
        positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
        positioner.set_constraint_adjustment(
            xdg_positioner::ConstraintAdjustment::SlideX
                | xdg_positioner::ConstraintAdjustment::SlideY
                | xdg_positioner::ConstraintAdjustment::FlipX
                | xdg_positioner::ConstraintAdjustment::FlipY,
        );
        let popup = xdg.get_popup(Some(&parent.xdg), &positioner, qh, ());
        positioner.destroy();
        if grab {
            popup.grab(self.seat.as_ref().unwrap(), self.press_serial.unwrap());
        }
        // Deliberate extra buffer content outside the declared popup geometry
        // stresses the outer card clip as well as ordinary menu overflow.
        xdg.set_window_geometry(0, 0, 140, 100);
        surface.commit();
        let buffer = self.buffer(qh, 500, 500, [0xdd, 0x55, 0x22, 0xff]);
        Popup {
            surface,
            xdg,
            popup,
            buffer,
        }
    }
    fn window(&self, qh: &QueueHandle<Self>, title: &str) -> Window {
        let window = self.uncommitted_window(qh, title, None);
        window.surface.commit();
        window
    }

    fn uncommitted_window(
        &self,
        qh: &QueueHandle<Self>,
        title: &str,
        parent: Option<&xdg_toplevel::XdgToplevel>,
    ) -> Window {
        let surface = self.compositor.as_ref().unwrap().create_surface(qh, ());
        self.window_on_surface(qh, surface, title, parent)
    }

    fn window_on_surface(
        &self,
        qh: &QueueHandle<Self>,
        surface: wl_surface::WlSurface,
        title: &str,
        parent: Option<&xdg_toplevel::XdgToplevel>,
    ) -> Window {
        let xdg = self
            .shell
            .as_ref()
            .unwrap()
            .get_xdg_surface(&surface, qh, ());
        let toplevel = xdg.get_toplevel(qh, ());
        toplevel.set_title(title.into());
        toplevel.set_app_id("org.tarot.LifecycleProbe".into());
        toplevel.set_parent(parent);
        // A declared window geometry without a root buffer is still unmapped.
        xdg.set_window_geometry(0, 0, 160, 240);

        let buffer = self.buffer(qh, 160, 240, [0x48, 0x80, 0x20, 0xff]);
        Window {
            surface,
            xdg,
            toplevel,
            buffer,
        }
    }

    fn buffer(
        &self,
        qh: &QueueHandle<Self>,
        width: i32,
        height: i32,
        color: [u8; 4],
    ) -> wl_buffer::WlBuffer {
        self.pixel_buffer(qh, width, height, &color.repeat((width * height) as usize))
    }

    fn pixel_buffer(
        &self,
        qh: &QueueHandle<Self>,
        width: i32,
        height: i32,
        pixels: &[u8],
    ) -> wl_buffer::WlBuffer {
        assert_eq!(pixels.len(), (width * height * 4) as usize);
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(pixels).unwrap();
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
            wl_shm::Format::Argb8888,
            qh,
            (),
        );
        pool.destroy();
        buffer
    }
}

#[test]
#[ignore = "requires an empty running Tarot session; see docs/BUILDING.md"]
fn initial_configure_waits_for_parent_hints_and_fresh_remap_commit() {
    let initial = status();
    assert_eq!(initial["cards"], json!([]));
    let socket = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap())
        .join(initial["wayland_display"].as_str().unwrap());
    let connection = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut client = Client::default();
    connection.display().get_registry(&qh, ());
    sync(&mut queue, &mut client);
    let root = client.uncommitted_window(&qh, "Initial root", None);
    sync(&mut queue, &mut client);
    assert!(
        client.sizes.iter().all(|(top, _, _)| top != &root.toplevel),
        "do not configure before the initial surface commit"
    );
    root.surface.commit();
    sync(&mut queue, &mut client);
    assert!(
        client
            .sizes
            .iter()
            .any(|(top, w, h)| top == &root.toplevel && *w > 0 && *h > 0)
    );
    root.map();
    sync(&mut queue, &mut client);
    let child = client.uncommitted_window(&qh, "Natural dialog", Some(&root.toplevel));
    sync(&mut queue, &mut client);
    assert!(
        client
            .sizes
            .iter()
            .all(|(top, _, _)| top != &child.toplevel),
        "parent_changed and focus updates must not send an initial configure early"
    );
    child.surface.commit();
    sync(&mut queue, &mut client);
    let sizes: Vec<_> = client
        .sizes
        .iter()
        .filter(|(top, _, _)| top == &child.toplevel)
        .collect();
    assert!(!sizes.is_empty());
    assert!(
        sizes.iter().all(|(_, w, h)| *w == 0 && *h == 0),
        "a parented dialog must never inherit an earlier fullscreen configure: {sizes:?}"
    );
    child.map();
    sync(&mut queue, &mut client);
    let before = client.sizes.len();
    child.unmap();
    sync(&mut queue, &mut client);
    child.toplevel.set_parent(Some(&root.toplevel));
    sync(&mut queue, &mut client);
    assert!(
        client.sizes[before..]
            .iter()
            .all(|(top, _, _)| top != &child.toplevel),
        "unmapping is not the next mapping's initial commit: {:?}",
        &client.sizes[before..]
    );
    child.surface.commit();
    sync(&mut queue, &mut client);
    let remapped: Vec<_> = client.sizes[before..]
        .iter()
        .filter(|(top, _, _)| top == &child.toplevel)
        .collect();
    assert!(!remapped.is_empty());
    assert!(
        remapped.iter().all(|(_, w, h)| *w == 0 && *h == 0),
        "{remapped:?}"
    );
    child.map();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"].as_array().unwrap().len(), 1);
    child.destroy();
    root.destroy();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"], json!([]));
    println!("Root/parented initial configure, zero natural sizes and remap handshake verified");
}

#[test]
#[ignore = "requires an empty running Tarot session; see docs/BUILDING.md"]
fn window_state_hints_preserve_initial_commit_and_reply_to_rejections() {
    let initial = status();
    assert_eq!(initial["cards"], json!([]));
    let socket = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap())
        .join(initial["wayland_display"].as_str().unwrap());
    let connection = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut client = Client::default();
    connection.display().get_registry(&qh, ());
    sync(&mut queue, &mut client);
    let root = client.window(&qh, "State hint parent");
    sync(&mut queue, &mut client);
    root.map();
    sync(&mut queue, &mut client);
    let original_view = status()["view"].clone();
    for request in 0..4 {
        let child = client.uncommitted_window(&qh, "State hint dialog", None);
        match request {
            0 => child.toplevel.set_maximized(),
            1 => child.toplevel.unset_maximized(),
            2 => child.toplevel.set_fullscreen(None),
            _ => child.toplevel.unset_fullscreen(),
        }
        sync(&mut queue, &mut client);
        child.toplevel.set_parent(Some(&root.toplevel));
        sync(&mut queue, &mut client);
        assert!(
            client
                .sizes
                .iter()
                .all(|(top, _, _)| top != &child.toplevel),
            "window-state request {request} sent a premature initial configure"
        );
        assert_eq!(status()["view"], original_view);
        child.surface.commit();
        sync(&mut queue, &mut client);
        assert!(
            client
                .sizes
                .iter()
                .any(|(top, w, h)| top == &child.toplevel && *w == 0 && *h == 0)
        );
        assert!(
            client.capabilities[&child.toplevel].is_empty(),
            "transient state controls are not advertised"
        );
        child.map();
        sync(&mut queue, &mut client);
        let mapped_view = status()["view"].clone();
        for request in 0..4 {
            let before = client.sizes.len();
            match request {
                0 => child.toplevel.set_maximized(),
                1 => child.toplevel.unset_maximized(),
                2 => child.toplevel.set_fullscreen(None),
                _ => child.toplevel.unset_fullscreen(),
            }
            sync(&mut queue, &mut client);
            let reply: Vec<_> = client.sizes[before..]
                .iter()
                .filter(|(top, _, _)| top == &child.toplevel)
                .collect();
            assert!(
                !reply.is_empty(),
                "mapped state request {request} needs a configure, including a rejection"
            );
            assert!(
                reply.iter().all(|(_, w, h)| *w == 0 && *h == 0),
                "a transient keeps its natural size: {reply:?}"
            );
            let states = &client
                .states
                .iter()
                .rev()
                .find(|(top, _)| top == &child.toplevel)
                .unwrap()
                .1;
            assert!(
                !states.contains(&(xdg_toplevel::State::Maximized as u32))
                    && !states.contains(&(xdg_toplevel::State::Fullscreen as u32))
            );
            assert_eq!(
                status()["view"],
                mapped_view,
                "a state hint cannot change focus/view"
            );
            assert_eq!(status()["cards"].as_array().unwrap().len(), 1);
        }
        child.destroy();
        sync(&mut queue, &mut client);
    }
    let startup = client.uncommitted_window(&qh, "Initial fullscreen hints", None);
    startup.toplevel.set_fullscreen(None);
    sync(&mut queue, &mut client);
    assert!(
        client
            .sizes
            .iter()
            .all(|(top, _, _)| top != &startup.toplevel)
    );
    assert_eq!(status()["view"], original_view);
    startup.surface.commit();
    sync(&mut queue, &mut client);
    assert!(latest_states(&client, &startup).contains(&(xdg_toplevel::State::Fullscreen as u32)));
    startup.map();
    sync(&mut queue, &mut client);
    let parent_id = status()["cards"][0]["id"].clone();
    control(json!({"command":"activate","id":parent_id}));
    sync(&mut queue, &mut client);
    let view = status()["view"].clone();
    startup.toplevel.unset_fullscreen();
    startup.toplevel.set_maximized();
    sync(&mut queue, &mut client);
    startup.surface.commit();
    sync(&mut queue, &mut client);
    assert_eq!(
        status()["view"],
        view,
        "hidden window state requests cannot activate its card"
    );
    assert_eq!(client.focus.as_ref(), Some(&root.surface));
    startup.destroy();
    sync(&mut queue, &mut client);
    root.destroy();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"], json!([]));
    println!(
        "All four pre-commit hints defer configure; natural transients receive rejection replies without focus changes"
    );
}

impl Window {
    fn map(&self) {
        self.surface.attach(Some(&self.buffer), 0, 0);
        self.surface.damage(0, 0, 160, 240);
        self.surface.commit();
    }

    fn unmap(&self) {
        self.surface.attach(None, 0, 0);
        self.surface.commit();
    }

    fn destroy(self) {
        self.toplevel.destroy();
        self.xdg.destroy();
        self.surface.destroy();
        self.buffer.destroy();
    }
}

fn control(request: Value) -> Value {
    let path = std::env::var_os("TAROT_TEST_CONTROL_SOCKET")
        .expect("set TAROT_TEST_CONTROL_SOCKET to the isolated session's private socket");
    let mut stream = UnixStream::connect(path).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    serde_json::to_writer(&mut stream, &request).unwrap();
    stream.write_all(b"\n").unwrap();
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response).unwrap();
    let response: Value = serde_json::from_str(&response).unwrap();
    assert_eq!(response["ok"], true, "{response}");
    response
}

fn status() -> Value {
    control(json!({"command": "status"}))
}

fn sync(queue: &mut EventQueue<Client>, state: &mut Client) {
    // The second round trip also flushes configure acknowledgements generated by the first.
    queue.roundtrip(state).unwrap();
    queue.roundtrip(state).unwrap();
}

fn await_frame(queue: &mut EventQueue<Client>, state: &mut Client, previous: usize) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while state.frames.len() == previous && Instant::now() < deadline {
        sync(queue, state);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        state.frames.len(),
        previous + 1,
        "a requested native frame callback must eventually arrive"
    );
}

#[test]
#[ignore = "requires an empty running Tarot session; see docs/BUILDING.md"]
fn transient_families_preserve_card_and_native_focus() {
    let initial = status();
    assert_eq!(initial["cards"], json!([]));
    let socket = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap())
        .join(initial["wayland_display"].as_str().unwrap());
    let connection = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut client = Client::default();
    connection.display().get_registry(&qh, ());
    sync(&mut queue, &mut client);
    let parent = client.window(&qh, "Document");
    sync(&mut queue, &mut client);
    parent.map();
    sync(&mut queue, &mut client);
    let id = status()["cards"][0]["id"].clone();
    let dialog = client.window(&qh, "Document dialog");
    dialog.toplevel.set_parent(Some(&parent.toplevel));
    sync(&mut queue, &mut client);
    assert_eq!(client.focus.as_ref(), Some(&parent.surface));
    dialog.map();
    sync(&mut queue, &mut client);
    assert_eq!(
        status()["cards"].as_array().unwrap().len(),
        1,
        "a transient is part of its parent's card"
    );
    assert_eq!(status()["cards"][0]["id"], id);
    assert_eq!(status()["cards"][0]["title"], "Document");
    assert_eq!(client.focus.as_ref(), Some(&dialog.surface));
    let modal = client
        .dialogs
        .as_ref()
        .expect("xdg-dialog global")
        .get_xdg_dialog(&dialog.toplevel, &qh, ());
    modal.set_modal();
    let nested = client.window(&qh, "Nested dialog");
    nested.toplevel.set_parent(Some(&dialog.toplevel));
    sync(&mut queue, &mut client);
    nested.map();
    sync(&mut queue, &mut client);
    let nested_id = status()["native_windows"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["id"]
        .clone();
    assert_eq!(status()["cards"].as_array().unwrap().len(), 1);
    assert_eq!(client.focus.as_ref(), Some(&nested.surface));
    let sibling = client.window(&qh, "Nonmodal sibling");
    sibling.toplevel.set_parent(Some(&parent.toplevel));
    sync(&mut queue, &mut client);
    sibling.map();
    sync(&mut queue, &mut client);
    assert_eq!(
        client.focus.as_ref(),
        Some(&nested.surface),
        "a nonmodal sibling cannot displace a modal branch"
    );
    control(json!({"command":"overview"}));
    sync(&mut queue, &mut client);
    assert!(client.focus.is_none());
    control(json!({"command":"activate","id":id}));
    sync(&mut queue, &mut client);
    assert_eq!(client.focus.as_ref(), Some(&nested.surface));
    control(json!({"command":"close","id":id}));
    sync(&mut queue, &mut client);
    assert_eq!(
        client.close_targets.last(),
        Some(&parent.toplevel),
        "card close asks its root, allowing client confirmation"
    );
    assert_eq!(status()["cards"].as_array().unwrap().len(), 1);
    nested.unmap();
    sync(&mut queue, &mut client);
    assert_eq!(client.focus.as_ref(), Some(&dialog.surface));
    sibling.destroy();
    sync(&mut queue, &mut client);
    dialog.unmap();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"].as_array().unwrap().len(), 1);
    assert_eq!(client.focus.as_ref(), Some(&parent.surface));
    println!(
        "Nested modal focus, overview restoration, root close refusal and child unmap verified"
    );

    // A hidden family's new transient must not steal another app or a shell panel.
    let other = client.window(&qh, "Independent app");
    sync(&mut queue, &mut client);
    other.map();
    sync(&mut queue, &mut client);
    let other_view = status()["view"].clone();
    dialog.surface.commit();
    sync(&mut queue, &mut client);
    dialog.toplevel.set_parent(Some(&parent.toplevel));
    sync(&mut queue, &mut client);
    dialog.map();
    sync(&mut queue, &mut client);
    assert_eq!(status()["view"], other_view);
    assert_eq!(client.focus.as_ref(), Some(&other.surface));
    control(json!({"command":"system_menu","open":true}));
    nested.surface.commit();
    sync(&mut queue, &mut client);
    nested.toplevel.set_parent(Some(&dialog.toplevel));
    sync(&mut queue, &mut client);
    nested.map();
    sync(&mut queue, &mut client);
    assert_eq!(status()["system_menu_open"], true);
    assert!(client.focus.is_none());
    control(json!({"command":"system_menu","open":false}));
    other.destroy();
    sync(&mut queue, &mut client);
    control(json!({"command":"activate","id":id}));
    sync(&mut queue, &mut client);
    assert_eq!(client.focus.as_ref(), Some(&nested.surface));
    println!("Hidden-family mapping and shell-panel focus isolation verified");

    // Parenting is immediate, may be changed after mapping, and is based on
    // protocol identity even when app IDs are identical across independent roots.
    modal.unset_modal();
    let late = client.window(&qh, "Late parent");
    sync(&mut queue, &mut client);
    late.map();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"].as_array().unwrap().len(), 2);
    let late_id = status()["cards"][1]["id"].clone();
    late.toplevel.set_parent(Some(&parent.toplevel));
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"].as_array().unwrap().len(), 1);
    assert_eq!(status()["view"], json!({"App":id}));
    assert_eq!(client.focus.as_ref(), Some(&late.surface));
    late.toplevel.set_parent(None);
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"][1]["id"], late_id);
    late.destroy();
    sync(&mut queue, &mut client);

    // Removing an intermediate parent inherits its parent; remapping does not
    // revive that edge. Removing the root detaches surviving children.
    dialog.unmap();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"].as_array().unwrap().len(), 1);
    let parent_of_nested = || {
        status()["native_windows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == nested_id)
            .unwrap()["parent"]
            .clone()
    };
    assert_eq!(parent_of_nested(), id);
    dialog.surface.commit();
    sync(&mut queue, &mut client);
    dialog.map();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"].as_array().unwrap().len(), 2);
    assert_eq!(parent_of_nested(), id);
    parent.unmap();
    sync(&mut queue, &mut client);
    assert!(parent_of_nested().is_null());
    assert_eq!(status()["cards"].as_array().unwrap().len(), 2);
    nested.toplevel.set_parent(Some(&parent.toplevel)); // currently unmapped => null
    sync(&mut queue, &mut client);
    parent.surface.commit();
    sync(&mut queue, &mut client);
    parent.map();
    sync(&mut queue, &mut client);
    assert!(parent_of_nested().is_null());
    assert_eq!(status()["cards"].as_array().unwrap().len(), 3);
    println!(
        "Late attach/detach, inherited orphan parenting and permanent unmapped-parent reset verified"
    );
    modal.destroy();
    nested.destroy();
    dialog.destroy();
    parent.destroy();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"], json!([]));
}

fn drive(request: Value) {
    let mut stream = UnixStream::connect(
        std::env::var_os("TAROT_TEST_INPUT_SOCKET")
            .expect("run tests/transient_session_replay.py for scoped real input"),
    )
    .unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .unwrap();
    serde_json::to_writer(&mut stream, &request).unwrap();
    stream.write_all(b"\n").unwrap();
    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&response).unwrap()["ok"],
        true,
        "{response}"
    );
}

#[test]
#[ignore = "requires owned display/input and protected session; run transient_session_replay.py --case gtk-modal"]
fn gtk_modal_hints_combine_with_xdg_and_preserve_focus_boundaries() {
    fn observe(label: &str, notes: &mut Vec<Value>, focus: bool) -> Value {
        let s = status();
        notes.push(json!({"label":label,"status":s,"child_focus":focus}));
        if let Some(path) = std::env::var_os("TAROT_TEST_CAPTURE_DIR") {
            std::fs::write(
                PathBuf::from(path).join("gtk-modal-proof.json"),
                serde_json::to_vec_pretty(notes).unwrap(),
            )
            .unwrap();
        }
        s
    }
    fn modal(s: &Value, id: &Value) -> bool {
        s["native_windows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|w| w["id"] == *id)
            .unwrap()["modal"]
            .as_bool()
            .unwrap()
    }
    assert_eq!(status()["cards"], json!([]));
    let initial = status();
    let socket = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap())
        .join(initial["wayland_display"].as_str().unwrap());
    let connection = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut client = Client::default();
    let mut notes = vec![];
    connection.display().get_registry(&qh, ());
    sync(&mut queue, &mut client);
    assert_eq!(
        client.gtk_capabilities,
        Some(0),
        "no global-menu flags before full acceptance"
    );
    let gtk = client.gtk.as_ref().unwrap().clone();
    let root = client.window(&qh, "GTK modal root");
    sync(&mut queue, &mut client);
    root.map();
    sync(&mut queue, &mut client);
    let root_id = status()["cards"][0]["id"].clone();
    let root_gtk = gtk.get_gtk_surface(&root.surface, &qh, ());
    let surface = client.compositor.as_ref().unwrap().create_surface(&qh, ());
    let extension = gtk.get_gtk_surface(&surface, &qh, ());
    extension.set_modal(); // Before xdg_surface and toplevel exist.
    sync(&mut queue, &mut client);
    let child = client.window_on_surface(&qh, surface, "Pre-role GTK modal", Some(&root.toplevel));
    child.surface.commit();
    sync(&mut queue, &mut client);
    child.map();
    sync(&mut queue, &mut client);
    let s = observe(
        "pre-role-gtk-mapped",
        &mut notes,
        client.focus == Some(child.surface.clone()),
    );
    let child_id = s["native_windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["parent"] == root_id)
        .unwrap()["id"]
        .clone();
    assert!(modal(&s, &child_id));
    assert_eq!(s["cards"].as_array().unwrap().len(), 1);
    let sibling = client.uncommitted_window(&qh, "Nonmodal sibling", Some(&root.toplevel));
    sibling.surface.commit();
    sync(&mut queue, &mut client);
    sibling.map();
    sync(&mut queue, &mut client);
    assert_eq!(
        client.focus,
        Some(child.surface.clone()),
        "GTK modal blocks sibling focus"
    );
    let xdg = client
        .dialogs
        .as_ref()
        .unwrap()
        .get_xdg_dialog(&child.toplevel, &qh, ());
    xdg.set_modal();
    extension.unset_modal();
    sync(&mut queue, &mut client);
    assert!(modal(
        &observe(
            "xdg-survives-gtk-unset",
            &mut notes,
            client.focus == Some(child.surface.clone())
        ),
        &child_id
    ));
    extension.set_modal();
    xdg.unset_modal();
    sync(&mut queue, &mut client);
    assert!(modal(
        &observe(
            "gtk-survives-xdg-unset",
            &mut notes,
            client.focus == Some(child.surface.clone())
        ),
        &child_id
    ));
    xdg.set_modal();
    extension.release();
    sync(&mut queue, &mut client);
    assert!(modal(
        &observe(
            "xdg-survives-gtk-release",
            &mut notes,
            client.focus == Some(child.surface.clone())
        ),
        &child_id
    ));
    xdg.unset_modal();
    sync(&mut queue, &mut client);
    assert!(!modal(
        &observe(
            "both-hints-cleared",
            &mut notes,
            client.focus == Some(child.surface.clone())
        ),
        &child_id
    ));
    let replacement = gtk.get_gtk_surface(&child.surface, &qh, ());
    replacement.set_modal();
    sync(&mut queue, &mut client);
    control(json!({"command":"overview"}));
    sync(&mut queue, &mut client);
    replacement.present(0);
    replacement.request_focus(Some("untrusted-startup-token".into()));
    sync(&mut queue, &mut client);
    let s = observe(
        "gtk-focus-hints-under-overview",
        &mut notes,
        client.focus == Some(child.surface.clone()),
    );
    assert_eq!(s["view"], "Overview");
    assert!(client.focus.is_none());
    control(json!({"command":"activate","id":root_id}));
    sync(&mut queue, &mut client);
    assert_eq!(client.focus, Some(child.surface.clone()));
    control(json!({"command":"system_menu","open":true}));
    sync(&mut queue, &mut client);
    root_gtk.present(0);
    root_gtk.request_focus(Some("another-untrusted-token".into()));
    sync(&mut queue, &mut client);
    assert!(client.focus.is_none());
    assert_eq!(
        observe("gtk-focus-hints-under-menu", &mut notes, false)["system_menu_open"],
        true
    );
    control(json!({"command":"system_menu","open":false}));
    sync(&mut queue, &mut client);
    assert_eq!(client.focus, Some(child.surface.clone()));
    drive(json!({"command":"lock"}));
    sync(&mut queue, &mut client);
    replacement.unset_modal();
    xdg.set_modal();
    replacement.present(0);
    replacement.request_focus(Some("locked-token".into()));
    sync(&mut queue, &mut client);
    let s = observe(
        "hints-during-protection",
        &mut notes,
        client.focus == Some(child.surface.clone()),
    );
    assert_eq!(s.as_object().unwrap().len(), 3);
    assert!(client.focus.is_none());
    drive(json!({"command":"unlock"}));
    sync(&mut queue, &mut client);
    let s = observe(
        "protected-return-to-xdg-modal",
        &mut notes,
        client.focus == Some(child.surface.clone()),
    );
    assert!(modal(&s, &child_id));
    assert_eq!(client.focus, Some(child.surface.clone()));
    replacement.release();
    xdg.unset_modal();
    sync(&mut queue, &mut client);
    assert!(!modal(
        &observe(
            "final-release-clears-modal",
            &mut notes,
            client.focus == Some(child.surface.clone())
        ),
        &child_id
    ));
    xdg.destroy();
    sibling.destroy();
    child.destroy();
    root_gtk.release();
    root.destroy();
    sync(&mut queue, &mut client);
    assert_eq!(
        observe("all-surfaces-gone", &mut notes, false)["cards"],
        json!([])
    );
    println!(
        "GTK pre-role hints, GTK/xdg independent ownership/release, focus-hint isolation and protected changes verified"
    );
}

fn output_point(s: &Value, x: f64, y: f64) -> (u32, u32) {
    let g = &s["output"];
    let w = g["physical_width"].as_f64().unwrap();
    let h = g["physical_height"].as_f64().unwrap();
    let scale = g["scale"].as_f64().unwrap();
    let orientation = g["orientation"].as_str().unwrap();
    let (lw, lh) = if matches!(orientation, "left-up" | "right-up") {
        (h / scale, w / scale)
    } else {
        (w / scale, h / scale)
    };
    let (x, y) = (x / lw, y / lh);
    let (x, y) = match orientation {
        "left-up" => (y, 1.0 - x),
        "bottom-up" => (1.0 - x, 1.0 - y),
        "right-up" => (1.0 - y, x),
        _ => (x, y),
    };
    ((x * w).round() as u32, (y * h).round() as u32)
}

fn captured_pixel(label: &str, s: &Value, point: (f64, f64)) -> [u8; 4] {
    let path = PathBuf::from(std::env::var_os("TAROT_TEST_CAPTURE_DIR").unwrap())
        .join(format!("{label}.png"));
    control(json!({"command":"capture","path":path}));
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let s = status();
        if s["capture_pending"] == false {
            assert!(s["capture_error"].is_null(), "{s}");
            break;
        }
        assert!(Instant::now() < deadline, "capture timeout");
        std::thread::sleep(Duration::from_millis(10));
    }
    let mut reader = png::Decoder::new(BufReader::new(std::fs::File::open(path).unwrap()))
        .read_info()
        .unwrap();
    let mut bytes = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut bytes).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let (x, y) = output_point(s, point.0, point.1);
    let offset = ((y * info.width + x) * 4) as usize;
    bytes[offset..offset + 4].try_into().unwrap()
}

#[test]
#[ignore = "requires scoped input driver; run tests/transient_session_replay.py --case viewport"]
fn viewport_crop_scale_commit_and_subsurface_input() {
    assert_eq!(status()["cards"], json!([]));
    control(json!({"command":"rotate","orientation":"normal"}));
    let initial = status();
    let socket = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap())
        .join(initial["wayland_display"].as_str().unwrap());
    let connection = Connection::from_socket(UnixStream::connect(&socket).unwrap()).unwrap();
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut client = Client::default();
    connection.display().get_registry(&qh, ());
    sync(&mut queue, &mut client);
    let manager = client
        .viewporter
        .as_ref()
        .expect("wp_viewporter global")
        .clone();
    let root = client.uncommitted_window(&qh, "Viewport crop and input", None);
    let area = &initial["app_area"];
    let (w, h) = (
        area["width"].as_f64().unwrap() as i32,
        area["height"].as_f64().unwrap() as i32,
    );
    root.xdg.set_window_geometry(0, 0, w, h);
    root.surface.commit();
    sync(&mut queue, &mut client);
    let (bw, bh) = (2 * w + 32, 2 * h + 40);
    let mut pixels = Vec::with_capacity((bw * bh * 4) as usize);
    for y in 0..bh {
        for x in 0..bw {
            pixels.extend_from_slice(
                if (16..16 + 2 * w).contains(&x) && (20..20 + 2 * h).contains(&y) {
                    &[0x48, 0x80, 0x20, 0xff]
                } else {
                    &[0xc0, 0x20, 0xc0, 0xff]
                },
            );
        }
    }
    let root_buffer = client.pixel_buffer(&qh, bw, bh, &pixels);
    let root_view = manager.get_viewport(&root.surface, &qh, ());
    root_view.set_source(16.0, 20.0, (2 * w).into(), (2 * h).into());
    root_view.set_destination(w, h);
    root.surface.attach(Some(&root_buffer), 0, 0);
    root.surface.damage(0, 0, w, h);
    root.surface.commit();
    sync(&mut queue, &mut client);
    std::thread::sleep(Duration::from_millis(450));
    let point = |x: f64, y: f64| (x, y + area["y"].as_f64().unwrap());
    // A preceding case can leave a mouse hovering on the shell title. Clear
    // that real tooltip with a verified root contact before checking client
    // pixels. The original sample positions and colors remain unchanged.
    let presses = client.presses.len();
    let root_local = (f64::from(w) - 4.0, f64::from(h) / 2.0);
    let root_global = point(root_local.0, root_local.1);
    drive(
        json!({"command":"click","x":root_global.0,"y":root_global.1,"output":status()["output"],"key":false}),
    );
    sync(&mut queue, &mut client);
    assert_eq!(client.presses.len(), presses + 1);
    let (surface, x, y) = client.presses.last().unwrap();
    assert_eq!(surface, &root.surface);
    assert!((*x - root_local.0).abs() < 1.5 && (*y - root_local.1).abs() < 1.5);
    assert_eq!(
        captured_pixel("viewport-root-cropped", &status(), point(5.0, 5.0)),
        [32, 128, 72, 255]
    );

    let child = client.compositor.as_ref().unwrap().create_surface(&qh, ());
    let sub = client
        .subcompositor
        .as_ref()
        .unwrap()
        .get_subsurface(&child, &root.surface, &qh, ());
    sub.set_desync();
    sub.set_position(30, 55);
    let child_view = manager.get_viewport(&child, &qh, ());
    child_view.set_source(20.0, 30.0, 100.0, 80.0);
    child_view.set_destination(50, 40);
    let mut pixels = Vec::with_capacity(160 * 160 * 4);
    for y in 0..160 {
        for x in 0..160 {
            pixels.extend_from_slice(if !(20..120).contains(&x) || !(30..110).contains(&y) {
                &[0x18, 0xc0, 0xe0, 0xff]
            } else if x < 70 {
                &[0x38, 0x38, 0xd8, 0xff]
            } else {
                &[0xd8, 0x58, 0x28, 0xff]
            });
        }
    }
    let child_buffer = client.pixel_buffer(&qh, 160, 160, &pixels);
    child.attach(Some(&child_buffer), 0, 0);
    child.damage(0, 0, 160, 160);
    child.commit();
    root.surface.commit();
    sync(&mut queue, &mut client);
    assert_eq!(
        captured_pixel("viewport-child-left", &status(), point(42.5, 75.0)),
        [216, 56, 56, 255]
    );
    assert_eq!(
        captured_pixel("viewport-child-right", &status(), point(67.5, 75.0)),
        [40, 88, 216, 255]
    );
    assert_eq!(
        captured_pixel("viewport-child-outside", &status(), point(28.0, 75.0)),
        [32, 128, 72, 255]
    );
    let click = |queue: &mut EventQueue<Client>,
                 client: &mut Client,
                 global: (f64, f64),
                 local: (f64, f64)| {
        let before = client.presses.len();
        drive(
            json!({"command":"click","x":global.0,"y":global.1,"output":status()["output"],"key":false}),
        );
        sync(queue, client);
        assert_eq!(client.presses.len(), before + 1);
        let (surface, x, y) = client.presses.last().unwrap();
        assert_eq!(surface, &child);
        assert!(
            (*x - local.0).abs() < 1.5 && (*y - local.1).abs() < 1.5,
            "viewport input: {x},{y}, wanted {local:?}"
        );
    };
    click(&mut queue, &mut client, point(42.5, 75.0), (12.5, 20.0));
    // Cropping and destination changes commit without a new buffer.
    child_view.set_source(70.0, 30.0, 50.0, 80.0);
    child.commit();
    sync(&mut queue, &mut client);
    assert_eq!(
        captured_pixel("viewport-source-only", &status(), point(42.5, 75.0)),
        [40, 88, 216, 255]
    );
    click(&mut queue, &mut client, point(42.5, 75.0), (12.5, 20.0));
    child_view.set_destination(100, 80);
    child.commit();
    sync(&mut queue, &mut client);
    assert_eq!(
        captured_pixel("viewport-destination-only", &status(), point(105.0, 115.0)),
        [40, 88, 216, 255]
    );
    click(&mut queue, &mut client, point(105.0, 115.0), (75.0, 60.0));
    // Subsurface position is parent state: no child commit/attachment follows.
    sub.set_position(80, 120);
    root.surface.commit();
    sync(&mut queue, &mut client);
    assert_eq!(
        captured_pixel("viewport-parent-position-old", &status(), point(42.5, 75.0)),
        [32, 128, 72, 255]
    );
    assert_eq!(
        captured_pixel(
            "viewport-parent-position-new",
            &status(),
            point(155.0, 180.0)
        ),
        [40, 88, 216, 255]
    );
    click(&mut queue, &mut client, point(155.0, 180.0), (75.0, 60.0));
    child_view.destroy();
    child.commit();
    sync(&mut queue, &mut client);
    assert_eq!(
        captured_pixel(
            "viewport-destroy-restores-buffer",
            &status(),
            point(85.0, 125.0)
        ),
        [224, 192, 24, 255]
    );
    click(&mut queue, &mut client, point(85.0, 125.0), (5.0, 5.0));
    root_view.destroy();
    root.surface.commit();
    sync(&mut queue, &mut client);
    assert_eq!(
        captured_pixel("viewport-root-destroy", &status(), point(5.0, 5.0)),
        [192, 32, 192, 255]
    );

    // Malformed clients must be disconnected without disturbing the valid one.
    for duplicate in [true, false] {
        let bad = Connection::from_socket(UnixStream::connect(&socket).unwrap()).unwrap();
        let mut bad_queue = bad.new_event_queue();
        let bad_qh = bad_queue.handle();
        let mut bad_state = Client::default();
        bad.display().get_registry(&bad_qh, ());
        sync(&mut bad_queue, &mut bad_state);
        let surface = bad_state
            .compositor
            .as_ref()
            .unwrap()
            .create_surface(&bad_qh, ());
        let view = bad_state
            .viewporter
            .as_ref()
            .unwrap()
            .get_viewport(&surface, &bad_qh, ());
        if duplicate {
            let _duplicate =
                bad_state
                    .viewporter
                    .as_ref()
                    .unwrap()
                    .get_viewport(&surface, &bad_qh, ());
        } else {
            let buffer = bad_state.buffer(&bad_qh, 16, 16, [0, 0, 0, 255]);
            view.set_source(0.0, 0.0, 100.0, 100.0);
            view.set_destination(16, 16);
            surface.attach(Some(&buffer), 0, 0);
            surface.commit();
        }
        assert!(
            bad_queue.roundtrip(&mut bad_state).is_err(),
            "invalid viewport must disconnect its client: duplicate={duplicate}"
        );
    }
    assert_eq!(status()["cards"].as_array().unwrap().len(), 1);
    sync(&mut queue, &mut client);
    drive(json!({"command":"lock"}));
    let protected_view = manager.get_viewport(&root.surface, &qh, ());
    protected_view.set_source(16.0, 20.0, (2 * w).into(), (2 * h).into());
    protected_view.set_destination(w, h);
    root.surface.commit();
    sync(&mut queue, &mut client);
    assert_eq!(
        status().as_object().unwrap().len(),
        3,
        "viewport commits must retain protected status redaction"
    );
    drive(json!({"command":"unlock"}));
    assert_eq!(
        captured_pixel("viewport-after-protection", &status(), point(5.0, 5.0)),
        [32, 128, 72, 255]
    );
    protected_view.destroy();
    sub.destroy();
    child.destroy();
    child_buffer.destroy();
    root.destroy();
    root_buffer.destroy();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"], json!([]));
    println!(
        "Viewport root/child source crop, destination scale, state-only commits, parent-only positioning, local input, destruction and malformed-client isolation verified"
    );
}

#[test]
#[ignore = "requires scoped input driver; run tests/transient_session_replay.py"]
fn transient_native_input_and_pixels() {
    let initial = status();
    assert_eq!(initial["cards"], json!([]));
    let socket = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap())
        .join(initial["wayland_display"].as_str().unwrap());
    let connection = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut client = Client::default();
    connection.display().get_registry(&qh, ());
    sync(&mut queue, &mut client);
    let parent = client.window(&qh, "Pixel parent");
    sync(&mut queue, &mut client);
    parent.map();
    sync(&mut queue, &mut client);
    let id = status()["cards"][0]["id"].clone();
    let child = client.window(&qh, "Pixel dialog");
    child.toplevel.set_parent(Some(&parent.toplevel));
    child.xdg.set_window_geometry(20, 30, 80, 120);
    let red = client.buffer(&qh, 100, 160, [0x38, 0x38, 0xd8, 0xff]);
    let modal = client
        .dialogs
        .as_ref()
        .unwrap()
        .get_xdg_dialog(&child.toplevel, &qh, ());
    modal.set_modal();
    sync(&mut queue, &mut client);
    child.surface.attach(Some(&red), 0, 0);
    child.surface.damage(0, 0, 100, 160);
    child.surface.commit();
    sync(&mut queue, &mut client);
    for orientation in ["normal", "left-up", "bottom-up", "right-up"] {
        control(json!({"command":"rotate","orientation":orientation}));
        std::thread::sleep(Duration::from_millis(420));
        let s = status();
        let area = &s["app_area"];
        let cx = area["x"].as_f64().unwrap() + area["width"].as_f64().unwrap() / 2.0;
        let cy = area["y"].as_f64().unwrap() + area["height"].as_f64().unwrap() / 2.0;
        // This fixture deliberately ignores fullscreen configure sizes. Its
        // real 160x240 parent is fitted and may be letterboxed in portrait.
        let parent_fit =
            (area["width"].as_f64().unwrap() / 160.0).min(area["height"].as_f64().unwrap() / 240.0);
        let top = cy - 120.0 * parent_fit + 20.0;
        assert_eq!(
            captured_pixel(&format!("{orientation}-app"), &s, (cx, cy)),
            [216, 56, 56, 255]
        );
        assert_eq!(
            captured_pixel(&format!("{orientation}-parent"), &s, (cx, top)),
            [32, 128, 72, 255]
        );
        drive(json!({"command":"click","x":cx,"y":cy,"output":s["output"],"key":true}));
        sync(&mut queue, &mut client);
        let (surface, x, y) = client.presses.last().unwrap();
        assert_eq!(surface, &child.surface);
        assert!(
            (*x - 60.0).abs() < 1.5 && (*y - 90.0).abs() < 1.5,
            "dialog input at nonzero geometry: {x},{y}"
        );
        assert_eq!(client.keys.last(), Some(&child.surface));
        let before = client.presses.len();
        drive(json!({"command":"click","x":cx,"y":top,"output":s["output"],"key":false}));
        sync(&mut queue, &mut client);
        assert_eq!(
            client.presses.len(),
            before,
            "modal dialog blocks the parent"
        );
        assert_eq!(client.focus.as_ref(), Some(&child.surface));
        modal.unset_modal();
        sync(&mut queue, &mut client);
        drive(json!({"command":"click","x":cx,"y":top,"output":s["output"],"key":true}));
        sync(&mut queue, &mut client);
        assert_eq!(&client.presses.last().unwrap().0, &parent.surface);
        assert_eq!(client.keys.last(), Some(&parent.surface));
        assert_eq!(client.focus.as_ref(), Some(&parent.surface));
        drive(json!({"command":"click","x":cx,"y":cy,"output":s["output"],"key":true}));
        sync(&mut queue, &mut client);
        assert_eq!(client.focus.as_ref(), Some(&child.surface));
        let popup = client.popup(&qh, &child, true);
        sync(&mut queue, &mut client);
        let (px, py, pw, ph) = client.popup_rects[&popup.popup];
        assert_eq!((pw, ph), (140, 100));
        popup.map();
        sync(&mut queue, &mut client);
        std::thread::sleep(Duration::from_millis(80));
        assert!(status()["popup_grab_active"].as_bool().unwrap());
        assert_eq!(status()["popup_owner"], id);
        let (mx, my) = [(15.0, 15.0), (125.0, 15.0), (15.0, 85.0), (125.0, 85.0)]
            .into_iter()
            .find(|(x, y)| {
                let x = f64::from(px) + x;
                let y = f64::from(py) + y;
                !(-20.0..=80.0).contains(&x) || !(-30.0..=130.0).contains(&y)
            })
            .expect("the popup extends outside its parent's actual buffer");
        let point = (
            cx - 40.0 + f64::from(px) + mx,
            cy - 60.0 + f64::from(py) + my,
        );
        assert_eq!(
            captured_pixel(&format!("{orientation}-popup-overflow"), &s, point),
            [34, 85, 221, 255],
            "a popup must render outside its dialog target"
        );
        drive(json!({"command":"click","x":point.0,"y":point.1,"output":s["output"],"key":true}));
        sync(&mut queue, &mut client);
        let (target, x, y) = client.presses.last().unwrap();
        assert_eq!(target, &popup.surface);
        assert!(
            (*x - mx).abs() < 1.5 && (*y - my).abs() < 1.5,
            "popup coordinates {x},{y} expected {mx},{my}"
        );
        assert_eq!(client.keys.last(), Some(&popup.surface));
        popup.destroy();
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            sync(&mut queue, &mut client);
            if client.focus.as_ref() == Some(&child.surface)
                && status()["popup_grab_active"] == false
            {
                break;
            }
            assert!(Instant::now() < until, "popup focus not restored");
            std::thread::sleep(Duration::from_millis(10));
        }
        control(json!({"command":"overview"}));
        std::thread::sleep(Duration::from_millis(420));
        let overview = status();
        let r = &overview["presentation"]["windows"][0]["rect"];
        let center = (
            r["x"].as_f64().unwrap() + r["width"].as_f64().unwrap() / 2.0,
            r["y"].as_f64().unwrap() + r["height"].as_f64().unwrap() / 2.0,
        );
        assert_eq!(
            captured_pixel(&format!("{orientation}-overview"), &overview, center),
            [216, 56, 56, 255]
        );
        assert_eq!(overview["cards"].as_array().unwrap().len(), 1);
        control(json!({"command":"activate","id":id}));
        sync(&mut queue, &mut client);
        assert_eq!(client.focus.as_ref(), Some(&child.surface));
        // A non-grabbing popup deliberately survives overview in this fixture
        // so actual native pixels can check the rounded outer family boundary.
        let preview = client.popup(&qh, &child, false);
        sync(&mut queue, &mut client);
        preview.map();
        sync(&mut queue, &mut client);
        control(json!({"command":"overview"}));
        std::thread::sleep(Duration::from_millis(420));
        let overview = status();
        let r = &overview["presentation"]["windows"][0]["rect"];
        let right = r["x"].as_f64().unwrap() + r["width"].as_f64().unwrap();
        let bottom = r["y"].as_f64().unwrap() + r["height"].as_f64().unwrap();
        assert_eq!(
            captured_pixel(
                &format!("{orientation}-popup-preview-inside"),
                &overview,
                (right - 20.0, bottom - 20.0)
            ),
            [34, 85, 221, 255]
        );
        assert_ne!(
            captured_pixel(
                &format!("{orientation}-popup-preview-corner"),
                &overview,
                (right - 0.5, bottom - 0.5)
            ),
            [34, 85, 221, 255]
        );
        assert_ne!(
            captured_pixel(
                &format!("{orientation}-popup-preview-outside"),
                &overview,
                (right + 3.0, bottom - 20.0)
            ),
            [34, 85, 221, 255]
        );
        preview.destroy();
        sync(&mut queue, &mut client);
        control(json!({"command":"activate","id":id}));
        sync(&mut queue, &mut client);
        modal.set_modal();
        sync(&mut queue, &mut client);
        println!(
            "{orientation}: native dialog/popup overflow and outer mask pixels, nonzero-origin input, modal/nonmodal focus and typing passed"
        );
    }
    drive(json!({"command":"lock"}));
    let during = client.window(&qh, "Dialog arriving while protected");
    during.toplevel.set_parent(Some(&child.toplevel));
    sync(&mut queue, &mut client);
    during.map();
    sync(&mut queue, &mut client);
    assert!(
        client.focus.is_none(),
        "a dialog mapping under protection cannot take keyboard focus"
    );
    let protected = status();
    assert_eq!(protected.as_object().unwrap().len(), 3);
    assert!(protected.get("native_windows").is_none() && protected.get("cards").is_none());
    drive(json!({"command":"unlock"}));
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"].as_array().unwrap().len(), 1);
    assert_eq!(client.focus.as_ref(), Some(&during.surface));
    during.destroy();
    sync(&mut queue, &mut client);
    assert_eq!(client.focus.as_ref(), Some(&child.surface));
    println!(
        "Protected transient mapping preserves focus and redaction; unlock restores its family"
    );
    modal.destroy();
    child.destroy();
    red.destroy();
    parent.destroy();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"], json!([]));
    fullscreen_input_and_pixels(&mut queue, &mut client, &qh);
}

fn paint_fullscreen(
    window: &mut Window,
    client: &Client,
    qh: &QueueHandle<Client>,
    width: i32,
    height: i32,
) {
    let buffer = client.buffer(qh, width, height, [0x60, 0x40, 0x20, 0x80]);
    window.buffer.destroy();
    window.buffer = buffer;
    window.xdg.set_window_geometry(0, 0, width, height);
    window.surface.attach(Some(&window.buffer), 0, 0);
    window.surface.damage(0, 0, width, height);
    window.surface.commit();
}

fn latest_states<'a>(client: &'a Client, window: &Window) -> &'a [u32] {
    &client
        .states
        .iter()
        .rev()
        .find(|(top, _)| top == &window.toplevel)
        .unwrap()
        .1
}

fn fullscreen_input_and_pixels(
    queue: &mut EventQueue<Client>,
    client: &mut Client,
    qh: &QueueHandle<Client>,
) {
    control(json!({"command":"rotate","orientation":"normal"}));
    let mut root = client.uncommitted_window(qh, "Fullscreen alpha fixture", None);
    root.toplevel.set_maximized();
    sync(queue, client);
    assert!(client.sizes.iter().all(|(top, _, _)| top != &root.toplevel));
    root.surface.commit();
    sync(queue, client);
    assert!(latest_states(client, &root).contains(&(xdg_toplevel::State::Maximized as u32)));
    let caps = &client.capabilities[&root.toplevel];
    assert!(
        caps.contains(&(xdg_toplevel::WmCapabilities::Fullscreen as u32))
            && !caps.contains(&(xdg_toplevel::WmCapabilities::WindowMenu as u32))
    );
    let (_, width, height) = client
        .sizes
        .iter()
        .rev()
        .find(|(top, _, _)| top == &root.toplevel)
        .unwrap();
    let (width, height) = (*width, *height);
    paint_fullscreen(&mut root, client, qh, width, height);
    sync(queue, client);
    let id = status()["cards"][0]["id"].clone();
    std::thread::sleep(Duration::from_millis(450));
    root.toplevel.set_fullscreen(None);
    sync(queue, client); // The fixture acknowledges, but has not committed.
    assert!(latest_states(client, &root).contains(&(xdg_toplevel::State::Fullscreen as u32)));
    let s = status();
    assert_eq!(
        s["presentation"]["windows"][0]["rect"], s["app_area"],
        "acknowledging fullscreen without a surface commit cannot resize presentation"
    );
    assert_ne!(
        captured_pixel("fullscreen-before-commit", &s, (2.0, 2.0)),
        [32, 64, 96, 255]
    );
    for orientation in ["normal", "left-up", "bottom-up", "right-up"] {
        control(json!({"command":"rotate","orientation":orientation}));
        sync(queue, client);
        let (_, width, height) = client
            .sizes
            .iter()
            .rev()
            .find(|(top, _, _)| top == &root.toplevel)
            .unwrap();
        let (width, height) = (*width, *height);
        let s = status();
        let output = &s["output"];
        let landscape = matches!(orientation, "left-up" | "right-up");
        let scale = output["scale"].as_i64().unwrap();
        let expected_width = output[if landscape {
            "physical_height"
        } else {
            "physical_width"
        }]
        .as_i64()
        .unwrap()
            / scale;
        let expected_height = output[if landscape {
            "physical_width"
        } else {
            "physical_height"
        }]
        .as_i64()
        .unwrap()
            / scale;
        assert_eq!(
            (width as i64, height as i64),
            (expected_width, expected_height)
        );
        paint_fullscreen(&mut root, client, qh, width, height);
        sync(queue, client);
        std::thread::sleep(Duration::from_millis(420));
        let s = status();
        for (label, point) in [
            ("top", (f64::from(width) / 2.0, 12.0)),
            ("bottom", (f64::from(width) / 2.0, f64::from(height) - 48.0)),
        ] {
            assert_eq!(
                captured_pixel(&format!("fullscreen-{orientation}-{label}"), &s, point),
                [32, 64, 96, 255],
                "fullscreen alpha must blend over black, with no wallpaper or status bar"
            );
            let before = client.presses.len();
            drive(
                json!({"command":"click","x":point.0,"y":point.1,"output":s["output"],"key":true}),
            );
            sync(queue, client);
            assert_eq!(client.presses.len(), before + 1);
            let (surface, x, y) = client.presses.last().unwrap();
            assert_eq!(surface, &root.surface);
            assert!(
                (*x - point.0).abs() < 1.5 && (*y - point.1).abs() < 1.5,
                "fullscreen input {point:?} arrived at {x},{y}"
            );
        }
        drive(
            json!({"command":"click","x":f64::from(width)/2.0,"y":f64::from(height)-18.0,"output":s["output"],"key":false}),
        );
        std::thread::sleep(Duration::from_millis(450));
        let s = status();
        assert_eq!(s["view"], "Overview");
        let rect = &s["presentation"]["windows"][0]["rect"];
        let center = (
            rect["x"].as_f64().unwrap() + rect["width"].as_f64().unwrap() / 2.0,
            rect["y"].as_f64().unwrap() + rect["height"].as_f64().unwrap() / 2.0,
        );
        assert_eq!(
            captured_pixel(&format!("fullscreen-{orientation}-overview"), &s, center),
            [32, 64, 96, 255]
        );
        assert_ne!(
            captured_pixel(
                &format!("fullscreen-{orientation}-corner"),
                &s,
                (
                    rect["x"].as_f64().unwrap() + 0.5,
                    rect["y"].as_f64().unwrap() + 0.5
                )
            ),
            [0, 0, 0, 255],
            "the black fullscreen backing must share the rounded card mask"
        );
        drive(json!({"command":"click","x":center.0,"y":center.1,"output":s["output"],"key":true}));
        sync(queue, client);
        std::thread::sleep(Duration::from_millis(450));
        assert_eq!(status()["view"], json!({"App":id}));
        println!(
            "{orientation}: full-output alpha pixels, top/bottom input and actual gesture-strip/card navigation passed"
        );
    }
    drive(json!({"command":"lock"}));
    sync(queue, client);
    assert!(client.focus.is_none());
    assert_eq!(status().as_object().unwrap().len(), 3);
    drive(json!({"command":"unlock"}));
    sync(queue, client);
    assert_eq!(client.focus.as_ref(), Some(&root.surface));
    assert_eq!(status()["view"], json!({"App":id}));
    let (_, width, height) = client
        .sizes
        .iter()
        .rev()
        .find(|(top, _, _)| top == &root.toplevel)
        .unwrap();
    let (width, height) = (*width, *height);
    paint_fullscreen(&mut root, client, qh, width, height);
    sync(queue, client);
    let restored = status();
    assert_eq!(
        captured_pixel(
            "fullscreen-restored",
            &restored,
            (f64::from(width) / 2.0, 12.0)
        ),
        [32, 64, 96, 255]
    );
    // Maximization requested during fullscreen changes the state restored later.
    for maximized in [false, true] {
        if maximized {
            root.toplevel.set_maximized();
        } else {
            root.toplevel.unset_maximized();
        }
        let before = client.sizes.len();
        sync(queue, client);
        assert!(client.sizes.len() > before);
        assert!(latest_states(client, &root).contains(&(xdg_toplevel::State::Fullscreen as u32)));
    }
    root.toplevel.unset_fullscreen();
    sync(queue, client);
    assert!(latest_states(client, &root).contains(&(xdg_toplevel::State::Maximized as u32)));
    assert!(!latest_states(client, &root).contains(&(xdg_toplevel::State::Fullscreen as u32)));
    assert_eq!(
        status()["presentation"]["windows"][0]["rect"]["y"],
        0.0,
        "unfullscreen presentation also waits for the client's surface commit"
    );
    let (_, width, height) = client
        .sizes
        .iter()
        .rev()
        .find(|(top, _, _)| top == &root.toplevel)
        .unwrap();
    let (width, height) = (*width, *height);
    paint_fullscreen(&mut root, client, qh, width, height);
    sync(queue, client);
    std::thread::sleep(Duration::from_millis(450));
    let s = status();
    assert_ne!(
        captured_pixel("fullscreen-exited", &s, (2.0, 2.0)),
        [32, 64, 96, 255]
    );
    root.toplevel.set_minimized();
    sync(queue, client);
    std::thread::sleep(Duration::from_millis(450));
    assert_eq!(status()["view"], "Overview");
    root.unmap();
    sync(queue, client);
    root.surface.commit();
    sync(queue, client);
    assert!(
        !latest_states(client, &root).contains(&(xdg_toplevel::State::Maximized as u32))
            && !latest_states(client, &root).contains(&(xdg_toplevel::State::Fullscreen as u32)),
        "unmapping discards requested state"
    );
    root.destroy();
    sync(queue, client);
    assert_eq!(status()["cards"], json!([]));
    println!(
        "Fullscreen protection/focus, restored maximization, cooperative minimization and unmap state reset passed"
    );
}

#[test]
#[ignore = "requires an empty running Tarot session; see docs/BUILDING.md"]
fn first_buffer_unmap_remap_close_and_disconnect() {
    let initial = status();
    assert_eq!(
        initial["cards"],
        json!([]),
        "use an empty development session"
    );
    // Failed development captures must report an error without ending the session.
    control(json!({"command": "capture", "path": "/proc/tarot-invalid-capture.png"}));
    let await_capture = || {
        for _ in 0..100 {
            let result = status();
            if result["capture_pending"] == false {
                return result;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("capture did not finish");
    };
    assert!(await_capture()["capture_error"].is_string());
    let temporary = tempfile::tempdir().unwrap();
    let snapshot = temporary.path().join("recovered-capture.png");
    control(json!({"command": "capture", "path": snapshot}));
    assert!(await_capture()["capture_error"].is_null());
    assert!(
        std::fs::read(&snapshot)
            .unwrap()
            .starts_with(b"\x89PNG\r\n\x1a\n")
    );
    // Connect to the display named by this private control socket, never the parent desktop.
    let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap());
    let socket = runtime.join(initial["wayland_display"].as_str().unwrap());
    let connection = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut client = Client::default();
    connection.display().get_registry(&qh, ());
    sync(&mut queue, &mut client);

    let first = client.window(&qh, "First lifecycle window");
    sync(&mut queue, &mut client);
    assert!(
        client.configures > 0,
        "client received its initial configure"
    );
    assert_eq!(
        status()["cards"],
        json!([]),
        "no card before its first buffer"
    );

    // An app finishing startup must not dismiss a status panel opened while
    // its first buffer was still pending.
    control(json!({"command":"system_menu","open":true}));
    first.map();
    sync(&mut queue, &mut client);
    let mapped = status();
    assert_eq!(mapped["cards"].as_array().unwrap().len(), 1);
    assert_eq!(
        mapped["system_menu_open"], true,
        "first map must preserve the modal"
    );
    assert_eq!(mapped["view"], json!("Home"));
    let first_id = mapped["cards"][0]["id"].clone();
    control(json!({"command":"system_menu","open":false}));
    let focused_view = control(json!({"command":"activate","id":first_id}))["view"].clone();

    // Keep a cached small SHM texture, then accumulate larger damage while
    // hidden before returning to the same size. Import must clip that history
    // to the current buffer, upload the new pixels and leave GL error-free.
    std::thread::sleep(Duration::from_millis(350));
    control(json!({"command":"capture","path":snapshot}));
    assert!(await_capture()["capture_error"].is_null());
    control(json!({"command":"home"}));
    std::thread::sleep(Duration::from_millis(350));
    let tall = client.buffer(&qh, 160, 480, [0x10, 0x20, 0x30, 0xff]);
    first.xdg.set_window_geometry(0, 0, 160, 480);
    first.surface.attach(Some(&tall), 0, 0);
    first.surface.damage(0, 0, 160, 480);
    first.surface.commit();
    sync(&mut queue, &mut client);
    let small = client.buffer(&qh, 160, 240, [0xa0, 0x30, 0x40, 0xff]);
    first.xdg.set_window_geometry(0, 0, 160, 240);
    first.surface.attach(Some(&small), 0, 0);
    first.surface.damage(0, 0, 160, 240);
    first.surface.commit();
    sync(&mut queue, &mut client);
    control(json!({"command":"activate","id":first_id}));
    std::thread::sleep(Duration::from_millis(350));
    control(json!({"command":"capture","path":snapshot}));
    let resized = await_capture();
    assert!(
        resized["capture_error"].is_null(),
        "hidden resize upload/capture failed: {resized}"
    );
    let mut reader = png::Decoder::new(BufReader::new(std::fs::File::open(&snapshot).unwrap()))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Rgba);
    let offset = (((info.height / 2) * info.width + info.width / 2) * 4) as usize;
    assert_eq!(
        &pixels[offset..offset + 4],
        &[0x40, 0x30, 0xa0, 0xff],
        "the resized buffer must replace old texture pixels"
    );
    println!("Hidden SHM grow/shrink upload and new center pixels verified");
    tall.destroy();
    small.destroy();

    let second = client.window(&qh, "Second lifecycle window");
    sync(&mut queue, &mut client);
    let pending = status();
    assert_eq!(pending["cards"].as_array().unwrap().len(), 1);
    assert_eq!(
        pending["view"], focused_view,
        "an unbuffered window cannot take focus"
    );

    second.map();
    sync(&mut queue, &mut client);
    let two_windows = status();
    assert_eq!(two_windows["cards"].as_array().unwrap().len(), 2);
    let second_id = two_windows["cards"][1]["id"].clone();
    assert_ne!(first_id, second_id);

    // A hidden sibling may drive the visible window on the same EGL thread.
    // Withholding every callback would freeze that entire native application.
    std::thread::sleep(Duration::from_millis(350));
    let sibling_start = client.frames.len();
    for _ in 0..3 {
        let before = client.frames.len();
        first.surface.frame(&qh, ());
        first.surface.commit();
        await_frame(&mut queue, &mut client, before);
    }
    let sibling_times = &client.frames[sibling_start..];
    assert!(
        sibling_times[2].wrapping_sub(sibling_times[0]) < 300,
        "a visible client's hidden sibling must not use the 250 ms background pace: {sibling_times:?}"
    );
    println!("Foreground client's hidden sibling callback times (ms): {sibling_times:?}");

    // Fully hidden clients still progress, but cannot request an unlimited loop.
    control(json!({"command": "home"}));
    std::thread::sleep(Duration::from_millis(350));
    let start = client.frames.len();
    for _ in 0..4 {
        let before = client.frames.len();
        first.surface.frame(&qh, ());
        first.surface.commit();
        await_frame(&mut queue, &mut client, before);
    }
    let times = &client.frames[start..];
    assert!(
        times
            .windows(2)
            .all(|pair| pair[1].wrapping_sub(pair[0]) >= 249),
        "hidden callbacks must be paced: {times:?}"
    );
    println!("Hidden native callback times (ms): {times:?}");
    control(json!({"command": "activate", "id": second_id}));

    control(json!({"command": "close", "id": second_id}));
    sync(&mut queue, &mut client);
    assert_eq!(client.close_requests, 1);
    assert_eq!(
        status()["cards"].as_array().unwrap().len(),
        2,
        "a close request waits for the client to unmap or destroy"
    );

    second.unmap();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"].as_array().unwrap().len(), 1);

    // xdg-shell remapping starts with a fresh bufferless configure/ack sequence.
    second.surface.commit();
    sync(&mut queue, &mut client);
    second.map();
    sync(&mut queue, &mut client);
    let remapped = status();
    assert_eq!(remapped["cards"].as_array().unwrap().len(), 2);
    assert_eq!(
        remapped["cards"][1]["id"], second_id,
        "remap keeps window identity"
    );

    second.destroy();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"].as_array().unwrap().len(), 1);

    // An abrupt disconnect must remove the remaining mapped window too.
    drop(first);
    drop(client);
    drop(queue);
    drop(connection);
    // The registry queue handle also owns a connection reference.
    drop(qh);
    for _ in 0..30 {
        if status()["cards"] == json!([]) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("client disconnect left a stale card: {}", status());
}

impl Dispatch<wl_registry::WlRegistry, ()> for Client {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => state.compositor = Some(registry.bind(name, 1, qh, ())),
                "wl_shm" => state.shm = Some(registry.bind(name, 1, qh, ())),
                "xdg_wm_base" => state.shell = Some(registry.bind(name, 6, qh, ())),
                "xdg_wm_dialog_v1" => state.dialogs = Some(registry.bind(name, 1, qh, ())),
                "gtk_shell1" => state.gtk = Some(registry.bind(name, 4, qh, ())),
                "wp_viewporter" => state.viewporter = Some(registry.bind(name, 1, qh, ())),
                "zwp_text_input_manager_v3" => {
                    state.text_manager = Some(registry.bind(name, 1, qh, ()))
                }
                "wl_subcompositor" => state.subcompositor = Some(registry.bind(name, 1, qh, ())),
                "wl_seat" => state.seat = Some(registry.bind(name, 1, qh, ())),
                _ => {}
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for Client {
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

impl Dispatch<xdg_surface::XdgSurface, ()> for Client {
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
            state.configures += 1;
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for Client {
    fn event(
        state: &mut Self,
        toplevel: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Close => {
                state.close_requests += 1;
                state.close_targets.push(toplevel.clone());
            }
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                state.sizes.push((toplevel.clone(), width, height));
                state.states.push((
                    toplevel.clone(),
                    states
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|b| u32::from_ne_bytes(*b))
                        .collect(),
                ));
            }
            xdg_toplevel::Event::WmCapabilities { capabilities } => {
                state.capabilities.insert(
                    toplevel.clone(),
                    capabilities
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|b| u32::from_ne_bytes(*b))
                        .collect(),
                );
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for Client {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { callback_data } = event {
            state.frames.push(callback_data);
        }
    }
}

delegate_noop!(Client: ignore wl_compositor::WlCompositor);
delegate_noop!(Client: ignore wl_shm::WlShm);
delegate_noop!(Client: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Client: ignore wl_surface::WlSurface);
delegate_noop!(Client: ignore wl_buffer::WlBuffer);
delegate_noop!(Client: ignore wl_subcompositor::WlSubcompositor);
delegate_noop!(Client: ignore wl_subsurface::WlSubsurface);
delegate_noop!(Client: ignore wp_viewporter::WpViewporter);
delegate_noop!(Client: ignore wp_viewport::WpViewport);
delegate_noop!(Client: ignore xdg_wm_dialog_v1::XdgWmDialogV1);
delegate_noop!(Client: ignore xdg_dialog_v1::XdgDialogV1);
delegate_noop!(Client: ignore xdg_positioner::XdgPositioner);
delegate_noop!(Client: ignore gtk_protocol::gtk_surface1::GtkSurface1);

impl Dispatch<gtk_protocol::gtk_shell1::GtkShell1, ()> for Client {
    fn event(
        state: &mut Self,
        _: &gtk_protocol::gtk_shell1::GtkShell1,
        event: gtk_protocol::gtk_shell1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let gtk_protocol::gtk_shell1::Event::Capabilities { capabilities } = event;
        state.gtk_capabilities = Some(capabilities);
    }
}

impl Dispatch<xdg_popup::XdgPopup, ()> for Client {
    fn event(
        state: &mut Self,
        popup: &xdg_popup::XdgPopup,
        event: xdg_popup::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_popup::Event::Configure {
            x,
            y,
            width,
            height,
        } = event
        {
            state
                .popup_rects
                .insert(popup.clone(), (x, y, width, height));
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Client {
    fn event(
        _: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event
            && let Ok(caps) = capabilities.into_result()
        {
            if caps.contains(wl_seat::Capability::Keyboard) {
                seat.get_keyboard(qh, ());
            }
            if caps.contains(wl_seat::Capability::Pointer) {
                seat.get_pointer(qh, ());
            }
            if caps.contains(wl_seat::Capability::Touch) {
                seat.get_touch(qh, ());
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Client {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { surface, .. } => state.focus = Some(surface),
            wl_keyboard::Event::Leave { .. } => state.focus = None,
            wl_keyboard::Event::Key { state: pressed, .. }
                if matches!(pressed.into_result(), Ok(wl_keyboard::KeyState::Pressed)) =>
            {
                if let Some(surface) = state.focus.as_ref() {
                    state.keys.push(surface.clone());
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for Client {
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
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                state.pointer_focus = Some(surface);
                state.pointer_position = (surface_x, surface_y);
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => state.pointer_position = (surface_x, surface_y),
            wl_pointer::Event::Leave { .. } => state.pointer_focus = None,
            wl_pointer::Event::Button {
                state: pressed,
                serial,
                ..
            } if matches!(pressed.into_result(), Ok(wl_pointer::ButtonState::Pressed)) => {
                state.press_serial = Some(serial);
                if let Some(surface) = state.pointer_focus.as_ref() {
                    state.presses.push((
                        surface.clone(),
                        state.pointer_position.0,
                        state.pointer_position.1,
                    ));
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_touch::WlTouch, ()> for Client {
    fn event(
        state: &mut Self,
        _: &wl_touch::WlTouch,
        event: wl_touch::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_touch::Event::Down {
            surface,
            x,
            y,
            serial,
            ..
        } = event
        {
            state.press_serial = Some(serial);
            state.presses.push((surface, x, y));
        }
    }
}

#[test]
#[ignore = "requires owned bus/display/input; run tests/transient_session_replay.py --case notifications"]
fn notification_well_keeps_oversized_client_input_and_popup_alignment() {
    fn wait(
        queue: &mut EventQueue<Client>,
        client: &mut Client,
        label: &str,
        condition: impl Fn(&Client, &Value) -> bool,
    ) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            sync(queue, client);
            let s = status();
            if condition(client, &s) {
                return s;
            }
            assert!(
                Instant::now() < deadline,
                "notification fixture {label}: {s}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn notify(bus: &zbus::blocking::Connection, replaces: u32) -> u32 {
        let proxy = zbus::blocking::Proxy::new(
            bus,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
        .unwrap();
        let hints: HashMap<&str, zbus::zvariant::Value<'_>> = HashMap::new();
        proxy
            .call(
                "Notify",
                &(
                    "Tarot Fixture",
                    replaces,
                    "",
                    "Synthetic notification",
                    "Known fixture body",
                    vec!["default", "Open"],
                    hints,
                    0i32,
                ),
            )
            .unwrap()
    }
    fn visual(s: &Value) -> &Value {
        &s["presentation"]["windows"][0]["rect"]
    }
    fn number(v: &Value, key: &str) -> f64 {
        v[key].as_f64().unwrap()
    }
    fn center(s: &Value) -> (f64, f64) {
        let v = visual(s);
        (
            number(v, "x") + number(v, "width") * 0.5,
            number(v, "y") + number(v, "height") * 0.5,
        )
    }
    fn click_root(
        queue: &mut EventQueue<Client>,
        client: &mut Client,
        root: &Window,
        s: &Value,
        label: &str,
    ) {
        let point = center(s);
        assert_eq!(captured_pixel(label, s, point), [32, 128, 72, 255]);
        let before = client.presses.len();
        let keys = client.keys.len();
        drive(json!({"command":"click","output":s["output"],"x":point.0,"y":point.1,"key":true}));
        sync(queue, client);
        assert_eq!(
            client.presses.len(),
            before + 1,
            "{label}: exactly one native press"
        );
        assert_eq!(client.presses.last().unwrap().0, root.surface);
        assert_eq!(
            client.keys.len(),
            keys + 1,
            "{label}: physical typing retained"
        );
        assert_eq!(client.keys.last(), Some(&root.surface));
        let factor = (number(visual(s), "width") / 160.0).min(number(visual(s), "height") / 1000.0);
        let tolerance = 2.0 / (factor * s["output"]["scale"].as_f64().unwrap());
        let (_, x, y) = client.presses.last().unwrap();
        assert!(
            (*x - 80.0).abs() <= tolerance && (*y - 500.0).abs() <= tolerance,
            "{label}: inverse fit {x},{y}"
        );
    }
    assert_eq!(status()["cards"], json!([]));
    let initial = status();
    let socket = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR").unwrap())
        .join(initial["wayland_display"].as_str().unwrap());
    let connection = Connection::from_socket(UnixStream::connect(socket).unwrap()).unwrap();
    let mut queue = connection.new_event_queue();
    let qh = queue.handle();
    let mut client = Client::default();
    connection.display().get_registry(&qh, ());
    sync(&mut queue, &mut client);
    let mut root = client.uncommitted_window(&qh, "Notification minimum-size fixture", None);
    root.buffer.destroy();
    root.buffer = client.buffer(&qh, 160, 1000, [0x48, 0x80, 0x20, 255]);
    root.xdg.set_window_geometry(0, 0, 160, 1000);
    root.toplevel.set_min_size(160, 1000);
    root.surface.commit();
    sync(&mut queue, &mut client);
    root.surface.attach(Some(&root.buffer), 0, 0);
    root.surface.damage(0, 0, 160, 1000);
    root.surface.commit();
    wait(&mut queue, &mut client, "initial map", |c, s| {
        c.focus == Some(root.surface.clone())
            && s["cards"].as_array().is_some_and(|a| a.len() == 1)
            && s["notifications"]["available"] == true
    });
    let bus = zbus::blocking::Connection::session().unwrap();
    let mut identifier = 0;
    for (index, orientation) in ["normal", "left-up", "bottom-up", "right-up", "normal"]
        .into_iter()
        .enumerate()
    {
        control(json!({"command":"rotate","orientation":orientation}));
        let before = wait(&mut queue, &mut client, "rotation", |_, s| {
            s["output"]["orientation"] == orientation
        });
        let full_height = before["output"]["logical_height"]
            .as_f64()
            .unwrap_or_else(|| {
                number(&before["output"], "physical_height") / number(&before["output"], "scale")
            });
        let baseline = if orientation == "left-up" || orientation == "right-up" {
            number(&before["output"], "physical_width") / number(&before["output"], "scale") - 68.0
        } else {
            full_height - 68.0
        };
        identifier = notify(&bus, identifier);
        // This first gate uses existing geometry, not newly added metadata. The
        // retained old binary must fail because its real client never shrinks.
        let banner = wait(
            &mut queue,
            &mut client,
            "banner reserves foreground",
            |_, s| {
                (number(&s["app_area"], "height") - (baseline - 48.0)).abs() < 0.01
                    && (number(visual(s), "height") - number(&s["app_area"], "height")).abs() < 0.01
            },
        );
        click_root(
            &mut queue,
            &mut client,
            &root,
            &banner,
            &format!("well-banner-{index}"),
        );
        control(json!({"command":"notifications","open":true}));
        let dashboard = wait(
            &mut queue,
            &mut client,
            "dashboard keeps app focus",
            |c, s| {
                c.focus == Some(root.surface.clone())
                    && s["notifications"]["dashboard_open"] == true
                    && (number(&s["app_area"], "height") - (baseline - 112.0)).abs() < 0.01
                    && ((number(visual(s), "height") - number(&s["app_area"], "height")).abs()
                        < 0.01)
            },
        );
        let presses = client.presses.len();
        let keys = client.keys.len();
        let well_y = number(&dashboard["app_area"], "y") + number(&dashboard["app_area"], "height");
        drive(
            json!({"command":"click","output":dashboard["output"],"x":10.0,"y":well_y+20.0,"key":true}),
        );
        sync(&mut queue, &mut client);
        assert_eq!(
            client.presses.len(),
            presses,
            "dashboard contact cannot reach the app"
        );
        assert_eq!(
            client.keys.len(),
            keys + 1,
            "dashboard does not own physical typing"
        );
        assert_eq!(client.keys.last(), Some(&root.surface));
        click_root(
            &mut queue,
            &mut client,
            &root,
            &dashboard,
            &format!("well-dashboard-{index}"),
        );
        let popup = client.popup(&qh, &root, true);
        sync(&mut queue, &mut client);
        popup.map();
        sync(&mut queue, &mut client);
        let pose = status();
        let r = visual(&pose);
        let factor = (number(r, "width") / 160.0).min(number(r, "height") / 1000.0);
        let origin = (
            number(r, "x") + (number(r, "width") - 160.0 * factor) * 0.5,
            number(r, "y") + (number(r, "height") - 1000.0 * factor) * 0.5,
        );
        let &(x, y, w, h) = client.popup_rects.get(&popup.popup).unwrap();
        let point = (
            origin.0 + (f64::from(x) + f64::from(w) * 0.5) * factor,
            origin.1 + (f64::from(y) + f64::from(h) * 0.5) * factor,
        );
        assert_eq!(
            captured_pixel(&format!("well-popup-{index}"), &pose, point),
            [34, 85, 221, 255]
        );
        drive(
            json!({"command":"click","output":pose["output"],"x":point.0,"y":point.1,"key":true}),
        );
        sync(&mut queue, &mut client);
        assert_eq!(client.presses.last().unwrap().0, popup.surface);
        assert_eq!(client.keys.last(), Some(&popup.surface));
        popup.destroy();
        sync(&mut queue, &mut client);
        let s = status();
        let revision = s["notifications"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == identifier)
            .unwrap()["revision"]
            .clone();
        control(
            json!({"command":"notification_action","action":{"HideBanner":{"id":identifier,"revision":revision}}}),
        );
        control(json!({"command":"notifications","open":false}));
        let rail = wait(
            &mut queue,
            &mut client,
            "rail restores focus and geometry",
            |c, s| {
                c.focus == Some(root.surface.clone())
                    && (number(&s["app_area"], "height") - (baseline - 48.0)).abs() < 0.01
            },
        );
        click_root(
            &mut queue,
            &mut client,
            &root,
            &rail,
            &format!("well-rail-{index}"),
        );
    }
    root.toplevel.set_fullscreen(None);
    root.surface.commit();
    sync(&mut queue, &mut client);
    root.surface.commit();
    let fullscreen = wait(&mut queue, &mut client, "fullscreen well", |c, s| {
        latest_states(c, &root).contains(&2)
            && number(visual(s), "y") == 0.0
            && (number(visual(s), "height") - (number(&s["app_area"], "height") + 32.0)).abs()
                < 0.01
    });
    click_root(
        &mut queue,
        &mut client,
        &root,
        &fullscreen,
        "well-fullscreen",
    );
    root.toplevel.unset_fullscreen();
    root.surface.commit();
    sync(&mut queue, &mut client);
    root.surface.commit();
    wait(&mut queue, &mut client, "normal restore", |c, s| {
        !latest_states(c, &root).contains(&2) && number(visual(s), "y") == 32.0
    });
    identifier = notify(&bus, identifier);
    control(json!({"command":"notifications","open":true}));
    wait(
        &mut queue,
        &mut client,
        "dashboard moving before protection",
        |_, s| {
            number(&s["notifications"]["well"], "height") > 48.0
                && number(&s["notifications"]["well"], "height") < 112.0
        },
    );
    drive(json!({"command":"lock"}));
    notify(&bus, identifier);
    sync(&mut queue, &mut client);
    assert_eq!(
        status().as_object().unwrap().len(),
        3,
        "protected notification metadata must be redacted"
    );
    let protected_frames = status()["lock"]["unprotected_presentations"]
        .as_u64()
        .unwrap();
    let checked_frames = std::cell::Cell::new(0usize);
    drive(json!({"command":"unlock"}));
    let restored = wait(
        &mut queue,
        &mut client,
        "protected notification restore",
        |c, s| {
            if s["lock"]["unprotected_presentations"].as_u64().unwrap() > protected_frames {
                checked_frames.set(checked_frames.get() + 1);
                let boundary = number(visual(s), "y") + number(visual(s), "height");
                let well = number(&s["notifications"]["well"], "y");
                assert!(
                    (boundary - well).abs() <= 1.0,
                    "first visible resume must share the notification boundary: app={boundary}, well={well}"
                );
            }
            c.focus == Some(root.surface.clone())
                && (number(&s["app_area"], "height") - (652.0 - 48.0)).abs() < 0.01
                && checked_frames.get() > 0
        },
    );
    click_root(
        &mut queue,
        &mut client,
        &root,
        &restored,
        "well-protected-return",
    );
    client.text_enabled = true;
    let text = client.text_manager.as_ref().unwrap().get_text_input(
        client.seat.as_ref().unwrap(),
        &qh,
        (),
    );
    sync(&mut queue, &mut client);
    for (index, orientation) in ["normal", "left-up", "bottom-up", "right-up", "normal"]
        .into_iter()
        .enumerate()
    {
        control(json!({"command":"rotate","orientation":orientation}));
        control(json!({"command":"notifications","open":true}));
        let landscape = orientation == "left-up" || orientation == "right-up";
        let expected = if landscape { 64.0 } else { 300.0 };
        let shown = wait(
            &mut queue,
            &mut client,
            "keyboard with dashboard",
            |c, s| {
                c.focus == Some(root.surface.clone())
                    && s["output"]["orientation"] == orientation
                    && s["keyboard_visible"] == true
                    && (number(&s["app_area"], "height") - expected).abs() < 0.01
                    && (number(visual(s), "height") - expected).abs() < 0.01
            },
        );
        let keyboard = &shown["keyboard_rect"];
        let commits = client.ime_commits.len();
        let presses = client.presses.len();
        drive(
            json!({"command":"click","output":shown["output"],"x":number(keyboard,"width")/20.0,"y":number(keyboard,"y")+number(keyboard,"height")/4.0,"key":false}),
        );
        wait(
            &mut queue,
            &mut client,
            "trusted keyboard commit",
            |c, _| c.ime_commits.len() > commits,
        );
        assert_eq!(client.ime_commits.len(), commits + 1);
        assert_eq!(client.ime_commits.last().map(String::as_str), Some("q"));
        assert_eq!(
            client.presses.len(),
            presses,
            "keyboard contact cannot reach the app"
        );
        click_root(
            &mut queue,
            &mut client,
            &root,
            &shown,
            &format!("well-keyboard-{index}"),
        );
    }
    client.text_enabled = false;
    text.disable();
    text.commit();
    sync(&mut queue, &mut client);
    control(json!({"command":"notifications","open":false}));
    wait(&mut queue, &mut client, "keyboard retired", |_, s| {
        s["keyboard_visible"] == false && (number(&s["app_area"], "height") - 604.0).abs() < 0.01
    });
    let other = zbus::blocking::Connection::session().unwrap();
    assert_ne!(bus.unique_name(), other.unique_name());
    let other_id = notify(&other, 0);
    let sources = wait(&mut queue, &mut client, "distinct sources", |_, s| {
        s["notifications"]["items"]
            .as_array()
            .is_some_and(|a| a.len() == 2)
    });
    let entries = sources["notifications"]["items"].as_array().unwrap();
    assert_ne!(
        entries.iter().find(|n| n["id"] == identifier).unwrap()["source"],
        entries.iter().find(|n| n["id"] == other_id).unwrap()["source"]
    );
    control(json!({"command":"notification_action","action":"Clear"}));
    let cleared = wait(
        &mut queue,
        &mut client,
        "empty well restores original area",
        |_, s| {
            s["notifications"]["items"].as_array().unwrap().is_empty()
                && (number(&s["app_area"], "height") - 652.0).abs() < 0.01
        },
    );
    click_root(
        &mut queue,
        &mut client,
        &root,
        &cleared,
        "well-cleared-return",
    );
    root.destroy();
    sync(&mut queue, &mut client);
    assert_eq!(status()["cards"], json!([]));
    println!(
        "Notification reservation, oversized-buffer fit/pixels/local input, non-modal typing, popups, four poses/return, fullscreen, source isolation, protected arrival and exact geometry restoration verified"
    );
}

impl Dispatch<zwp_text_input_v3::ZwpTextInputV3, ()> for Client {
    fn event(
        state: &mut Self,
        input: &zwp_text_input_v3::ZwpTextInputV3,
        event: zwp_text_input_v3::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_text_input_v3::Event::Enter { .. } if state.text_enabled => {
                input.enable();
                input.set_content_type(
                    zwp_text_input_v3::ContentHint::empty(),
                    zwp_text_input_v3::ContentPurpose::Normal,
                );
                input.set_surrounding_text(String::new(), 0, 0);
                input.commit();
            }
            zwp_text_input_v3::Event::Leave { .. } => {
                state.pending_commit = None;
            }
            zwp_text_input_v3::Event::CommitString { text } => {
                state.pending_commit = text;
            }
            zwp_text_input_v3::Event::Done { .. } => {
                if let Some(text) = state.pending_commit.take() {
                    state.ime_commits.push(text);
                }
            }
            _ => {}
        }
    }
}
delegate_noop!(Client: ignore zwp_text_input_manager_v3::ZwpTextInputManagerV3);
