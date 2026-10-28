//! Only NetworkManager can invoke the agent, and only an explicitly armed
//! connection may prompt. Replies never enter snapshots or a credential cache.
use super::{NM, Shared};
use std::{
    collections::HashMap,
    sync::{Mutex, Weak},
    time::{Duration, Instant},
};
use tarot_core::network::{PasswordKind, Secret, SecretPrompt, ssid_label};
use zbus::{
    message::Header,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
};
pub const PATH: &str = "/org/freedesktop/NetworkManager/SecretAgent";
pub const MANAGER: &str = "/org/freedesktop/NetworkManager/AgentManager";
pub type Settings = HashMap<String, HashMap<String, OwnedValue>>;
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.NetworkManager.SecretAgent")]
pub enum Error {
    UserCanceled(String),
    AgentCanceled(String),
    NoSecrets(String),
    PermissionDenied(String),
    #[zbus(error)]
    ZBus(zbus::Error),
}
type Reply = Result<Secret, Error>;
struct Armed {
    owner: String,
    path: OwnedObjectPath,
    ssid: Vec<u8>,
    kind: PasswordKind,
}
struct Pending {
    prompt: SecretPrompt,
    deadline: Instant,
    sender: async_channel::Sender<Reply>,
}
#[derive(Default)]
struct Data {
    next: u64,
    armed: Option<Armed>,
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
        if let Some(shared) = self.wake.upgrade() {
            shared.wake();
        }
    }
    pub fn arm(&self, owner: &str, path: &OwnedObjectPath, ssid: &[u8], kind: PasswordKind) {
        self.clear();
        self.data.lock().unwrap().armed = Some(Armed {
            owner: owner.into(),
            path: path.clone(),
            ssid: ssid.to_vec(),
            kind,
        });
    }
    pub fn clear(&self) {
        let mut data = self.data.lock().unwrap();
        let changed = data.armed.is_some() || data.pending.is_some();
        data.armed = None;
        if let Some(pending) = data.pending.take() {
            let _ = pending
                .sender
                .try_send(Err(Error::AgentCanceled("Connection request ended".into())));
        }
        drop(data);
        if changed {
            self.wake();
        }
    }
    pub fn prompt(&self) -> Option<SecretPrompt> {
        self.data
            .lock()
            .unwrap()
            .pending
            .as_ref()
            .map(|p| p.prompt.clone())
    }
    pub fn deadline(&self) -> Option<Instant> {
        self.data
            .lock()
            .unwrap()
            .pending
            .as_ref()
            .map(|p| p.deadline)
    }
    pub fn expire(&self) {
        if self.deadline().is_some_and(|d| Instant::now() >= d) {
            self.clear();
        }
    }
    pub fn respond(&self, id: u64, secret: Option<Secret>) -> anyhow::Result<()> {
        let mut data = self.data.lock().unwrap();
        let pending = data
            .pending
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Password request ended"))?;
        anyhow::ensure!(pending.prompt.id == id, "Password request ended");
        if Instant::now() >= pending.deadline {
            drop(data);
            self.clear();
            anyhow::bail!("Password request expired");
        }
        anyhow::ensure!(
            secret
                .as_ref()
                .is_none_or(|s| pending.prompt.kind.accepts(s)),
            "Invalid password format"
        );
        let pending = data.pending.take().unwrap();
        if secret.is_none() {
            data.armed = None;
        }
        let reply = secret.ok_or_else(|| Error::UserCanceled("Password entry canceled".into()));
        let _ = pending.sender.try_send(reply);
        drop(data);
        self.wake();
        Ok(())
    }
}
pub struct Agent(pub std::sync::Arc<State>);
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_responses_leave_the_prompt_and_expiry_cancels_without_a_cache() {
        let state = State::new(Weak::new());
        let (sender, receiver) = async_channel::bounded(1);
        state.data.lock().unwrap().pending = Some(Pending {
            prompt: SecretPrompt {
                id: 1,
                label: "Test".into(),
                kind: PasswordKind::Psk,
            },
            deadline: Instant::now() + Duration::from_secs(1),
            sender,
        });
        assert!(
            state
                .respond(2, Some(Secret::new("password".into())))
                .is_err()
        );
        assert!(state.prompt().is_some());
        state
            .respond(1, Some(Secret::new("password".into())))
            .unwrap();
        assert!(state.prompt().is_none());
        assert_eq!(receiver.try_recv().unwrap().unwrap().expose(), "password");
        assert!(
            state
                .respond(1, Some(Secret::new("password".into())))
                .is_err()
        );
        let (sender, receiver) = async_channel::bounded(1);
        state.data.lock().unwrap().pending = Some(Pending {
            prompt: SecretPrompt {
                id: 2,
                label: "Test".into(),
                kind: PasswordKind::Psk,
            },
            deadline: Instant::now() - Duration::from_millis(1),
            sender,
        });
        state.expire();
        assert!(state.prompt().is_none());
        assert!(matches!(
            receiver.try_recv().unwrap(),
            Err(Error::AgentCanceled(_))
        ));
    }
}
async fn authorized(connection: &zbus::Connection, header: &Header<'_>) -> Result<String, Error> {
    let owner = zbus::fdo::DBusProxy::new(connection)
        .await?
        .get_name_owner(NM.try_into().unwrap())
        .await
        .map_err(zbus::Error::from)?;
    if header.sender().is_none_or(|s| s.as_str() != owner.as_str()) {
        return Err(Error::PermissionDenied(
            "Only NetworkManager may request credentials".into(),
        ));
    }
    Ok(owner.to_string())
}
#[zbus::interface(name = "org.freedesktop.NetworkManager.SecretAgent")]
impl Agent {
    // The standard method has five wire arguments plus injected bus/header.
    #[allow(clippy::too_many_arguments)]
    async fn get_secrets(
        &self,
        settings: Settings,
        path: OwnedObjectPath,
        setting: &str,
        _hints: Vec<String>,
        flags: u32,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<Settings, Error> {
        let owner = authorized(connection, &header).await?;
        if setting != "802-11-wireless-security" || flags & 1 == 0 {
            return Err(Error::NoSecrets("Interactive personal Wi-Fi only".into()));
        }
        let ssid = settings
            .get("802-11-wireless")
            .and_then(|s| s.get("ssid"))
            .and_then(|s| s.try_clone().ok())
            .and_then(|s| Vec::<u8>::try_from(s).ok())
            .unwrap_or_default();
        let key = settings
            .get(setting)
            .and_then(|s| s.get("key-mgmt"))
            .and_then(|v| <&str>::try_from(v).ok())
            .unwrap_or("");
        let (id, receiver) = {
            let mut data = self.0.data.lock().unwrap();
            let Some(armed) = &data.armed else {
                return Err(Error::NoSecrets("No user connection request".into()));
            };
            if armed.owner != owner
                || armed.path != path
                || armed.ssid != ssid
                || !matches!(
                    (armed.kind, key),
                    (PasswordKind::Psk, "wpa-psk") | (PasswordKind::Sae, "sae")
                )
            {
                return Err(Error::NoSecrets(
                    "Connection is not awaiting this password".into(),
                ));
            }
            if data.pending.is_some() {
                return Err(Error::NoSecrets(
                    "A password request is already open".into(),
                ));
            }
            let kind = armed.kind;
            data.next = data
                .next
                .checked_add(1)
                .ok_or_else(|| Error::NoSecrets("Request limit reached".into()))?;
            let id = data.next;
            let (sender, receiver) = async_channel::bounded(1);
            data.pending = Some(Pending {
                prompt: SecretPrompt {
                    id,
                    label: ssid_label(&ssid),
                    kind,
                },
                deadline: Instant::now() + Duration::from_secs(90),
                sender,
            });
            (id, receiver)
        };
        self.0.wake();
        let secret = receiver
            .recv()
            .await
            .map_err(|_| Error::AgentCanceled("Password request ended".into()))??;
        // Check the live bus owner again after waiting for human input.
        authorized(connection, &header).await?;
        let valid = {
            let data = self.0.data.lock().unwrap();
            data.armed
                .as_ref()
                .is_some_and(|a| a.owner == owner && a.path == path)
                && data.next == id
        };
        if !valid {
            return Err(Error::AgentCanceled("Connection request ended".into()));
        }
        Ok(HashMap::from([(
            setting.into(),
            HashMap::from([
                ("name".into(), Value::from(setting).try_into().unwrap()),
                (
                    "psk".into(),
                    Value::from(secret.expose()).try_into().unwrap(),
                ),
            ]),
        )]))
    }
    async fn cancel_get_secrets(
        &self,
        path: OwnedObjectPath,
        setting: &str,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        authorized(connection, &header).await?;
        if setting == "802-11-wireless-security"
            && self
                .0
                .data
                .lock()
                .unwrap()
                .armed
                .as_ref()
                .is_some_and(|a| a.path == path)
        {
            self.0.clear();
        }
        Ok(())
    }
    async fn save_secrets(
        &self,
        _settings: Settings,
        _path: OwnedObjectPath,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        authorized(connection, &header).await?;
        Err(Error::NoSecrets(
            "Tarot does not store agent-owned passwords".into(),
        ))
    }
    async fn delete_secrets(
        &self,
        _settings: Settings,
        _path: OwnedObjectPath,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        authorized(connection, &header).await?;
        Ok(())
    }
}
