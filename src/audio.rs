//! Event-driven audio controls. Only the worker calls libpulse; UI gets snapshots.
mod pulse;
use crate::{CalloopData, Compositor};
use anyhow::{Result, bail, ensure};
use smithay::reexports::calloop::{
    LoopHandle,
    ping::{Ping, make_ping},
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tarot_core::audio::{Card, CardProfile, Output, Port, Snapshot, UiAction};

enum Event {
    Wake,
    Action {
        action: UiAction,
        default_epoch: Option<u64>,
    },
    Stop,
}
struct Shared {
    pending: Mutex<Option<Snapshot>>,
    latest: Mutex<Snapshot>,
    sender: mpsc::SyncSender<Event>,
    ping: Ping,
    stop: AtomicBool,
    dirty: AtomicBool,
    busy: AtomicBool,
    requested: Mutex<Option<UiAction>>,
}
impl Shared {
    fn finish(&self) {
        *self.requested.lock().unwrap() = None;
        self.busy.store(false, Ordering::Release);
    }
    fn wake(&self) {
        if !self.dirty.swap(true, Ordering::AcqRel) {
            let _ = self.sender.try_send(Event::Wake);
        }
    }
    fn publish(&self, value: Snapshot) {
        self.publish_result(value, false);
    }
    fn publish_result(&self, mut value: Snapshot, force: bool) {
        value.enabled = true;
        value.pending = self.requested.lock().unwrap().clone();
        let mut latest = self.latest.lock().unwrap();
        value.generation = latest.generation;
        if *latest == value && !force {
            return;
        }
        value.generation += 1;
        *latest = value.clone();
        *self.pending.lock().unwrap() = Some(value);
        self.ping.ping();
    }
}
pub struct Server {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn action(&self, action: UiAction) -> Result<()> {
        self.send_action(action, None)
    }
    pub fn default_action(&self, action: UiAction, epoch: u64) -> Result<()> {
        self.send_action(action, Some(epoch))
    }
    pub fn busy(&self) -> bool {
        self.shared.busy.load(Ordering::Acquire)
    }
    fn send_action(&self, action: UiAction, default_epoch: Option<u64>) -> Result<()> {
        ensure!(action.valid(), "Invalid audio control");
        let latest = self.shared.latest.lock().unwrap();
        ensure!(latest.available, "Audio service unavailable");
        ensure!(
            default_epoch.is_none_or(|epoch| !action.is_input()
                && action.port().is_none()
                && action.card_profile().is_none()
                && latest.default_identity() == Some((epoch, action.device()))),
            "Default audio output changed"
        );
        ensure!(
            action.selectable(&latest),
            "Audio device or route is no longer available"
        );
        ensure!(
            !self.shared.busy.swap(true, Ordering::AcqRel),
            "Audio change is still pending"
        );
        *self.shared.requested.lock().unwrap() = Some(action.clone());
        if self
            .shared
            .sender
            .try_send(Event::Action {
                action,
                default_epoch,
            })
            .is_err()
        {
            self.shared.finish();
            bail!("Audio controls are busy or unavailable");
        }
        Ok(())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        let _ = self.shared.sender.try_send(Event::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
    address: Option<String>,
) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let (sender, receiver) = mpsc::sync_channel(64);
    let shared = Arc::new(Shared {
        pending: Mutex::new(None),
        latest: Mutex::new(Snapshot::default()),
        sender,
        ping,
        stop: AtomicBool::new(false),
        dirty: AtomicBool::new(false),
        busy: AtomicBool::new(false),
        requested: Mutex::new(None),
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |(), _, data| {
            if let Some(snapshot) = updates.pending.lock().unwrap().take() {
                let mut snapshot = snapshot;
                snapshot.pending = updates.requested.lock().unwrap().clone();
                data.state.shell.audio = snapshot;
                data.state.ui_dirty = true;
                data.state.redraw = true;
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    state.shell.audio.enabled = true;
    let worker_shared = shared.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-audio".into())
        .spawn(move || supervise(worker_shared, receiver, address))?;
    Ok(Server {
        shared,
        worker: Some(worker),
    })
}
struct Session {
    ids: HashMap<(bool, u32), (std::ffi::CString, u64)>,
    ports: HashMap<(u64, std::ffi::CString), u64>,
    devices: HashMap<u64, pulse::Device>,
    card_ids: HashMap<u32, (std::ffi::CString, u64)>,
    profile_ids: HashMap<(u64, std::ffi::CString), u64>,
    cards: HashMap<u64, pulse::Card>,
    snapshot: Snapshot,
    pending: Option<(UiAction, Instant)>,
}
fn allocate(next: &mut u64) -> Result<u64> {
    *next = next
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("Audio identity exhausted"))?;
    Ok(*next)
}
impl Session {
    fn refresh(&mut self, connection: &pulse::Connection, next_id: &mut u64) -> Result<()> {
        self.update_inventory(connection.snapshot()?, next_id)
    }
    fn update_inventory(&mut self, inventory: pulse::Inventory, next_id: &mut u64) -> Result<()> {
        for index in &inventory.removed_cards {
            self.card_ids.remove(index);
        }
        for key in inventory.removed {
            self.ids.remove(&key);
        }
        // A card can be removed/re-added before its child removal signal is
        // observed. Those children must not retain identities from the old card.
        self.ids.retain(|_, (_, id)| {
            self.devices
                .get(id)
                .is_none_or(|d| d.card.is_none_or(|c| !inventory.removed_cards.contains(&c)))
        });
        let mut card_ids = HashMap::new();
        let mut profile_ids = HashMap::new();
        let mut actual_cards = HashMap::new();
        let mut cards = Vec::new();
        for card in inventory.cards {
            let id = match self.card_ids.get(&card.index) {
                Some((name, id)) if *name == card.name => *id,
                _ => allocate(next_id)?,
            };
            let mut profiles = Vec::new();
            for profile in &card.profiles {
                let key = (id, profile.name.clone());
                let profile_id = match self.profile_ids.get(&key) {
                    Some(id) => *id,
                    None => allocate(next_id)?,
                };
                profiles.push(CardProfile {
                    id: profile_id,
                    label: profile.label.clone(),
                    active: profile.active,
                    available: profile.available,
                    outputs: profile.outputs,
                    inputs: profile.inputs,
                });
                profile_ids.insert(key, profile_id);
            }
            cards.push(Card {
                id,
                label: card.label.clone(),
                profiles,
            });
            card_ids.insert(card.index, (card.name.clone(), id));
            actual_cards.insert(id, card);
        }
        cards.sort_by_key(|c| c.id);
        self.card_ids = card_ids;
        self.profile_ids = profile_ids;
        self.cards = actual_cards;
        self.snapshot.cards = cards;
        let mut ids = HashMap::new();
        let mut port_ids = HashMap::new();
        let mut actual = HashMap::new();
        let mut outputs = Vec::new();
        let mut inputs = Vec::new();
        for device in inventory.outputs.into_iter().chain(inventory.inputs) {
            let key = (device.input, device.index);
            let id = match self.ids.get(&key) {
                Some((name, id)) if *name == device.name => *id,
                _ => allocate(next_id)?,
            };
            let mut ports = Vec::new();
            for port in &device.ports {
                let key = (id, port.name.clone());
                let port_id = match self.ports.get(&key) {
                    Some(id) => *id,
                    None => allocate(next_id)?,
                };
                ports.push(Port {
                    id: port_id,
                    label: port.label.clone(),
                    available: port.available,
                    active: port.active,
                });
                port_ids.insert(key, port_id);
            }
            let default = if device.input {
                &inventory.defaults.input
            } else {
                &inventory.defaults.output
            };
            let row = Output {
                id,
                label: device.label.clone(),
                percent: device.percent(),
                muted: device.muted,
                default: default.as_ref() == Some(&device.name),
                ports,
            };
            if device.input {
                inputs.push(row);
            } else {
                outputs.push(row);
            }
            ids.insert(key, (device.name.clone(), id));
            actual.insert(id, device);
        }
        outputs.sort_by_key(|o| (!o.default, o.id));
        inputs.sort_by_key(|o| (!o.default, o.id));
        self.ids = ids;
        self.ports = port_ids;
        self.devices = actual;
        self.snapshot.outputs = outputs;
        self.snapshot.inputs = inputs;
        self.snapshot.available = true;
        if let Some((action, deadline)) = &self.pending {
            if action.confirmed(&self.snapshot) {
                self.pending = None;
                self.snapshot.action_error = None;
            } else if !action.selectable(&self.snapshot) {
                self.pending = None;
                self.snapshot.action_error = Some("Audio device or route disappeared".into());
            } else if Instant::now() >= *deadline {
                self.pending = None;
                self.snapshot.action_error = Some("Audio change was not confirmed".into());
            }
        }
        self.snapshot.pending = self.pending.as_ref().map(|(a, _)| a.clone());
        Ok(())
    }
    fn apply(&self, connection: &pulse::Connection, action: &UiAction) -> Result<()> {
        ensure!(
            action.selectable(&self.snapshot),
            "Audio device or route is no longer available"
        );
        if let Some((card_id, profile_id)) = action.card_profile() {
            let card = self
                .cards
                .get(&card_id)
                .ok_or_else(|| anyhow::anyhow!("Audio card disappeared"))?;
            let name = self
                .profile_ids
                .iter()
                .find_map(|((id, name), value)| {
                    (*id == card_id && *value == profile_id).then_some(name)
                })
                .ok_or_else(|| anyhow::anyhow!("Audio mode disappeared"))?;
            return connection.profile(card, name);
        }
        let device = self
            .devices
            .get(&action.device())
            .ok_or_else(|| anyhow::anyhow!("Audio device disappeared"))?;
        match *action {
            UiAction::Volume { percent, .. } | UiAction::InputVolume { percent, .. } => {
                connection.volume(device, percent)
            }
            UiAction::Mute { muted, .. } | UiAction::InputMute { muted, .. } => {
                connection.mute(device, muted)
            }
            UiAction::Default { .. } | UiAction::DefaultInput { .. } => connection.default(device),
            UiAction::OutputPort { port, .. } | UiAction::InputPort { port, .. } => {
                let name = self
                    .ports
                    .iter()
                    .find_map(|((id, name), port_id)| {
                        (*id == action.device() && *port_id == port).then_some(name)
                    })
                    .ok_or_else(|| anyhow::anyhow!("Audio route disappeared"))?;
                connection.port(device, name)
            }
            UiAction::CardProfile { .. } => unreachable!("profile handled above"),
        }
    }
}

fn supervise(shared: Arc<Shared>, receiver: mpsc::Receiver<Event>, address: Option<String>) {
    let mut epoch = 0;
    let mut next_id = 0;
    let mut failures = 0u32;
    while !shared.stop.load(Ordering::Acquire) {
        epoch += 1;
        shared.dirty.store(false, Ordering::Release);
        let started = Instant::now();
        let result = run(&shared, &receiver, address.as_deref(), epoch, &mut next_id);
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        shared.finish();
        shared.publish(Snapshot {
            epoch,
            error: Some("Audio service unavailable".into()),
            action_error: result.err().map(|e| e.to_string()),
            ..Default::default()
        });
        if started.elapsed() >= Duration::from_secs(30) {
            failures = 0;
        }
        let deadline = Instant::now() + Duration::from_millis(250 << failures.min(4));
        failures = failures.saturating_add(1);
        while !shared.stop.load(Ordering::Acquire) && Instant::now() < deadline {
            match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Event::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Ok(Event::Action { .. }) => {
                    shared.finish();
                }
                Ok(Event::Wake) | Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
}
fn run(
    shared: &Arc<Shared>,
    receiver: &mpsc::Receiver<Event>,
    address: Option<&str>,
    epoch: u64,
    next_id: &mut u64,
) -> Result<()> {
    let connection = pulse::Connection::connect(shared.clone(), address)?;
    let mut session = Session {
        ids: HashMap::new(),
        devices: HashMap::new(),
        card_ids: HashMap::new(),
        profile_ids: HashMap::new(),
        cards: HashMap::new(),
        ports: HashMap::new(),
        snapshot: Snapshot {
            epoch,
            ..Default::default()
        },
        pending: None,
    };
    session.refresh(&connection, next_id)?;
    shared.publish(session.snapshot.clone());
    loop {
        if shared.stop.load(Ordering::Acquire) {
            return Ok(());
        }
        if shared.dirty.swap(false, Ordering::AcqRel) {
            ensure!(connection.ready(), "Audio service disconnected");
            let was_pending = session.pending.is_some();
            session.refresh(&connection, next_id)?;
            if was_pending && session.pending.is_none() {
                shared.finish();
            }
            shared.publish(session.snapshot.clone());
        }
        let event = if let Some((_, deadline)) = &session.pending {
            receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        } else {
            receiver
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        };
        match event {
            Ok(Event::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Ok(Event::Wake) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => {
                session.refresh(&connection, next_id)?;
                if session.pending.is_none() {
                    shared.finish();
                }
                shared.publish(session.snapshot.clone());
            }
            Ok(Event::Action {
                action,
                default_epoch,
            }) => {
                shared.dirty.store(false, Ordering::Release);
                // Re-query immediately before dispatch: a removed/replaced output
                // must never inherit an action for an earlier snapshot identity.
                session.refresh(&connection, next_id)?;
                let result = if default_epoch.is_some_and(|e| {
                    session.snapshot.default_identity() != Some((e, action.device()))
                }) {
                    Err(anyhow::anyhow!("Default audio output changed"))
                } else {
                    session.apply(&connection, &action)
                };
                match result {
                    Ok(()) => {
                        session.pending = Some((action, Instant::now() + Duration::from_secs(3)));
                        session.snapshot.action_error = None;
                    }
                    Err(error) => {
                        session.snapshot.action_error = Some(error.to_string());
                        session.pending = None;
                    }
                }
                session.refresh(&connection, next_id)?;
                if session.pending.is_none() {
                    shared.finish();
                }
                // An already-current target may leave every server field
                // unchanged. Still retire the compositor's local reservation.
                shared.publish_result(session.snapshot.clone(), true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::HashSet, ffi::CString};
    use tarot_core::audio::Availability;
    fn device(input: bool) -> pulse::Device {
        let mut volume = libpulse_sys::pa_cvolume {
            channels: 1,
            values: [0; 32],
        };
        volume.values[0] = libpulse_sys::PA_VOLUME_NORM;
        pulse::Device {
            index: 7,
            name: CString::new("same.backend.name").unwrap(),
            label: "Device".into(),
            volume,
            muted: false,
            input,
            card: None,
            ports: vec![pulse::Port {
                name: CString::new("route").unwrap(),
                label: "Route".into(),
                available: Availability::Unknown,
                active: true,
            }],
        }
    }
    fn inventory(removed: HashSet<(bool, u32)>) -> pulse::Inventory {
        pulse::Inventory {
            cards: Vec::new(),
            removed_cards: HashSet::new(),
            outputs: vec![device(false)],
            inputs: vec![device(true)],
            defaults: pulse::Defaults {
                input: Some(CString::new("same.backend.name").unwrap()),
                output: None,
            },
            removed,
        }
    }
    #[test]
    fn coalesced_remove_readd_retires_device_and_port_without_changing_other_direction() {
        let mut session = Session {
            ids: HashMap::new(),
            ports: HashMap::new(),
            devices: HashMap::new(),
            card_ids: HashMap::new(),
            profile_ids: HashMap::new(),
            cards: HashMap::new(),
            snapshot: Snapshot::default(),
            pending: None,
        };
        let mut next = 0;
        session
            .update_inventory(inventory(HashSet::new()), &mut next)
            .unwrap();
        let input = session.snapshot.inputs[0].clone();
        let output = session.snapshot.outputs[0].clone();
        assert_ne!(input.id, output.id);
        assert_ne!(input.ports[0].id, output.ports[0].id);
        assert!(input.default && !output.default);
        let old = UiAction::InputPort {
            input: input.id,
            port: input.ports[0].id,
        };
        session.pending = Some((old.clone(), Instant::now() + Duration::from_secs(3)));
        session
            .update_inventory(inventory(HashSet::from([(true, 7)])), &mut next)
            .unwrap();
        assert!(!old.selectable(&session.snapshot));
        assert!(session.pending.is_none() && session.snapshot.action_error.is_some());
        assert_ne!(session.snapshot.inputs[0].id, input.id);
        assert_ne!(session.snapshot.inputs[0].ports[0].id, input.ports[0].id);
        assert_eq!(session.snapshot.outputs[0], output);
        let current = session.snapshot.inputs[0].clone();
        let mut next_inventory = inventory(HashSet::new());
        next_inventory.inputs[0].ports[0].available = Availability::No;
        session.update_inventory(next_inventory, &mut next).unwrap();
        assert_eq!(session.snapshot.inputs[0].id, current.id);
        assert_eq!(session.snapshot.inputs[0].ports[0].id, current.ports[0].id);
        assert!(
            !UiAction::InputPort {
                input: current.id,
                port: current.ports[0].id
            }
            .selectable(&session.snapshot)
        );
    }
    #[test]
    fn card_remove_readd_retires_modes_and_child_devices_even_before_child_signals() {
        let mut session = Session {
            ids: HashMap::new(),
            ports: HashMap::new(),
            devices: HashMap::new(),
            card_ids: HashMap::new(),
            profile_ids: HashMap::new(),
            cards: HashMap::new(),
            snapshot: Snapshot::default(),
            pending: None,
        };
        let mut next = 0;
        let make = |removed_cards| {
            let mut data = inventory(HashSet::new());
            data.outputs[0].card = Some(12);
            data.inputs[0].card = Some(12);
            data.cards = vec![pulse::Card {
                index: 12,
                name: CString::new("card").unwrap(),
                label: "Card".into(),
                profiles: vec![pulse::CardProfile {
                    name: CString::new("hifi").unwrap(),
                    label: "HiFi".into(),
                    available: true,
                    active: true,
                    outputs: 1,
                    inputs: 1,
                }],
            }];
            data.removed_cards = removed_cards;
            data
        };
        session
            .update_inventory(make(HashSet::new()), &mut next)
            .unwrap();
        let old_card = session.snapshot.cards[0].clone();
        let old_input = session.snapshot.inputs[0].clone();
        let old_output = session.snapshot.outputs[0].clone();
        let old_mode = UiAction::CardProfile {
            card: old_card.id,
            profile: old_card.profiles[0].id,
        };
        session.pending = Some((old_mode.clone(), Instant::now() + Duration::from_secs(3)));
        session
            .update_inventory(make(HashSet::from([12])), &mut next)
            .unwrap();
        assert!(!old_mode.selectable(&session.snapshot));
        assert!(session.pending.is_none() && session.snapshot.action_error.is_some());
        assert_ne!(session.snapshot.cards[0].id, old_card.id);
        assert_ne!(
            session.snapshot.cards[0].profiles[0].id,
            old_card.profiles[0].id
        );
        assert_ne!(session.snapshot.inputs[0].id, old_input.id);
        assert_ne!(session.snapshot.outputs[0].id, old_output.id);
        let mut off = make(HashSet::new());
        off.inputs.clear();
        off.outputs.clear();
        off.cards[0].profiles[0].outputs = 0;
        off.cards[0].profiles[0].inputs = 0;
        session.update_inventory(off, &mut next).unwrap();
        assert!(session.snapshot.inputs.is_empty() && session.snapshot.outputs.is_empty());
        let c = &session.snapshot.cards[0];
        assert!(
            UiAction::CardProfile {
                card: c.id,
                profile: c.profiles[0].id
            }
            .confirmed(&session.snapshot)
        );
    }
}
