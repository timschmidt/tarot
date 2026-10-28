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
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, delegate_noop,
    protocol::{
        wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat,
        wl_shm, wl_shm_pool, wl_surface, wl_touch,
    },
};
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
    configures: usize,
    sizes: Vec<(xdg_toplevel::XdgToplevel, i32, i32)>,
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
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&color.repeat((width * height) as usize))
            .unwrap();
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
                "xdg_wm_base" => state.shell = Some(registry.bind(name, 1, qh, ())),
                "xdg_wm_dialog_v1" => state.dialogs = Some(registry.bind(name, 1, qh, ())),
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
            xdg_toplevel::Event::Configure { width, height, .. } => {
                state.sizes.push((toplevel.clone(), width, height))
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
delegate_noop!(Client: ignore xdg_wm_dialog_v1::XdgWmDialogV1);
delegate_noop!(Client: ignore xdg_dialog_v1::XdgDialogV1);
delegate_noop!(Client: ignore xdg_positioner::XdgPositioner);

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
