// Adapted from Smithay PR #2141 at e96548206bdb134c272ec302cb1e0f32fa580836.
// Upstream license: MIT; see third-party/Smithay-MIT.txt.
use smithay::reexports::wayland_server::Client;
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::{
    Barrier, CompositorClientState, CompositorHandler, CompositorState, SubsurfaceCachedState,
    add_blocker, with_states,
};

#[test]
fn subsurface_position_applies_with_delayed_parent_transaction() {
    use std::{
        os::unix::net::UnixStream,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    };

    use smithay::reexports::wayland_server::{
        Display,
        backend::{ClientData, ClientId, DisconnectReason},
    };
    use wayland_client::{
        Connection, Dispatch, QueueHandle,
        globals::{GlobalListContents, registry_queue_init},
        protocol::{
            wl_callback::WlCallback, wl_compositor::WlCompositor, wl_registry::WlRegistry,
            wl_subcompositor::WlSubcompositor, wl_subsurface::WlSubsurface, wl_surface::WlSurface,
        },
    };

    #[derive(Default)]
    struct TestClientData {
        compositor_state: CompositorClientState,
    }

    impl ClientData for TestClientData {
        fn initialized(&self, _client_id: ClientId) {}
        fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
    }

    type Observation = (bool, Point<i32, Logical>);
    struct TestState {
        compositor_state: CompositorState,
        surfaces: Vec<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
        blocker: Barrier,
        observed_locations: Arc<Mutex<Vec<Observation>>>,
    }

    impl CompositorHandler for TestState {
        fn compositor_state(&mut self) -> &mut CompositorState {
            &mut self.compositor_state
        }

        fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
            &client
                .get_data::<TestClientData>()
                .unwrap()
                .compositor_state
        }

        fn new_surface(
            &mut self,
            surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
        ) {
            if self.surfaces.is_empty() {
                add_blocker(surface, self.blocker.clone());
            }
            self.surfaces.push(surface.clone());
        }

        fn commit(
            &mut self,
            surface: &smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
        ) {
            if let Some(child) = self.surfaces.get(1) {
                let location = with_states(child, |states| {
                    states
                        .cached_state
                        .get::<SubsurfaceCachedState>()
                        .current()
                        .location
                });
                self.observed_locations
                    .lock()
                    .unwrap()
                    .push((self.surfaces.first() == Some(surface), location));
            }
        }
    }

    smithay::delegate_compositor!(TestState);

    struct ClientState;

    impl Dispatch<WlRegistry, GlobalListContents> for ClientState {
        fn event(
            _state: &mut Self,
            _proxy: &WlRegistry,
            _event: wayland_client::protocol::wl_registry::Event,
            _data: &GlobalListContents,
            _conn: &Connection,
            _qh: &QueueHandle<Self>,
        ) {
        }
    }

    wayland_client::delegate_noop!(ClientState: ignore WlCallback);
    wayland_client::delegate_noop!(ClientState: ignore WlCompositor);
    wayland_client::delegate_noop!(ClientState: ignore WlSubcompositor);
    wayland_client::delegate_noop!(ClientState: ignore WlSubsurface);
    wayland_client::delegate_noop!(ClientState: ignore WlSurface);

    let (server_socket, client_socket) = UnixStream::pair().unwrap();
    let observed_locations = Arc::new(Mutex::new(Vec::new()));
    let server_result = observed_locations.clone();
    let release_blocker = Arc::new(AtomicBool::new(false));
    let server_release = release_blocker.clone();

    let server = thread::spawn(move || {
        let mut display = Display::<TestState>::new().unwrap();
        let mut dh = display.handle();
        let compositor_state = CompositorState::new::<TestState>(&dh);
        let client_data = Arc::new(TestClientData::default());
        dh.insert_client(server_socket, client_data.clone())
            .unwrap();
        let mut state = TestState {
            compositor_state,
            surfaces: Vec::new(),
            blocker: Barrier::new(false),
            observed_locations: server_result,
        };

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while state
            .observed_locations
            .lock()
            .unwrap()
            .iter()
            .filter(|(is_parent, _)| *is_parent)
            .count()
            < 2
        {
            assert!(
                std::time::Instant::now() < deadline,
                "delayed transaction test timed out"
            );
            display.dispatch_clients(&mut state).unwrap();
            if server_release.load(Ordering::Acquire) && !state.blocker.is_signaled() {
                state.blocker.signal();
                client_data
                    .compositor_state
                    .blocker_cleared(&mut state, &dh);
            }
            display.flush_clients().unwrap();
            thread::yield_now();
        }
    });

    let conn = Connection::from_socket(client_socket).unwrap();
    let (globals, mut event_queue) = registry_queue_init::<ClientState>(&conn).unwrap();
    let qh = event_queue.handle();
    let compositor = globals.bind::<WlCompositor, _, _>(&qh, 1..=6, ()).unwrap();
    let subcompositor = globals
        .bind::<WlSubcompositor, _, _>(&qh, 1..=1, ())
        .unwrap();
    let parent = compositor.create_surface(&qh, ());
    let child = compositor.create_surface(&qh, ());
    let subsurface = subcompositor.get_subsurface(&child, &parent, &qh, ());

    subsurface.set_desync();
    child.commit();
    subsurface.set_position(37, 23);
    parent.commit();
    child.commit();
    subsurface.set_sync();
    child.commit();
    subsurface.set_position(71, 41);
    parent.commit();
    event_queue.roundtrip(&mut ClientState).unwrap();
    release_blocker.store(true, Ordering::Release);

    server.join().unwrap();
    assert_eq!(
        *observed_locations.lock().unwrap(),
        vec![
            (false, (0, 0).into()),
            (false, (0, 0).into()),
            (true, (37, 23).into()),
            (false, (71, 41).into()),
            (true, (71, 41).into()),
        ]
    );
}
