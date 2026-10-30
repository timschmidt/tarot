//! Standard authentication-agent object; only the enrolled authority may call it.
use super::*;
use zbus::message::Header;
pub const PATH: &str = "/org/tarot/AuthenticationAgent";
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.PolicyKit1.Error")]
pub enum Error {
    Failed(String),
    Cancelled(String),
    #[zbus(error)]
    ZBus(zbus::Error),
}
pub struct Agent(pub Arc<Shared>);
async fn caller(s: &Shared, c: &zbus::Connection, h: &Header<'_>) -> Result<Enrollment, Error> {
    let dbus = zbus::fdo::DBusProxy::new(c).await?;
    let actual = within(dbus.get_name_owner(AUTHORITY.try_into().unwrap()))
        .await
        .map_err(|_| Error::Cancelled("Authorization service unavailable".into()))?;
    let e = s
        .data
        .lock()
        .unwrap()
        .enrollment
        .clone()
        .ok_or_else(|| Error::Cancelled("Agent is not enrolled".into()))?;
    if h.sender().is_none_or(|n| n.as_str() != actual.as_str())
        || actual.as_str() != e.authority
        || s.stop.load(Ordering::Acquire)
    {
        return Err(Error::Failed(
            "Only the enrolled authority may call the agent".into(),
        ));
    }
    Ok(e)
}
async fn admission(s: &Shared, c: &zbus::Connection, h: &Header<'_>) -> Result<Enrollment, Error> {
    let e = caller(s, c, h).await?;
    if s.protected.load(Ordering::Acquire) || !e.active {
        return Err(Error::Cancelled("Session is protected or inactive".into()));
    }
    let dbus = zbus::fdo::DBusProxy::new(c).await?;
    let owner = within(dbus.get_name_owner(LOGIN.try_into().unwrap()))
        .await
        .map_err(|_| Error::Cancelled("Session service unavailable".into()))?;
    if owner.as_str() != e.login {
        return Err(Error::Cancelled("Session service changed".into()));
    }
    let manager = zbus::Proxy::new(
        c,
        e.login.clone(),
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )
    .await?;
    let path: OwnedObjectPath = within(manager.call("GetSessionByPID", &(std::process::id(),)))
        .await
        .map_err(|_| Error::Cancelled("Session unavailable".into()))?;
    let properties = zbus::Proxy::new(
        c,
        e.login.clone(),
        e.session.clone(),
        "org.freedesktop.DBus.Properties",
    )
    .await?;
    let p: Properties = within(properties.call("GetAll", &(SESSION,)))
        .await
        .map_err(|_| Error::Cancelled("Session unavailable".into()))?;
    let uid: Option<(u32, OwnedObjectPath)> = prop(&p, "User");
    if path != e.session
        || prop::<String>(&p, "Id").as_deref() != Some(e.id.as_str())
        || uid.is_none_or(|u| u.0 != unsafe { libc::geteuid() })
        || prop::<bool>(&p, "Active") != Some(true)
        || prop::<bool>(&p, "LockedHint") == Some(true)
        || prop::<bool>(&p, "Remote") != Some(false)
        || prop::<String>(&p, "Type").as_deref() != Some("wayland")
    {
        return Err(Error::Cancelled(
            "Not the active local graphical session".into(),
        ));
    }
    let current = caller(s, c, h).await?;
    if current != e || s.protected.load(Ordering::Acquire) {
        return Err(Error::Cancelled("Authentication context changed".into()));
    }
    Ok(e)
}
#[zbus::interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Agent {
    #[allow(clippy::too_many_arguments)] // Standard D-Bus ABI.
    async fn begin_authentication(
        &self,
        action_id: String,
        message: String,
        icon_name: String,
        details: HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, Properties)>,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        let cookie = Zeroizing::new(cookie);
        let e = admission(&self.0, connection, &header).await?;
        if action_id.is_empty()
            || action_id.len() > 256
            || !action_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            || message.len() > 8192
            || icon_name.len() > 512
            || details.len() > 64
            || details.iter().any(|(k, v)| k.len() > 256 || v.len() > 4096)
            || cookie.is_empty()
            || cookie.len() > 4096
            || cookie.chars().any(char::is_control)
            || identities.is_empty()
            || identities.len() > 16
        {
            return Err(Error::Failed("Unsupported authentication request".into()));
        }
        let mut uids = Vec::new();
        for (kind, values) in identities {
            let uid = prop::<u32>(&values, "uid");
            if kind != "unix-user" || values.len() != 1 || uid.is_none() {
                return Err(Error::Failed("Unsupported authentication identity".into()));
            }
            let uid = uid.unwrap();
            if !uids.contains(&uid) {
                uids.push(uid);
            }
        }
        if self
            .0
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Error::Failed("Another authentication is ending".into()));
        }
        let (answer, receive) = async_channel::bounded(1);
        let (actions, commands) = mpsc::sync_channel(8);
        let cancelled = Arc::new(AtomicBool::new(false));
        let id = {
            let mut d = self.0.data.lock().unwrap();
            if d.current.is_some()
                || d.enrollment.as_ref() != Some(&e)
                || self.0.protected.load(Ordering::Acquire)
            {
                self.0.busy.store(false, Ordering::Release);
                return Err(Error::Cancelled("Authentication context changed".into()));
            }
            let Some(id) = d.next.checked_add(1) else {
                self.0.busy.store(false, Ordering::Release);
                return Err(Error::Failed("Authentication request limit".into()));
            };
            d.next = id;
            let request = Request {
                id,
                action: action_id,
                message: message
                    .chars()
                    .filter(|c| !c.is_control() || *c == '\n')
                    .collect(),
                identities: uids
                    .iter()
                    .map(|uid| Identity {
                        uid: *uid,
                        name: format!("User {uid}"),
                    })
                    .collect(),
                selected: None,
                phase: Phase::Starting,
                info: None,
            };
            d.current = Some(Current {
                request,
                cookie: cookie.clone(),
                epoch: e.epoch,
                cancelled: cancelled.clone(),
                actions,
                answer,
            });
            self.0.publish(&mut d, None);
            id
        };
        let state = self.0.clone();
        if std::thread::Builder::new()
            .name("tarot-authentication".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    authenticate(&state, id, cookie, uids, &cancelled, commands)
                }))
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Authentication worker ended")));
                state.finish(id, result);
                state.busy.store(false, Ordering::Release);
            })
            .is_err()
        {
            self.0.busy.store(false, Ordering::Release);
            self.0.cancel(Some(id));
            return Err(Error::Failed("Authentication worker unavailable".into()));
        }
        let mut receiver = std::pin::pin!(receive.recv());
        let mut deadline = std::pin::pin!(async_io::Timer::after(REQUEST_TIME));
        let result = std::future::poll_fn(|cx| {
            if let Poll::Ready(r) = receiver.as_mut().poll(cx) {
                return Poll::Ready(
                    r.unwrap_or_else(|_| Err(Error::Cancelled("Authentication ended".into()))),
                );
            }
            if deadline.as_mut().poll(cx).is_ready() {
                return Poll::Ready(Err(Error::Cancelled(
                    "Authentication request expired".into(),
                )));
            }
            Poll::Pending
        })
        .await;
        if result.is_err() {
            self.0.cancel(Some(id));
            return result;
        }
        let current = admission(&self.0, connection, &header).await?;
        if current.epoch != e.epoch {
            return Err(Error::Cancelled("Authorization session changed".into()));
        }
        Ok(())
    }
    async fn cancel_authentication(
        &self,
        cookie: String,
        #[zbus(connection)] connection: &zbus::Connection,
        #[zbus(header)] header: Header<'_>,
    ) -> Result<(), Error> {
        let cookie = Zeroizing::new(cookie);
        caller(&self.0, connection, &header).await?;
        let id = self
            .0
            .data
            .lock()
            .unwrap()
            .current
            .as_ref()
            .filter(|current| *current.cookie == *cookie)
            .map(|current| current.request.id);
        if let Some(id) = id {
            self.0.cancel(Some(id));
        }
        Ok(())
    }
}
