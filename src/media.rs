//! MPRIS media players on the session bus. Players are rediscovered after any
//! name or property change; only display text and capabilities are kept, and a
//! control goes to the exact unique name that was observed for that player.
use crate::{CalloopData, Compositor};
use anyhow::{Context, Result, ensure};
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
use tarot_core::media::{self, Action, Control, Player, Snapshot, Status};
use zbus::{
    blocking::{Connection, MessageIterator, connection::Builder},
    zvariant::{OwnedValue, Value},
};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const ROOT: &str = "org.mpris.MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const TIMEOUT: Duration = Duration::from_secs(2);
/// Metadata can carry art URLs and lyrics; a player cannot make Tarot buffer more.
const MAX_REPLY: usize = 64 * 1024;

enum Event {
    Wake,
    Control(Control),
}

struct Shared {
    latest: Mutex<Snapshot>,
    updates: Mutex<Option<Snapshot>>,
    ping: Ping,
    sender: mpsc::SyncSender<Event>,
    wake_queued: AtomicBool,
    lost: AtomicBool,
    stop: AtomicBool,
}

impl Shared {
    fn wake(&self) {
        if !self.wake_queued.swap(true, Ordering::AcqRel) {
            let _ = self.sender.try_send(Event::Wake);
        }
    }
    fn publish(&self, mut value: Snapshot) {
        value.enabled = true;
        let mut latest = self.latest.lock().unwrap();
        value.generation = latest.generation;
        if *latest == value {
            return;
        }
        value.generation = value.generation.saturating_add(1);
        *latest = value.clone();
        *self.updates.lock().unwrap() = Some(value);
        self.ping.ping();
    }
}

pub struct Server {
    shared: Arc<Shared>,
    connection: Arc<Mutex<Option<Connection>>>,
    worker: Option<JoinHandle<()>>,
}

