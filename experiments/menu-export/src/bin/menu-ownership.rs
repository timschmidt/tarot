// SPDX-License-Identifier: MIT OR Apache-2.0
//! Private protocol/bus experiment. No rendering or production capability.
#[path = "../gtk.rs"]
mod gtk;
#[path = "../process_identity.rs"]
mod process_identity;

use gio::{
    glib::{self, variant::ToVariant},
    prelude::*,
};
use gtk::server::{gtk_shell1, gtk_surface1};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    os::{
        fd::OwnedFd,
        unix::net::{UnixListener, UnixStream},
    },
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use wayland_server::protocol::{wl_compositor, wl_region, wl_surface};
use wayland_server::{
    Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, New, Resource,
};

const MENU: &str = "/org/tarot/Ownership/menu";
const ACTIONS: &str = "/org/tarot/Ownership/actions";
const BUS: &str = "org.tarot.OwnershipExperiment";

fn connection() -> gio::DBusConnection {
    let c = gio::DBusConnection::for_address_sync(
        &std::env::var("DBUS_SESSION_BUS_ADDRESS").expect("Use dbus-run-session"),
        gio::DBusConnectionFlags::AUTHENTICATION_CLIENT
            | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
        None,
        None::<&gio::Cancellable>,
    )
    .unwrap();
    c.set_exit_on_close(false);
    c
}
fn pump(context: &glib::MainContext) {
    for _ in 0..64 {
        if !context.pending() {
            break;
        }
        context.iteration(false);
    }
}
#[derive(Clone)]
struct Life {
    surface: Arc<AtomicBool>,
    connection: Arc<AtomicBool>,
    revision: Arc<AtomicU64>,
}
impl Life {
    fn current(&self, revision: u64) -> bool {
        self.surface.load(Ordering::SeqCst)
            && self.connection.load(Ordering::SeqCst)
            && self.revision.load(Ordering::SeqCst) == revision
    }
}
struct Peer {
    process: Arc<OwnedFd>,
    alive: Arc<AtomicBool>,
}
impl wayland_server::backend::ClientData for Peer {
    fn disconnected(
        &self,
        _: wayland_server::backend::ClientId,
        _: wayland_server::backend::DisconnectReason,
    ) {
        self.alive.store(false, Ordering::SeqCst);
    }
}
#[derive(Clone)]
struct Claim {
    life: Life,
    revision: u64,
    peer: Arc<OwnedFd>,
    owner: String,
    menu: String,
    actions: String,
}
impl Claim {
    fn current(&self) -> bool {
        self.life.current(self.revision)
    }
}
#[derive(Default)]
struct Server {
    claims: Vec<Claim>,
}
impl GlobalDispatch<wl_compositor::WlCompositor, ()> for Server {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<wl_compositor::WlCompositor>,
        _: &(),
        init: &mut DataInit<'_, Self>,
    ) {
        init.init(resource, ());
    }
}
impl Dispatch<wl_compositor::WlCompositor, ()> for Server {
    fn request(
        _: &mut Self,
        client: &Client,
        _: &wl_compositor::WlCompositor,
        request: wl_compositor::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_compositor::Request::CreateSurface { id } => {
                init.init(
                    id,
                    Life {
                        surface: Arc::new(AtomicBool::new(true)),
                        connection: client.get_data::<Peer>().unwrap().alive.clone(),
                        revision: Arc::new(AtomicU64::new(0)),
                    },
                );
            }
            wl_compositor::Request::CreateRegion { id } => {
                init.init(id, ());
            }
            _ => unreachable!(),
        }
    }
}
impl Dispatch<wl_region::WlRegion, ()> for Server {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_region::WlRegion,
        _: wl_region::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}
