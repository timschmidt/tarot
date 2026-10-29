//! Exercise committed keyboard coordinates without constructing a renderer.
use super::*;
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use std::{
    os::{fd::AsFd, unix::net::UnixStream},
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use tarot_core::{View, session::LockPhase};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::{
        wl_buffer, wl_compositor, wl_registry, wl_shm, wl_shm_pool, wl_subcompositor,
        wl_subsurface, wl_surface,
    },
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

struct KeyboardClient {
    root: wl_surface::WlSurface,
    buffer: wl_buffer::WlBuffer,
    configured: bool,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for KeyboardClient {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for KeyboardClient {
    fn event(
        state: &mut Self,
        layer: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, .. } = event {
            layer.ack_configure(serial);
            state.root.attach(Some(&state.buffer), 0, 0);
            state.root.damage(0, 0, 360, 240);
            state.root.commit();
            state.configured = true;
        }
    }
}

delegate_noop!(KeyboardClient: ignore wl_compositor::WlCompositor);
delegate_noop!(KeyboardClient: ignore wl_subcompositor::WlSubcompositor);
delegate_noop!(KeyboardClient: ignore wl_subsurface::WlSubsurface);
delegate_noop!(KeyboardClient: ignore wl_surface::WlSurface);
delegate_noop!(KeyboardClient: ignore wl_shm::WlShm);
delegate_noop!(KeyboardClient: ignore wl_shm_pool::WlShmPool);
delegate_noop!(KeyboardClient: ignore wl_buffer::WlBuffer);
delegate_noop!(KeyboardClient: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);

#[test]
fn mapped_keyboard_has_coordinates_before_first_render() {
    // Compositor::new binds a listening socket. Isolate its runtime directory
    // in a subprocess rather than mutating the parallel test runner's env.
    const CHILD: &str = "TAROT_TEST_KEYBOARD_PLACEMENT";
    if std::env::var_os(CHILD).is_none() {
        let runtime = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "focus::keyboard_tests::mapped_keyboard_has_coordinates_before_first_render",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("XDG_RUNTIME_DIR", runtime.path())
            .output()
            .unwrap();
        assert!(
            output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "isolated placement regression failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let (server_socket, client_socket) = UnixStream::pair().unwrap();
    let display = Display::<Compositor>::new().unwrap();
    let mut dh = display.handle();
    let client = dh
        .insert_client(
            server_socket,
            Arc::new(crate::state::ClientState {
                trusted_input_method: true,
                ..Default::default()
            }),
        )
        .unwrap();
    let mut event_loop = EventLoop::try_new().unwrap();
    let mut state = Compositor::new(&mut event_loop, display);
    state.shell.view = View::App(WindowId(1));
    let mut data = crate::CalloopData {
        state,
        display_handle: dh.clone(),
    };
    let (ready_send, ready_receive) = mpsc::channel();
    let (stop_send, stop_receive) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let conn = Connection::from_socket(client_socket).unwrap();
        let (globals, mut queue) = registry_queue_init::<KeyboardClient>(&conn).unwrap();
        let qh = queue.handle();
        let compositor: wl_compositor::WlCompositor = globals.bind(&qh, 1..=1, ()).unwrap();
        let subcompositor: wl_subcompositor::WlSubcompositor =
            globals.bind(&qh, 1..=1, ()).unwrap();
        let shm: wl_shm::WlShm = globals.bind(&qh, 1..=1, ()).unwrap();
        let layer_shell: zwlr_layer_shell_v1::ZwlrLayerShellV1 =
            globals.bind(&qh, 1..=1, ()).unwrap();
        let root = compositor.create_surface(&qh, ());
        let child = compositor.create_surface(&qh, ());
        let sibling = compositor.create_surface(&qh, ());
        let layer = layer_shell.get_layer_surface(
            &root,
            None,
            zwlr_layer_shell_v1::Layer::Overlay,
            "tarot-keyboard".into(),
            &qh,
            (),
        );
        use zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity};
        layer.set_anchor(Anchor::Bottom | Anchor::Left | Anchor::Right);
        layer.set_size(360, 240);
        layer.set_margin(0, 0, 28, 0);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        let file = tempfile::tempfile().unwrap();
        file.set_len(360 * 240 * 4).unwrap();
        let pool = shm.create_pool(file.as_fd(), 360 * 240 * 4, &qh, ());
        let buffer = pool.create_buffer(0, 360, 240, 360 * 4, wl_shm::Format::Xrgb8888, &qh, ());
        let subsurface = subcompositor.get_subsurface(&child, &root, &qh, ());
        subsurface.set_position(40, 50);
        child.attach(Some(&buffer), 0, 0);
        child.commit();
        sibling.attach(Some(&buffer), 0, 0);
        sibling.commit();
        root.commit();
        let mut state = KeyboardClient {
            root,
            buffer,
            configured: false,
        };
        while !state.configured {
            queue.roundtrip(&mut state).unwrap();
        }
        // The configure callback queues the first buffer commit after its sync.
        queue.roundtrip(&mut state).unwrap();
        ready_send.send(sibling.id().protocol_id()).unwrap();
        stop_receive.recv_timeout(Duration::from_secs(10)).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let sibling_id = loop {
        assert!(Instant::now() < deadline, "keyboard mapping timed out");
        event_loop
            .dispatch(Some(Duration::from_millis(10)), &mut data)
            .unwrap();
        dh.flush_clients().unwrap();
        if let Ok(id) = ready_receive.try_recv() {
            break id;
        }
    };
    assert!(data.state.native_keyboard_visible());
    assert!(data.state.presented_surfaces.is_empty());
    assert!(data.state.presented_windows.is_empty());
    let rect = data.state.layout.keyboard();
    let origin: Point<f64, Logical> = (f64::from(rect.x), f64::from(rect.y)).into();
    let root_point = origin + Point::from((10.0, 10.0));
    let child_point = origin + Point::from((45.0, 55.0));
    let root = data.state.keyboard_surface_under(root_point).unwrap().0;
    let child = data.state.keyboard_surface_under(child_point).unwrap().0;
    let sibling = SurfaceFocus::from(
        client
            .object_from_protocol_id::<WlSurface>(&dh, sibling_id)
            .unwrap(),
    );
    let observations = [
        root.position(&data.state, root_point),
        child.position(&data.state, child_point),
        child.position(&data.state, origin + Point::from((-10.0, -10.0))),
        sibling.position(&data.state, root_point),
    ];
    data.state.shell.view = View::Overview;
    let hidden = root.position(&data.state, root_point);
    data.state.shell.view = View::App(WindowId(1));
    data.state.protection.gate.phase = LockPhase::Locked;
    let protected = root.position(&data.state, root_point);
    stop_send.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(
        observations,
        [
            Some((10.0, 10.0).into()),
            Some((5.0, 5.0).into()),
            Some((-50.0, -60.0).into()),
            None,
        ]
    );
    assert_eq!(hidden, None);
    assert_eq!(protected, None);
}