impl Server {
    pub fn control(&self, control: Control) -> bool {
        self.shared.sender.try_send(Event::Control(control)).is_ok()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.shared.wake();
        if let Some(c) = self.connection.lock().unwrap().take() {
            let _ = c.close();
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn start(handle: LoopHandle<'static, CalloopData>, state: &mut Compositor) -> Result<Server> {
    let (ping, source) = make_ping()?;
    let (sender, receiver) = mpsc::sync_channel(16);
    let shared = Arc::new(Shared {
        latest: Mutex::new(Snapshot::default()),
        updates: Mutex::new(None),
        ping,
        sender,
        wake_queued: AtomicBool::new(false),
        lost: AtomicBool::new(false),
        stop: AtomicBool::new(false),
    });
    let updates = shared.clone();
    handle
        .insert_source(source, move |(), _, data| {
            if let Some(value) = updates.updates.lock().unwrap().take() {
                data.state.shell.media = value;
                data.state.ui_dirty = true;
                data.state.redraw = true;
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state.shell.media.enabled = true;
    let connection = Arc::new(Mutex::new(None));
    let saved = connection.clone();
    let worker_shared = shared.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-media".into())
        .spawn(move || {
            while !worker_shared.stop.load(Ordering::Acquire) {
                worker_shared.lost.store(false, Ordering::Release);
                let _ = run(&worker_shared, &receiver, &saved);
                if let Some(c) = saved.lock().unwrap().take() {
                    let _ = c.close();
                }
                if worker_shared.stop.load(Ordering::Acquire) {
                    break;
                }
                worker_shared.publish(Snapshot {
                    error: Some("Media controls unavailable; reconnecting".into()),
                    ..Default::default()
                });
                let until = Instant::now() + Duration::from_secs(2);
                while !worker_shared.stop.load(Ordering::Acquire) && Instant::now() < until {
                    if let Ok(Event::Wake) =
                        receiver.recv_timeout(until.saturating_duration_since(Instant::now()))
                    {
                        worker_shared.wake_queued.store(false, Ordering::Release);
                    }
                }
            }
        })?;
    Ok(Server {
        shared,
        connection,
        worker: Some(worker),
    })
}

/// A known exporter: well-known name, its unique owner, and the shown player.
struct Known {
    name: String,
    owner: String,
    player: Player,
}

fn run(
    shared: &Arc<Shared>,
    receiver: &mpsc::Receiver<Event>,
    saved: &Arc<Mutex<Option<Connection>>>,
) -> Result<()> {
    let connection = Builder::session()?
        .max_queued(128)
        .method_timeout(TIMEOUT)
        .build()?;
    *saved.lock().unwrap() = Some(connection.clone());
    let watcher = watch(shared.clone(), &connection)?;
    let mut known: Vec<Known> = Vec::new();
    let mut next_id = 1u64;
    let mut error = None;
    let result = (|| -> Result<()> {
        loop {
            if shared.stop.load(Ordering::Acquire) {
                return Ok(());
            }
            ensure!(
                !shared.lost.load(Ordering::Acquire),
                "Session bus disconnected"
            );
            known = scan(&connection, known, &mut next_id)?;
            shared.publish(Snapshot {
                available: true,
                players: known.iter().map(|k| k.player.clone()).collect(),
                error: error.take(),
                ..Default::default()
            });
            match receiver.recv()? {
                Event::Wake => shared.wake_queued.store(false, Ordering::Release),
                Event::Control(control) => {
                    let mut busy = shared.latest.lock().unwrap().clone();
                    busy.pending = Some(control);
                    shared.publish(busy);
                    if let Err(e) = send(&connection, &known, control) {
                        tracing::info!(error = %e, "media control not delivered");
                        error = Some("The player did not accept that control".into());
                    }
                }
            }
        }
    })();
    let _ = connection.close();
    let _ = watcher.join();
    result
}

/// Wake on any MPRIS name change or player property change; content is
/// always reread, so signal bodies are never trusted or retained.
fn watch(shared: Arc<Shared>, connection: &Connection) -> Result<JoinHandle<()>> {
    let messages = MessageIterator::from(connection);
    for rule in [
        "type='signal',sender='org.freedesktop.DBus',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0namespace='org.mpris.MediaPlayer2'",
        "type='signal',interface='org.freedesktop.DBus.Properties',member='PropertiesChanged',path='/org/mpris/MediaPlayer2'",
    ] {
        connection.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            "AddMatch",
            &(rule,),
        )?;
    }
    Ok(std::thread::Builder::new()
        .name("media-events".into())
        .spawn(move || {
            for message in messages {
                let Ok(message) = message else {
                    break;
                };
                if message.header().message_type() == zbus::message::Type::Signal {
                    shared.wake();
                }
            }
            shared.lost.store(true, Ordering::Release);
            shared.wake();
        })?)
}

fn checked(reply: zbus::Message, owner: &str) -> Result<zbus::Message> {
    ensure!(
        reply.body().len() <= MAX_REPLY
            && reply.data().fds().is_empty()
            && reply.header().sender().is_some_and(|s| s.as_str() == owner),
        "invalid media player reply"
    );
    Ok(reply)
}

fn bus<B: serde::Serialize + zbus::zvariant::DynamicType>(
    connection: &Connection,
    method: &str,
    body: &B,
) -> Result<zbus::Message> {
    checked(
        connection.call_method(
            Some("org.freedesktop.DBus"),
            "/org/freedesktop/DBus",
            Some("org.freedesktop.DBus"),
            method,
            body,
        )?,
        "org.freedesktop.DBus",
    )
}

fn scan(connection: &Connection, previous: Vec<Known>, next_id: &mut u64) -> Result<Vec<Known>> {
    let names: Vec<String> = bus(connection, "ListNames", &())?.body().deserialize()?;
    let mut names: Vec<String> = names
        .into_iter()
        .filter(|n| n.starts_with(PREFIX) && n.len() <= 255)
        .collect();
    names.sort();
    names.truncate(media::MAX_PLAYERS);
    let mut previous: HashMap<(String, String), (usize, Known)> = previous
        .into_iter()
        .enumerate()
        .map(|(index, k)| ((k.name.clone(), k.owner.clone()), (index, k)))
        .collect();
    let mut changed = Vec::new();
    let mut unchanged = Vec::new();
    for name in names {
        let Ok(reply) = bus(connection, "GetNameOwner", &(name.as_str(),)) else {
            continue;
        };
        let owner: String = reply.body().deserialize()?;
        ensure!(
            owner.starts_with(':') && owner.len() <= 255,
            "invalid owner"
        );
        // A player that fails to answer is skipped, not fatal to the others.
        let Ok(mut player) = read(connection, &owner) else {
            continue;
        };
        match previous.remove(&(name.clone(), owner.clone())) {
            Some((index, old)) => {
                player.id = old.player.id;
                let entry = Known {
                    name,
                    owner,
                    player,
                };
                if entry.player == old.player {
                    unchanged.push((index, entry));
                } else {
                    changed.push(entry);
                }
            }
            None => {
                player.id = *next_id;
                *next_id = next_id.checked_add(1).context("media ids exhausted")?;
                changed.push(Known {
                    name,
                    owner,
                    player,
                });
            }
        }
    }
    // Most recently changed first; unchanged players keep their order.
    unchanged.sort_by_key(|(position, _)| *position);
    changed.extend(unchanged.into_iter().map(|(_, k)| k));
    Ok(changed)
}

fn properties(
    connection: &Connection,
    owner: &str,
    interface: &str,
) -> Result<HashMap<String, OwnedValue>> {
    let reply = checked(
        connection.call_method(Some(owner), PATH, Some(PROPERTIES), "GetAll", &(interface,))?,
        owner,
    )?;
    Ok(reply.body().deserialize()?)
}

fn text(value: &Value) -> Option<String> {
    match value {
        Value::Str(s) => Some(media::clean(s.as_str())),
        Value::Value(inner) => text(inner),
        _ => None,
    }
}

fn flag(map: &HashMap<String, OwnedValue>, key: &str) -> bool {
    matches!(map.get(key).map(|v| &**v), Some(Value::Bool(true)))
}

fn read(connection: &Connection, owner: &str) -> Result<Player> {
    let player = properties(connection, owner, PLAYER)?;
    let root = properties(connection, owner, ROOT).unwrap_or_default();
    let mut out = Player {
        identity: root
            .get("Identity")
            .and_then(|v| text(v))
            .unwrap_or_else(|| "Media player".into()),
        status: match player
            .get("PlaybackStatus")
            .and_then(|v| text(v))
            .as_deref()
        {
            Some("Playing") => Status::Playing,
            Some("Paused") => Status::Paused,
            _ => Status::Stopped,
        },
        can_play: flag(&player, "CanPlay"),
        can_pause: flag(&player, "CanPause"),
        can_go_next: flag(&player, "CanGoNext"),
        can_go_previous: flag(&player, "CanGoPrevious"),
        ..Default::default()
    };
    // CanControl false means no control at all, whatever else is claimed.
    if matches!(
        player.get("CanControl").map(|v| &**v),
        Some(Value::Bool(false))
    ) {
        out.can_play = false;
        out.can_pause = false;
        out.can_go_next = false;
        out.can_go_previous = false;
    }
    if let Some(Value::Dict(metadata)) = player.get("Metadata").map(|v| &**v) {
        for (key, value) in metadata.iter() {
            let (Value::Str(key), Value::Value(value)) = (key, value) else {
                continue;
            };
            match key.as_str() {
                "xesam:title" => out.title = text(value).unwrap_or_default(),
                "xesam:artist" => {
                    if let Value::Array(names) = &**value {
                        let names: Vec<String> = names.iter().filter_map(text).take(4).collect();
                        out.artist = media::clean(&names.join(", "));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(out)
}

/// Deliver to the unique name observed for this player, after confirming that
/// the well-known name still belongs to it.
fn send(connection: &Connection, known: &[Known], control: Control) -> Result<()> {
    let target = known
        .iter()
        .find(|k| k.player.id == control.player)
        .context("player is gone")?;
    let owner: String = bus(connection, "GetNameOwner", &(target.name.as_str(),))?
        .body()
        .deserialize()?;
    ensure!(owner == target.owner, "player changed owner");
    let method = match control.action {
        Action::PlayPause => "PlayPause",
        Action::Next => "Next",
        Action::Previous => "Previous",
    };
    checked(
        connection.call_method(Some(owner.as_str()), PATH, Some(PLAYER), method, &())?,
        &owner,
    )?;
    Ok(())
}