impl Dispatch<wl_surface::WlSurface, Life> for Server {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wl_surface::WlSurface,
        request: wl_surface::Request,
        _: &Life,
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let wl_surface::Request::Frame { callback } = request {
            let callback = init.init(callback, ());
            callback.done(0);
        }
    }
    fn destroyed(
        _: &mut Self,
        _: wayland_server::backend::ClientId,
        _: &wl_surface::WlSurface,
        life: &Life,
    ) {
        life.surface.store(false, Ordering::SeqCst);
    }
}
impl Dispatch<wayland_server::protocol::wl_callback::WlCallback, ()> for Server {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &wayland_server::protocol::wl_callback::WlCallback,
        _: wayland_server::protocol::wl_callback::Request,
        _: &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
    }
}
impl GlobalDispatch<gtk_shell1::GtkShell1, ()> for Server {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<gtk_shell1::GtkShell1>,
        _: &(),
        init: &mut DataInit<'_, Self>,
    ) {
        init.init(resource, ()).capabilities(0);
    }
}
impl Dispatch<gtk_shell1::GtkShell1, ()> for Server {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &gtk_shell1::GtkShell1,
        request: gtk_shell1::Request,
        _: &(),
        _: &DisplayHandle,
        init: &mut DataInit<'_, Self>,
    ) {
        if let gtk_shell1::Request::GetGtkSurface {
            gtk_surface,
            surface,
        } = request
        {
            init.init(gtk_surface, surface.data::<Life>().unwrap().clone());
        }
    }
}
impl Dispatch<gtk_surface1::GtkSurface1, Life> for Server {
    fn request(
        state: &mut Self,
        client: &Client,
        _: &gtk_surface1::GtkSurface1,
        request: gtk_surface1::Request,
        life: &Life,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let gtk_surface1::Request::SetDbusProperties {
            app_menu_path,
            application_object_path,
            unique_bus_name,
            ..
        } = request
        {
            let revision = life.revision.fetch_add(1, Ordering::SeqCst) + 1;
            state.claims.push(Claim {
                life: life.clone(),
                revision,
                peer: client.get_data::<Peer>().unwrap().process.clone(),
                owner: unique_bus_name.unwrap_or_default(),
                menu: app_menu_path.unwrap_or_default(),
                actions: application_object_path.unwrap_or_default(),
            });
        }
    }
    fn destroyed(
        _: &mut Self,
        _: wayland_server::backend::ClientId,
        _: &gtk_surface1::GtkSurface1,
        life: &Life,
    ) {
        life.revision.fetch_add(1, Ordering::SeqCst);
    }
}

