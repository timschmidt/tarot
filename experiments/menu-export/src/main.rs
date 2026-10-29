// SPDX-License-Identifier: MIT OR Apache-2.0
// Isolated host feasibility experiment; never part of Tarot's production build.
use gio::{
    glib::{self, variant::ToVariant},
    prelude::*,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

const BUS: &str = "org.tarot.MenuExportSpike";
const MENU: &str = "/org/tarot/MenuExportSpike/menu";
const ACTIONS: &str = "/org/tarot/MenuExportSpike/actions";
enum Command {
    Mutate,
    Pause,
    Stop,
}
struct Producer {
    owner: String,
    commands: mpsc::Sender<Command>,
    events: mpsc::Receiver<String>,
    thread: Option<thread::JoinHandle<()>>,
}

fn connection() -> gio::DBusConnection {
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS").expect("Use dbus-run-session");
    let c = gio::DBusConnection::for_address_sync(
        &address,
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
fn until(context: &glib::MainContext, mut ready: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        pump(context);
        if ready() {
            return;
        }
        assert!(Instant::now() < end, "Private-bus experiment deadline");
        thread::sleep(Duration::from_millis(1));
    }
}
impl Producer {
    fn start() -> Self {
        let (commands, receive) = mpsc::channel();
        let (events, observed) = mpsc::sync_channel(64);
        let (ready, owner) = mpsc::sync_channel(1);
        let worker = thread::spawn(move || {
            let context = glib::MainContext::new();
            context
                .with_thread_default(|| {
                    let c = connection();
                    let reply = c
                        .call_sync(
                            Some("org.freedesktop.DBus"),
                            "/org/freedesktop/DBus",
                            "org.freedesktop.DBus",
                            "RequestName",
                            Some(&(BUS, 0u32).to_variant()),
                            None,
                            gio::DBusCallFlags::NONE,
                            500,
                            None::<&gio::Cancellable>,
                        )
                        .unwrap();
                    assert_eq!(reply.get::<(u32,)>(), Some((1,)));
                    let group = gio::SimpleActionGroup::new();
                    let toggle =
                        gio::SimpleAction::new_stateful("toggle", None, &false.to_variant());
                    let tx = events.clone();
                    toggle.connect_activate(move |a, _| {
                        let value = !a.state().unwrap().get::<bool>().unwrap();
                        a.set_state(&value.to_variant());
                        tx.send(format!("toggle:{value}")).unwrap();
                    });
                    let choose = gio::SimpleAction::new("choose", Some(glib::VariantTy::STRING));
                    let tx = events.clone();
                    choose.connect_activate(move |_, p| {
                        tx.send(format!("choose:{}", p.unwrap().get::<String>().unwrap()))
                            .unwrap()
                    });
                    let disabled = gio::SimpleAction::new("disabled", None);
                    disabled.set_enabled(false);
                    let tx = events.clone();
                    disabled.connect_activate(move |_, _| tx.send("disabled".into()).unwrap());
                    for a in [&toggle, &choose, &disabled] {
                        group.add_action(a);
                    }
                    let menu = gio::Menu::new();
                    let sub = gio::Menu::new();
                    sub.append(Some("Toggle"), Some("app.toggle"));
                    let item = gio::MenuItem::new(Some("Choose night"), Some("app.choose"));
                    item.set_attribute_value("target", Some(&"night".to_variant()));
                    sub.append_item(&item);
                    sub.append(Some("Disabled"), Some("app.disabled"));
                    menu.append_submenu(Some("Preferences"), &sub);
                    let menu_id = c.export_menu_model(MENU, &menu).unwrap();
                    let action_id = c.export_action_group(ACTIONS, &group).unwrap();
                    ready.send(c.unique_name().unwrap().to_string()).unwrap();
                    let end = Instant::now() + Duration::from_secs(30);
                    'worker: loop {
                        while let Ok(command) = receive.try_recv() {
                            match command {
                                Command::Stop => break 'worker,
                                Command::Mutate => {
                                    disabled.set_enabled(true);
                                    menu.append(Some("New item"), Some("app.toggle"));
                                    events.send("mutated".into()).unwrap();
                                }
                                Command::Pause => {
                                    events.send("paused".into()).unwrap();
                                    thread::sleep(Duration::from_millis(350));
                                }
                            }
                        }
                        assert!(Instant::now() < end, "Exporter lifetime bound");
                        pump(&context);
                        thread::sleep(Duration::from_millis(1));
                    }
                    c.unexport_menu_model(menu_id);
                    c.unexport_action_group(action_id);
                    c.close_sync(None::<&gio::Cancellable>).unwrap();
                })
                .unwrap();
        });
        Self {
            owner: owner.recv_timeout(Duration::from_secs(3)).unwrap(),
            commands,
            events: observed,
            thread: Some(worker),
        }
    }
    fn stop(mut self) {
        self.commands.send(Command::Stop).unwrap();
        self.thread.take().unwrap().join().unwrap();
    }
}
impl Drop for Producer {
    fn drop(&mut self) {
        if let Some(worker) = self.thread.take() {
            let _ = self.commands.send(Command::Stop);
            let _ = worker.join();
        }
    }
}
fn explicit_activate(
    c: &gio::DBusConnection,
    owner: &str,
    value: glib::Variant,
    timeout: i32,
) -> Result<glib::Variant, glib::Error> {
    let parameters = (
        "choose",
        vec![value],
        HashMap::<String, glib::Variant>::new(),
    )
        .to_variant();
    c.call_sync(
        Some(owner),
        ACTIONS,
        "org.gtk.Actions",
        "Activate",
        Some(&parameters),
        None,
        gio::DBusCallFlags::NO_AUTO_START,
        timeout,
        None::<&gio::Cancellable>,
    )
}
fn main() {
    let context = glib::MainContext::new();
    let report = context.with_thread_default(|| {
        let exporter = Producer::start();
        let c = connection();
        assert_ne!(c.unique_name().unwrap().as_str(), exporter.owner);
        let owner_changes = Rc::new(RefCell::new(Vec::<(String, String, String)>::new()));
        let observed = owner_changes.clone();
        let _subscription = c.subscribe_to_signal(Some("org.freedesktop.DBus"),
            Some("org.freedesktop.DBus"), Some("NameOwnerChanged"), Some("/org/freedesktop/DBus"),
            Some(BUS), gio::DBusSignalFlags::NONE,
            move |signal| observed.borrow_mut().push(signal.parameters.get().unwrap()));
        let model = gio::DBusMenuModel::get(&c, Some(&exporter.owner), MENU);
        let actions = gio::DBusActionGroup::get(&c, Some(&exporter.owner), ACTIONS);
        let additions = Rc::new(Cell::new(0));
        let observed = additions.clone();
        actions.connect_action_added(None, move |_, _| observed.set(observed.get()+1));
        let enabled_changes = Rc::new(Cell::new(0));
        let observed = enabled_changes.clone();
        actions.connect_action_enabled_changed(None, move |_, _, _| observed.set(observed.get()+1));
        let menu_changes = Rc::new(Cell::new(0));
        let observed = menu_changes.clone();
        model.connect_items_changed(move |_, _, _, _| observed.set(observed.get()+1));
        let initial_actions = actions.list_actions().len();
        let initial_items = model.n_items();
        until(&context, || actions.list_actions().len()==3 && model.n_items()==1);
        let submenu = model.item_link(0, "submenu").unwrap();
        until(&context, || submenu.n_items()==3);
        assert_eq!(submenu.item_attribute_value(1,"target",None).unwrap().get::<String>().as_deref(),Some("night"));
        assert_eq!(actions.action_parameter_type("choose").unwrap().as_str(), "s");
        assert_eq!(actions.action_state("toggle").unwrap().get::<bool>(),Some(false));
        assert!(!actions.is_action_enabled("disabled"));
        assert!(additions.get()>=3);
        actions.activate_action("disabled",None);
        actions.activate_action("toggle",None);
        let mut events = Vec::new();
        until(&context, || {
            events.extend(exporter.events.try_iter());
            actions.action_state("toggle").and_then(|s|s.get::<bool>())==Some(true)
                && events.iter().any(|s|s=="toggle:true")
        });
        let reply = explicit_activate(&c,&exporter.owner,"night".to_variant(),500).unwrap();
        assert_eq!(reply.type_().as_str(),"()");
        until(&context, || { events.extend(exporter.events.try_iter());events.iter().any(|s|s=="choose:night") });
        assert!(!events.iter().any(|s|s=="disabled"));
        let bad_type = explicit_activate(&c,&exporter.owner,42i32.to_variant(),500);
        assert!(bad_type.is_err());
        exporter.commands.send(Command::Mutate).unwrap();
        until(&context, || model.n_items()==2 && actions.is_action_enabled("disabled"));
        assert!(enabled_changes.get()>=1 && menu_changes.get()>=2);
        exporter.commands.send(Command::Pause).unwrap();
        until(&context, || { events.extend(exporter.events.try_iter());events.iter().any(|s|s=="paused") });
        let started = Instant::now();
        let timeout = explicit_activate(&c,&exporter.owner,"late".to_variant(),100);
        let elapsed = started.elapsed();
        assert!(timeout.is_err() && elapsed<Duration::from_secs(1));
        assert!(timeout.as_ref().unwrap_err().matches(gio::IOErrorEnum::TimedOut));
        until(&context, || { events.extend(exporter.events.try_iter());events.iter().any(|s|s=="choose:late") });
        let old_owner = exporter.owner.clone();
        exporter.stop();
        until(&context, || owner_changes.borrow().iter().any(|(_,old,new)|old==&old_owner && new.is_empty()));
        assert!(explicit_activate(&c,&old_owner,"stale".to_variant(),100).is_err());
        let replacement = Producer::start();
        assert_ne!(old_owner,replacement.owner);
        until(&context, || owner_changes.borrow().iter().any(|(_,_,new)|new==&replacement.owner));
        let fresh = gio::DBusActionGroup::get(&c,Some(&replacement.owner),ACTIONS);
        until(&context, || fresh.list_actions().len()==3);
        assert_eq!(fresh.action_state("toggle").unwrap().get::<bool>(),Some(false));
        replacement.stop();
        c.close_sync(None::<&gio::Cancellable>).unwrap();
        serde_json::json!({"passed":true,"scope":"isolated host real GIO exports and proxies; no Wayland binding or production integration",
            "gio_binding":"0.21.5","initial_action_count":initial_actions,"initial_menu_count":initial_items,
            "different_dbus_connections":true,"owned_main_contexts_on_separate_threads":true,
            "submenu_and_typed_target_observed":true,"boolean_state_observed_after_activation":true,
            "disabled_handler_not_activated":true,"typed_argument_received":"night","wrong_type_rejected":true,
            "menu_mutation_observed":true,"enabled_state_mutation_observed":true,
            "explicit_timeout_ms":100,"observed_timeout_elapsed_ms":elapsed.as_millis(),
            "transport_timeout_error_confirmed":true,
            "action_executed_after_transport_timeout":true,"old_unique_owner_call_rejected":true,
            "replacement_owner_is_fresh":true,"application_completion_inferred_from_method_reply":false,
            "wayland_client_credentials_verified":false,"phone_build_verified":false})
    }).unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
