//! Standard BlueZ agent. Only the current daemon can call it; every interactive
//! reply is scoped to a live device token, service epoch and visible pairing UI.
use super::{BLUEZ, DEVICE, SHORT, Shared, label, property, timed};
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex, Weak, atomic::Ordering},
    task::Poll,
    time::{Duration, Instant},
};
use tarot_core::{
    bluetooth::{Answer, Prompt, PromptKind},
    network::Secret,
};
use zbus::{
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue},
};
pub const PATH: &str = "/org/tarot/BluetoothAgent";
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
pub enum Error {
    Rejected(String),
    Canceled(String),
    #[zbus(error)]
    ZBus(zbus::Error),
}
type Reply = Result<Answer, Error>;
struct Pending {
    prompt: Prompt,
    deadline: Instant,
    sender: Option<async_channel::Sender<Reply>>,
}
#[derive(Default)]
struct Data {
    owner: String,
    epoch: u64,
    adapter: String,
    armed: Option<String>,
    incoming: bool,
    next: u64,
    retired: bool,
    pending: Option<Pending>,
}
pub struct State {
    data: Mutex<Data>,
    wake: Weak<Shared>,
}
impl State {
    pub fn new(wake: Weak<Shared>) -> Self {
        Self {
            data: Mutex::new(Data::default()),
            wake,
        }
    }
    fn wake(&self) {
        if let Some(s) = self.wake.upgrade() {
            s.wake();
        }
    }
    fn cancel(data: &mut Data) {
        data.retired = true;
        if let Some(p) = data.pending.take()
            && let Some(sender) = p.sender
        {
            let _ = sender.try_send(Err(Error::Canceled("Pairing request ended".into())));
        }
    }
    pub fn clear(&self) {
        let mut d = self.data.lock().unwrap();
        let changed = d.pending.is_some() || d.armed.is_some() || d.incoming || !d.retired;
        Self::cancel(&mut d);
        d.armed = None;
        d.incoming = false;
        drop(d);
        if changed {
            self.wake();
        }
    }
    pub fn bind(&self, owner: &str, epoch: u64, adapter: &str, incoming: bool) {
        let mut d = self.data.lock().unwrap();
        if d.owner != owner || d.epoch != epoch || d.adapter != adapter {
            Self::cancel(&mut d);
            d.armed = None;
        }
        d.owner = owner.into();
        d.epoch = epoch;
        d.adapter = adapter.into();
        d.incoming = incoming;
    }
    pub fn arm(&self, path: &str) {
        self.data.lock().unwrap().armed = Some(path.into());
    }
    pub fn disarm(&self) {
        let mut d = self.data.lock().unwrap();
        Self::cancel(&mut d);
        d.armed = None;
        drop(d);
        self.wake();
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.data
            .lock()
            .unwrap()
            .pending
            .as_ref()
            .map(|p| p.deadline)
    }
    pub fn expire(&self) -> bool {
        let mut d = self.data.lock().unwrap();
        let expired = d
            .pending
            .as_ref()
            .is_some_and(|p| Instant::now() >= p.deadline);
        if expired {
            Self::cancel(&mut d);
            d.armed = None;
        }
        drop(d);
        if expired {
            self.wake();
        }
        expired
    }
    pub fn prompt(&self) -> Option<Prompt> {
        self.data
            .lock()
            .unwrap()
            .pending
            .as_ref()
            .map(|p| p.prompt.clone())
    }
    pub fn reply(&self, id: u64, answer: Answer) -> anyhow::Result<()> {
        let s = self
            .wake
            .upgrade()
            .ok_or_else(|| anyhow::anyhow!("Bluetooth stopped"))?;
        let mut d = self.data.lock().unwrap();
        let p = d
            .pending
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Pairing request ended"))?;
        anyhow::ensure!(p.prompt.id == id, "Pairing request ended");
        anyhow::ensure!(
            Instant::now() < p.deadline
                && s.open.load(Ordering::Acquire)
                && s.unlocked.load(Ordering::Acquire),
            "Pairing request expired"
        );
        anyhow::ensure!(
            match &answer {
                Answer::Reject => true,
                Answer::Accept => !p.prompt.kind.needs_code() && !p.prompt.kind.display_only(),
                Answer::Code(code) => p.prompt.kind.accepts(code),
            },
            "Invalid pairing response"
        );
        let p = d.pending.take().unwrap();
        if let Some(sender) = p.sender {
            let result = if answer == Answer::Reject {
                Err(Error::Canceled("Pairing canceled".into()))
            } else {
                Ok(answer)
            };
            let _ = sender.try_send(result);
        }
        drop(d);
        self.wake();
        Ok(())
    }
    pub fn removed(&self, path: &str) {
        let mut d = self.data.lock().unwrap();
        if d.armed.as_deref() == Some(path) {
            Self::cancel(&mut d);
            d.armed = None;
        }
        // Token retirement also cancels an incoming request, whose path was not
        // armed by a local Pair operation.
        if let Some(shared) = self.wake.upgrade()
            && d.pending
                .as_ref()
                .is_some_and(|p| !shared.registry.lock().unwrap().has(p.prompt.device))
        {
            Self::cancel(&mut d);
        }
        drop(d);
        self.wake();
    }
}
pub struct Agent(pub Arc<State>);
struct Peer {
    id: u64,
    label: String,
    epoch: u64,
    owner: String,
    path: OwnedObjectPath,
    paired: bool,
    trusted: bool,
    service: Option<String>,
}
async fn daemon_owner(
    connection: &zbus::Connection,
    header: &Header<'_>,
    state: &State,
) -> Result<String, Error> {
    let dbus = zbus::fdo::DBusProxy::new(connection).await?;
    let actual = timed(dbus.get_name_owner(BLUEZ.try_into().unwrap()), SHORT)
        .await
        .map_err(|_| Error::Canceled("Bluetooth service unavailable".into()))?;
    if header
        .sender()
        .is_none_or(|s| s.as_str() != actual.as_str())
        || state.data.lock().unwrap().owner != actual.as_str()
    {
        return Err(Error::Rejected(
            "Only the current Bluetooth service may call the agent".into(),
        ));
    }
    Ok(actual.to_string())
}
impl Agent {
    async fn peer(
        &self,
        device: OwnedObjectPath,
        connection: &zbus::Connection,
        header: &Header<'_>,
    ) -> Result<Peer, Error> {
        let owner = daemon_owner(connection, header, &self.0).await?;
        let proxy = zbus::Proxy::new(
            connection,
            owner.clone(),
            device.clone(),
            "org.freedesktop.DBus.Properties",
        )
        .await?;
        let props: HashMap<String, OwnedValue> = timed(proxy.call("GetAll", &(DEVICE,)), SHORT)
            .await
            .map_err(|_| Error::Canceled("Bluetooth device unavailable".into()))?;
        // GetAll may overlap an owner change. Recheck before any authorization,
        // including the trusted-device path which does not open a prompt.
        daemon_owner(connection, header, &self.0).await?;
        let adapter = property::<OwnedObjectPath>(&props, "Adapter")
            .ok_or_else(|| Error::Rejected("Device has no adapter".into()))?;
        let d = self.0.data.lock().unwrap();
        if d.owner != owner
            || d.adapter != adapter.as_str()
            || property::<bool>(&props, "Blocked") == Some(true)
        {
            return Err(Error::Rejected("Device is no longer available".into()));
        }
        let shared = self
            .0
            .wake
            .upgrade()
            .ok_or_else(|| Error::Canceled("Bluetooth stopped".into()))?;
        let mut registry = shared.registry.lock().unwrap();
        if registry.owner != owner || registry.epoch != d.epoch {
            return Err(Error::Canceled("Bluetooth service changed".into()));
        }
        let id = registry
            .id(device.as_str())
            .map_err(|_| Error::Rejected("Device limit reached".into()))?;
        Ok(Peer {
            id,
            label: label(&props),
            epoch: d.epoch,
            owner,
            path: device,
            paired: property(&props, "Paired").unwrap_or(false),
            trusted: property(&props, "Trusted").unwrap_or(false),
            service: None,
        })
    }
    fn begin(
        &self,
        peer: &Peer,
        kind: PromptKind,
        code: Option<Secret>,
        entered: u16,
        service: Option<String>,
        reply: bool,
    ) -> Result<(u64, Instant, Option<async_channel::Receiver<Reply>>), Error> {
        let s = self
            .0
            .wake
            .upgrade()
            .ok_or_else(|| Error::Canceled("Bluetooth stopped".into()))?;
        if !s.open.load(Ordering::Acquire) || !s.unlocked.load(Ordering::Acquire) {
            return Err(Error::Rejected("Open Bluetooth to pair".into()));
        }
        let mut d = self.0.data.lock().unwrap();
        if d.owner != peer.owner
            || d.epoch != peer.epoch
            || (!d.incoming
                && d.armed.as_deref() != Some(peer.path.as_str())
                && kind != PromptKind::AuthorizeService)
        {
            return Err(Error::Rejected("No active pairing request".into()));
        }
        if let Some(p) = &mut d.pending {
            if !reply
                && p.prompt.device == peer.id
                && p.prompt.kind == kind
                && p.prompt.code == code
            {
                p.prompt.entered = entered.min(6);
                let result = (p.prompt.id, p.deadline, None);
                drop(d);
                self.0.wake();
                return Ok(result);
            }
            return Err(Error::Rejected("Another pairing request is open".into()));
        }
        d.next = d
            .next
            .checked_add(1)
            .ok_or_else(|| Error::Rejected("Pairing request limit reached".into()))?;
        d.retired = false;
        let id = d.next;
        let deadline = Instant::now() + Duration::from_secs(60);
        let (sender, receiver) = async_channel::bounded(1);
        d.pending = Some(Pending {
            prompt: Prompt {
                id,
                device: peer.id,
                label: peer.label.clone(),
                kind,
                code,
                entered: entered.min(6),
                service,
            },
            deadline,
            sender: reply.then_some(sender),
        });
        drop(d);
        self.0.wake();
        Ok((id, deadline, reply.then_some(receiver)))
    }
    async fn request(
        &self,
        peer: Peer,
        kind: PromptKind,
        code: Option<Secret>,
        connection: &zbus::Connection,
        header: &Header<'_>,
    ) -> Result<Answer, Error> {
        let (id, deadline, receiver) =
            self.begin(&peer, kind, code, 0, peer.service.clone(), true)?;
        let receiver = receiver.unwrap();
        let mut receive = std::pin::pin!(receiver.recv());
        let mut timer = std::pin::pin!(async_io::Timer::after(
            deadline.saturating_duration_since(Instant::now())
        ));
        let answer = std::future::poll_fn(|cx| {
            if let Poll::Ready(value) = receive.as_mut().poll(cx) {
                return Poll::Ready(
                    value
                        .map_err(|_| Error::Canceled("Pairing ended".into()))
                        .and_then(|v| v),
                );
            }
            if timer.as_mut().poll(cx).is_ready() {
                return Poll::Ready(Err(Error::Canceled("Pairing request expired".into())));
            }
            Poll::Pending
        })
        .await;
        {
            let mut d = self.0.data.lock().unwrap();
            if d.pending.as_ref().is_some_and(|p| p.prompt.id == id) {
                State::cancel(&mut d);
            }
        }
        self.0.wake();
        let answer = answer?;
        daemon_owner(connection, header, &self.0).await?;
        let s = self
            .0
            .wake
            .upgrade()
            .ok_or_else(|| Error::Canceled("Bluetooth stopped".into()))?;
        let d = self.0.data.lock().unwrap();
        if Instant::now() >= deadline
            || d.retired
            || d.epoch != peer.epoch
            || d.next != id
            || !s.open.load(Ordering::Acquire)
            || !s.unlocked.load(Ordering::Acquire)
            || !s
                .registry
                .lock()
                .unwrap()
                .matches(peer.path.as_str(), peer.id)
        {
            return Err(Error::Canceled("Pairing context changed".into()));
        }
        Ok(answer)
    }
}
#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    async fn release(
        &self,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        daemon_owner(connection, &header, &self.0).await?;
        self.0.clear();
        if let Some(s) = self.0.wake.upgrade() {
            s.agent_released.store(true, Ordering::Release);
        }
        self.0.wake();
        Ok(())
    }
    async fn request_pin_code(
        &self,
        device: OwnedObjectPath,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<String, Error> {
        let peer = self.peer(device, connection, &header).await?;
        match self
            .request(peer, PromptKind::Pin, None, connection, &header)
            .await?
        {
            Answer::Code(code) => Ok(code.expose().into()),
            _ => Err(Error::Rejected("PIN required".into())),
        }
    }
    async fn request_passkey(
        &self,
        device: OwnedObjectPath,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<u32, Error> {
        let peer = self.peer(device, connection, &header).await?;
        match self
            .request(peer, PromptKind::Passkey, None, connection, &header)
            .await?
        {
            Answer::Code(code) => code
                .expose()
                .parse()
                .map_err(|_| Error::Rejected("Passkey required".into())),
            _ => Err(Error::Rejected("Passkey required".into())),
        }
    }
    async fn request_confirmation(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        if passkey > 999999 {
            return Err(Error::Rejected("Invalid passkey".into()));
        }
        let peer = self.peer(device, connection, &header).await?;
        self.request(
            peer,
            PromptKind::Confirm,
            Some(Secret::new(format!("{passkey:06}"))),
            connection,
            &header,
        )
        .await?;
        Ok(())
    }
    async fn display_pin_code(
        &self,
        device: OwnedObjectPath,
        pincode: String,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        let code = Secret::new(pincode);
        if !PromptKind::Pin.accepts(&code) {
            return Err(Error::Rejected("Invalid PIN".into()));
        }
        let peer = self.peer(device, connection, &header).await?;
        self.begin(&peer, PromptKind::DisplayPin, Some(code), 0, None, false)?;
        Ok(())
    }
    async fn display_passkey(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
        entered: u16,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        if passkey > 999999 || entered > 6 {
            return Err(Error::Rejected("Invalid passkey".into()));
        }
        let peer = self.peer(device, connection, &header).await?;
        self.begin(
            &peer,
            PromptKind::DisplayPasskey,
            Some(Secret::new(format!("{passkey:06}"))),
            entered,
            None,
            false,
        )?;
        Ok(())
    }
    async fn request_authorization(
        &self,
        device: OwnedObjectPath,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        let peer = self.peer(device, connection, &header).await?;
        self.request(
            peer,
            PromptKind::AuthorizePairing,
            None,
            connection,
            &header,
        )
        .await?;
        Ok(())
    }
    async fn authorize_service(
        &self,
        device: OwnedObjectPath,
        uuid: String,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        if uuid.len() != 36
            || !uuid.bytes().enumerate().all(|(i, b)| {
                if [8, 13, 18, 23].contains(&i) {
                    b == b'-'
                } else {
                    b.is_ascii_hexdigit()
                }
            })
        {
            return Err(Error::Rejected("Invalid Bluetooth service".into()));
        }
        let mut peer = self.peer(device, connection, &header).await?;
        let uuid = uuid.to_ascii_lowercase();
        peer.service = Some(if uuid.ends_with("-0000-1000-8000-00805f9b34fb") {
            match &uuid[..8] {
                "00001124" | "00001812" => "Keyboard or pointing device".into(),
                "0000110a" | "0000110b" => "Bluetooth audio".into(),
                "0000111e" | "0000111f" | "00001108" => "Hands-free audio".into(),
                _ => format!("Unrecognized service: {uuid}"),
            }
        } else {
            format!("Unrecognized service: {uuid}")
        });
        if peer.paired
            && peer.trusted
            && self
                .0
                .wake
                .upgrade()
                .is_some_and(|s| s.unlocked.load(Ordering::Acquire))
        {
            return Ok(());
        }
        self.request(
            peer,
            PromptKind::AuthorizeService,
            None,
            connection,
            &header,
        )
        .await?;
        Ok(())
    }
    async fn cancel(
        &self,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        daemon_owner(connection, &header, &self.0).await?;
        self.0.disarm();
        Ok(())
    }
}