fn credential_fd(reply: &glib::Variant, list: Option<&gio::UnixFDList>) -> Result<OwnedFd, String> {
    let (credentials,) = reply
        .get::<(HashMap<String, glib::Variant>,)>()
        .ok_or("invalid credentials reply")?;
    let handle = credentials
        .get("ProcessFD")
        .and_then(|v| v.get::<glib::variant::Handle>())
        .ok_or("ProcessFD absent or wrong type")?;
    let list = list.ok_or("ProcessFD list absent")?;
    // GIO's wrapper requires an in-range index; malformed handles must not
    // reach its assertion or be confused with an arbitrary local descriptor.
    if handle.0 < 0 || handle.0 >= list.length() {
        return Err("ProcessFD index out of range".into());
    }
    list.get(handle.0).map_err(|e| e.to_string())
}
fn exporter_fd(c: &gio::DBusConnection, owner: &str) -> Result<OwnedFd, String> {
    if !gio::dbus_is_unique_name(owner) {
        return Err("claim requires a unique bus name".into());
    }
    let (reply, list) = c
        .call_with_unix_fd_list_sync(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetConnectionCredentials",
            Some(&(owner,).to_variant()),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            300,
            None::<&gio::UnixFDList>,
            None::<&gio::Cancellable>,
        )
        .map_err(|e| e.to_string())?;
    credential_fd(&reply, list.as_ref())
}
struct Proof {
    claim: Claim,
    exporter: OwnedFd,
}
enum Work {
    Validate(Claim, mpsc::Sender<Result<Proof, String>>),
    Activate(Proof, mpsc::Sender<Result<Value, String>>),
    Stop,
}
struct Worker {
    send: mpsc::Sender<Work>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Worker {
    fn start() -> Self {
        let (send, receive) = mpsc::channel();
        let thread = thread::spawn(move || {
            let context = glib::MainContext::new();
            context.with_thread_default(|| {
                let c = connection();
                while let Ok(work) = receive.recv() {
                    match work {
                        Work::Stop => break,
                        Work::Validate(claim, result) => {
                            let validation = (|| {
                                if !claim.current() { return Err("retired claim".into()); }
                                if !glib::Variant::is_object_path(&claim.menu) || !glib::Variant::is_object_path(&claim.actions) { return Err("invalid object path".into()); }
                                let exporter = exporter_fd(&c, &claim.owner)?;
                                if !process_identity::same_live_process(&claim.peer, &exporter).map_err(|e| e.to_string())? { return Err("different process".into()); }
                                if !claim.current() { return Err("retired validation".into()); }
                                Ok(Proof { claim, exporter })
                            })();
                            let _ = result.send(validation);
                        }
                        Work::Activate(proof, result) => {
                            let activation = (|| {
                                let claim = &proof.claim;
                                if !claim.current() { return Err("retired choice".into()); }
                                let current = exporter_fd(&c, &claim.owner)?;
                                if !process_identity::same_live_process(&proof.exporter, &current).map_err(|e| e.to_string())?
                                    || !process_identity::same_live_process(&claim.peer, &current).map_err(|e| e.to_string())? { return Err("export owner changed or exited".into()); }
                                let menu = gio::DBusMenuModel::get(&c, Some(&claim.owner), &claim.menu);
                                let actions = gio::DBusActionGroup::get(&c, Some(&claim.owner), &claim.actions);
                                let deadline = Instant::now() + Duration::from_secs(2);
                                while menu.n_items() != 1 || !actions.has_action("choose") {
                                    pump(&context);
                                    if !claim.current() { return Err("retired while loading".into()); }
                                    if Instant::now() >= deadline { return Err("export load deadline".into()); }
                                    thread::sleep(Duration::from_millis(1));
                                }
                                let label = menu.item_attribute_value(0, "label", None).and_then(|v| v.get::<String>()).ok_or("missing real menu label")?;
                                if !actions.is_action_enabled("choose") || actions.action_parameter_type("choose").as_deref() != Some(glib::VariantTy::STRING) { return Err("action disabled or type changed".into()); }
                                if !claim.current() { return Err("retired before dispatch".into()); }
                                // A reply is dispatch evidence. The parent separately requires
                                // the actual exporter callback with this exact argument.
                                c.call_sync(Some(&claim.owner), &claim.actions, "org.gtk.Actions", "Activate",
                                    Some(&("choose", vec!["night".to_variant()], HashMap::<String, glib::Variant>::new()).to_variant()),
                                    None, gio::DBusCallFlags::NO_AUTO_START, 300, None::<&gio::Cancellable>).map_err(|e| e.to_string())?;
                                Ok(json!({"menu_label":label,"destination":claim.owner,"target":"night"}))
                            })();
                            let _ = result.send(activation);
                        }
                    }
                }
                c.close_sync(None::<&gio::Cancellable>).unwrap();
            }).unwrap();
        });
        Self {
            send,
            thread: Some(thread),
        }
    }
    fn validate(&self, claim: Claim) -> mpsc::Receiver<Result<Proof, String>> {
        let (tx, rx) = mpsc::channel();
        self.send.send(Work::Validate(claim, tx)).unwrap();
        rx
    }
    fn activate(&self, proof: Proof) -> mpsc::Receiver<Result<Value, String>> {
        let (tx, rx) = mpsc::channel();
        self.send.send(Work::Activate(proof, tx)).unwrap();
        rx
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.send.send(Work::Stop);
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
    }
}

struct Fixture {
    child: Child,
    input: ChildStdin,
    events: mpsc::Receiver<Value>,
    pending: Vec<Value>,
}
impl Fixture {
    fn start(path: &Path, name: &str) -> Self {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--producer", path.to_str().unwrap(), name])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let output = child.stdout.take().unwrap();
        let (tx, events) = mpsc::sync_channel(128);
        thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                match line {
                    Ok(line) => {
                        if tx.send(serde_json::from_str(&line).unwrap()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            input: child.stdin.take().unwrap(),
            child,
            events,
            pending: vec![],
        }
    }
    fn send(&mut self, value: Value) {
        writeln!(self.input, "{value}").unwrap();
        self.input.flush().unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
struct Harness {
    display: Display<Server>,
    server: Server,
    listener: UnixListener,
    worker: Worker,
}
impl Harness {
    fn tick(&mut self) {
        loop {
            match self.listener.accept() {
                Ok((socket, _)) => {
                    let process = process_identity::peer(&socket).unwrap();
                    process_identity::identity(&process).unwrap();
                    self.display
                        .handle()
                        .insert_client(
                            socket,
                            Arc::new(Peer {
                                process: Arc::new(process),
                                alive: Arc::new(AtomicBool::new(true)),
                            }),
                        )
                        .unwrap();
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => panic!("accept: {e}"),
            }
        }
        self.display.dispatch_clients(&mut self.server).unwrap();
        self.display.flush_clients().unwrap();
    }
    fn until<T>(&mut self, mut ready: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(4);
        loop {
            self.tick();
            if let Some(value) = ready() {
                return value;
            }
            assert!(Instant::now() < deadline, "ownership experiment deadline");
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn event(&mut self, fixture: &mut Fixture, kind: &str) -> Value {
        self.until(|| {
            fixture.pending.extend(fixture.events.try_iter());
            fixture
                .pending
                .iter()
                .position(|v| v["event"] == kind)
                .map(|i| fixture.pending.remove(i))
        })
    }
    fn command(&mut self, fixture: &mut Fixture, value: Value) -> Value {
        fixture.send(value);
        self.event(fixture, "ack")
    }
    fn claim(&mut self) -> Claim {
        assert_eq!(self.server.claims.len(), 1);
        self.server.claims.pop().unwrap()
    }
    fn proof(&mut self, claim: Claim) -> Proof {
        let rx = self.worker.validate(claim);
        self.until(|| rx.try_recv().ok())
            .unwrap_or_else(|e| panic!("validation: {e}"))
    }
    fn activate(&mut self, proof: Proof, fixture: &mut Fixture) -> Value {
        assert!(proof.claim.current());
        let rx = self.worker.activate(proof);
        let reply = self.until(|| rx.try_recv().ok()).unwrap();
        let callback = self.event(fixture, "activated");
        assert_eq!(callback["target"], "night");
        json!({"reply":reply,"callback":callback})
    }
    fn reject_retired_choice(&mut self, proof: Proof) {
        assert!(!proof.claim.current());
        let rx = self.worker.activate(proof);
        assert!(matches!(self.until(|| rx.try_recv().ok()), Err(e) if e == "retired choice"));
    }
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--producer") {
        producer(Path::new(&args[2]), &args[3]);
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("wayland");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let display = Display::<Server>::new().unwrap();
    display
        .handle()
        .create_global::<Server, wl_compositor::WlCompositor, _>(1, ());
    display
        .handle()
        .create_global::<Server, gtk_shell1::GtkShell1, _>(4, ());
    let mut h = Harness {
        display,
        listener,
        server: Server::default(),
        worker: Worker::start(),
    };
    let mut a = Fixture::start(&path, "owner");
    let a_ready = h.event(&mut a, "ready");
    let a_claim = h.claim();
    let mut b = Fixture::start(&path, "other");
    let b_ready = h.event(&mut b, "ready");
    let b_claim = h.claim();
    assert_ne!(a_ready["pid"], b_ready["pid"]);
    assert_eq!(a_ready["uid"], b_ready["uid"]);
    assert_eq!(a_ready["cgroup"], b_ready["cgroup"]);
    assert!(!process_identity::same_live_process(&a_claim.peer, &b_claim.peer).unwrap());
    let a_proof = h.proof(a_claim.clone());
    let first = h.activate(a_proof, &mut a);
    let b_proof = h.proof(b_claim.clone());
    let second = h.activate(b_proof, &mut b);
    h.command(&mut b, json!({"command":"claim","owner":a_ready["owner"]}));
    let forged = h.claim();
    let rx = h.worker.validate(forged);
    let wrong = h.until(|| rx.try_recv().ok());
    assert!(matches!(wrong, Err(ref e) if e == "different process"));
    h.command(&mut b, json!({"command":"claim","owner":BUS}));
    let alias = h.claim();
    let rx = h.worker.validate(alias);
    assert!(h.until(|| rx.try_recv().ok()).is_err());
    h.command(&mut b, json!({"command":"claim","owner":b_ready["owner"]}));
    let b_current = h.claim();

    let old_bus_proof = h.proof(a_claim.clone());
    h.command(&mut a, json!({"command":"disconnect-bus"}));
    assert!(
        old_bus_proof.claim.current(),
        "same surface/process, but bus connection is gone"
    );
    let rx = h.worker.activate(old_bus_proof);
    assert!(h.until(|| rx.try_recv().ok()).is_err());
    let reconnected = h.command(&mut a, json!({"command":"reconnect-bus"}));
    assert_ne!(reconnected["owner"], a_ready["owner"]);
    let replacement = h.claim();
    assert!(!a_claim.current());
    assert!(process_identity::same_live_process(&a_claim.peer, &replacement.peer).unwrap());
    let proof = h.proof(replacement.clone());
    let replaced = h.activate(proof, &mut a);

    let before_destroy = h.proof(replacement.clone());
    let rx = h.worker.validate(replacement.clone());
    h.command(&mut a, json!({"command":"destroy-surface"}));
    if let Ok(proof) = h.until(|| rx.try_recv().ok()) {
        h.reject_retired_choice(proof);
    }
    h.reject_retired_choice(before_destroy);
    assert!(!replacement.current());
    h.command(&mut a, json!({"command":"create-surface"}));
    let recreated = h.claim();
    let proof = h.proof(recreated.clone());
    let fresh = h.activate(proof, &mut a);

    let before_change = h.proof(recreated.clone());
    let rx = h.worker.validate(recreated.clone());
    h.command(&mut a, json!({"command":"claim","owner":b_ready["owner"]}));
    let changed = h.claim();
    if let Ok(proof) = h.until(|| rx.try_recv().ok()) {
        h.reject_retired_choice(proof);
    }
    h.reject_retired_choice(before_change);
    let rx = h.worker.validate(changed);
    assert!(h.until(|| rx.try_recv().ok()).is_err());
    let owner = reconnected["owner"].clone();
    h.command(&mut a, json!({"command":"claim","owner":owner}));
    let restored = h.claim();
    let proof = h.proof(restored.clone());
    h.command(&mut a, json!({"command":"release-menu"}));
    h.reject_retired_choice(proof);
    h.command(&mut a, json!({"command":"create-menu"}));
    let fresh_menu = h.claim();
    let proof = h.proof(fresh_menu);
    let after_release = h.activate(proof, &mut a);

    let b_counts = h.command(&mut b, json!({"command":"counts"}));
    assert_eq!(b_counts["activations"], 1);
    let dead_proof = h.proof(b_current.clone());
    b.child.kill().unwrap();
    b.child.wait().unwrap();
    h.until(|| (!b_current.current()).then_some(()));
    assert!(!process_identity::live(&dead_proof.exporter).unwrap());
    assert!(!process_identity::live(&dead_proof.claim.peer).unwrap());
    let rx = h.worker.activate(dead_proof);
    assert!(h.until(|| rx.try_recv().ok()).is_err());
    let empty = (HashMap::<String, glib::Variant>::new(),).to_variant();
    assert!(credential_fd(&empty, None).is_err());
    let invalid = ([(
        "ProcessFD".to_string(),
        glib::variant::Handle(0).to_variant(),
    )]
    .into_iter()
    .collect::<HashMap<_, _>>(),)
        .to_variant();
    assert!(credential_fd(&invalid, None).is_err());
    assert!(credential_fd(&invalid, Some(&gio::UnixFDList::new())).is_err());
    for value in [
        glib::variant::Handle(-1).to_variant(),
        "not a descriptor".to_variant(),
    ] {
        let malformed = ([("ProcessFD".to_string(), value)]
            .into_iter()
            .collect::<HashMap<_, _>>(),)
            .to_variant();
        assert!(credential_fd(&malformed, Some(&gio::UnixFDList::new())).is_err());
    }
    let ordinary: OwnedFd = std::fs::File::open("/dev/null").unwrap().into();
    assert!(process_identity::identity(&ordinary).is_err());
    // Exact callback counts reject accidental forwarding from all denied cases.
    a.pending.extend(a.events.try_iter());
    b.pending.extend(b.events.try_iter());
    assert!(!a.pending.iter().any(|v| v["event"] == "activated"));
    assert!(!b.pending.iter().any(|v| v["event"] == "activated"));
    let counts = h.command(&mut a, json!({"command":"counts"}));
    assert_eq!(counts["activations"], 4);
    h.command(&mut a, json!({"command":"exit"}));
    assert!(a.child.wait().unwrap().success());
    println!("{}", serde_json::to_string_pretty(&json!({"passed":true,"scope":"unmapped Wayland resources and real GIO exports on an owned private bus; no rendering or production capability", "producers":[a_ready,b_ready],"different_processes_same_uid_cgroup":true,"pidfs_peer_and_bus_handles_match":true,"wrong_client_denied":true,"well_known_claim_denied":true,"exporter_connection_replacement_retires_old_choices":true,"surface_destroy_and_recreation_retire_late_validation":true,"claim_revision_retires_late_validation":true,"extension_release_retires_choice":true,"process_exit_retires_handles":true,"missing_malformed_non_process_descriptors_denied":true,"actual_callbacks":[first,second,replaced,fresh,after_release],"same_process_after_bus_reconnect":true,"phone_ui_tested":false,"target_arch":std::env::consts::ARCH,"gtk_shell_version":4,"global_menu_capabilities":0,"kernel":std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap().trim()})).unwrap());
}

#[derive(Default)]
struct ProducerWayland {
    compositor: Option<wayland_client::protocol::wl_compositor::WlCompositor>,
    shell: Option<gtk::client::gtk_shell1::GtkShell1>,
}
impl wayland_client::Dispatch<wayland_client::protocol::wl_registry::WlRegistry, ()>
    for ProducerWayland
{
    fn event(
        state: &mut Self,
        registry: &wayland_client::protocol::wl_registry::WlRegistry,
        event: wayland_client::protocol::wl_registry::Event,
        _: &(),
        _: &wayland_client::Connection,
        qh: &wayland_client::QueueHandle<Self>,
    ) {
        if let wayland_client::protocol::wl_registry::Event::Global {
            name, interface, ..
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => state.compositor = Some(registry.bind(name, 1, qh, ())),
                "gtk_shell1" => state.shell = Some(registry.bind(name, 4, qh, ())),
                _ => (),
            }
        }
    }
}
impl wayland_client::Dispatch<gtk::client::gtk_shell1::GtkShell1, ()> for ProducerWayland {
    fn event(
        _: &mut Self,
        _: &gtk::client::gtk_shell1::GtkShell1,
        event: gtk::client::gtk_shell1::Event,
        _: &(),
        _: &wayland_client::Connection,
        _: &wayland_client::QueueHandle<Self>,
    ) {
        let gtk::client::gtk_shell1::Event::Capabilities { capabilities } = event;
        assert_eq!(capabilities, 0);
    }
}
wayland_client::delegate_noop!(ProducerWayland: ignore wayland_client::protocol::wl_compositor::WlCompositor);
wayland_client::delegate_noop!(ProducerWayland: ignore wayland_client::protocol::wl_surface::WlSurface);
wayland_client::delegate_noop!(ProducerWayland: ignore gtk::client::gtk_surface1::GtkSurface1);

fn emit(value: Value) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}
fn producer(path: &Path, name: &str) {
    let context = glib::MainContext::new();
    context.with_thread_default(|| {
        let activations = std::rc::Rc::new(std::cell::Cell::new(0));
        let group = gio::SimpleActionGroup::new(); let choose = gio::SimpleAction::new("choose", Some(glib::VariantTy::STRING));
        let count = activations.clone(); let label = name.to_string();
        choose.connect_activate(move |_, value| { count.set(count.get()+1); emit(json!({"event":"activated","producer":label,"target":value.unwrap().get::<String>().unwrap()})); }); group.add_action(&choose);
        let menu = gio::Menu::new(); let item = gio::MenuItem::new(Some("Choose night"), Some("app.choose")); item.set_attribute_value("target", Some(&"night".to_variant())); menu.append_item(&item);
        let connect = || {
            let c = connection();
            if name == "owner" { let r = c.call_sync(Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName", Some(&(BUS, 0u32).to_variant()), None, gio::DBusCallFlags::NONE, 300, None::<&gio::Cancellable>).unwrap(); assert_eq!(r.get::<(u32,)>(), Some((1,))); }
            c.export_menu_model(MENU, &menu).unwrap(); c.export_action_group(ACTIONS, &group).unwrap(); c
        };
        let mut bus = Some(connect());
        let connection = wayland_client::Connection::from_socket(UnixStream::connect(path).unwrap()).unwrap();
        let mut queue = connection.new_event_queue(); let qh = queue.handle(); let mut state = ProducerWayland::default();
        connection.display().get_registry(&qh, ()); queue.roundtrip(&mut state).unwrap();
        let mut surface = Some(state.compositor.as_ref().unwrap().create_surface(&qh, ()));
        let mut gtk = Some(state.shell.as_ref().unwrap().get_gtk_surface(surface.as_ref().unwrap(), &qh, ()));
        let claim = |gtk: &gtk::client::gtk_surface1::GtkSurface1, owner: &str| { gtk.set_dbus_properties(Some("org.tarot.OwnershipFixture".into()), Some(MENU.into()), None, None, Some(ACTIONS.into()), Some(owner.into())); };
        claim(gtk.as_ref().unwrap(), bus.as_ref().unwrap().unique_name().unwrap().as_str()); surface.as_ref().unwrap().commit(); queue.roundtrip(&mut state).unwrap();
        emit(json!({"event":"ready","owner":bus.as_ref().unwrap().unique_name().map(|s|s.to_string()),"pid":std::process::id(),"uid":unsafe {libc::getuid()},"cgroup":std::fs::read_to_string("/proc/self/cgroup").unwrap()}));
        let (tx, commands) = mpsc::sync_channel(32);
        thread::spawn(move || { for line in std::io::stdin().lock().lines() { if tx.send(serde_json::from_str::<Value>(&line.unwrap()).unwrap()).is_err() { break; } } });
        let deadline = Instant::now()+Duration::from_secs(30);
        loop {
            assert!(Instant::now()<deadline,"producer lifetime bound"); pump(&context);
            match commands.recv_timeout(Duration::from_millis(1)) {
                Ok(command) => {
                    match command["command"].as_str().unwrap() {
                        "claim" => claim(gtk.as_ref().unwrap(), command["owner"].as_str().unwrap()),
                        "disconnect-bus" => { bus.take().unwrap().close_sync(None::<&gio::Cancellable>).unwrap(); }
                        "reconnect-bus" => { bus=Some(connect()); claim(gtk.as_ref().unwrap(), bus.as_ref().unwrap().unique_name().unwrap().as_str()); }
                        "destroy-surface" => { gtk.take().unwrap().release(); surface.take().unwrap().destroy(); }
                        "create-surface" => { surface=Some(state.compositor.as_ref().unwrap().create_surface(&qh, ())); gtk=Some(state.shell.as_ref().unwrap().get_gtk_surface(surface.as_ref().unwrap(), &qh, ())); claim(gtk.as_ref().unwrap(),bus.as_ref().unwrap().unique_name().unwrap().as_str()); surface.as_ref().unwrap().commit(); }
                        "release-menu" => gtk.take().unwrap().release(),
                        "create-menu" => { gtk=Some(state.shell.as_ref().unwrap().get_gtk_surface(surface.as_ref().unwrap(), &qh, ())); claim(gtk.as_ref().unwrap(),bus.as_ref().unwrap().unique_name().unwrap().as_str()); }
                        "counts" => (),
                        "exit" => { emit(json!({"event":"ack"})); break; }
                        other => panic!("unknown command {other}"),
                    }
                    queue.roundtrip(&mut state).unwrap();
                    emit(json!({"event":"ack","owner":bus.as_ref().and_then(|c|c.unique_name()).map(|s|s.to_string()),"activations":activations.get()}));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => (),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        if let Some(c)=bus { c.close_sync(None::<&gio::Cancellable>).unwrap(); }
    }).unwrap();
}
