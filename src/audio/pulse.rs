//! Narrow ownership wrapper for libpulse's asynchronous control API.
//!
//! Safety invariant: only this worker owns the loop/context. Every access after
//! starting the loop, including operation cancellation/unref and callback-data
//! destruction, holds the threaded-mainloop lock. Callbacks run with that same
//! lock, borrow library data only for their call, and copy what they retain.
//! Each request owns a stable Box until its callback completes or cancellation
//! prevents all future callbacks. Context callbacks are removed before their
//! boxed data is freed; the context is disconnected/unref'd before loop stop.
use super::Shared;
use anyhow::{Result, anyhow, bail, ensure};
use libpulse_sys as pa;
use std::{
    collections::HashSet,
    ffi::{CStr, CString, c_char, c_void},
    ptr,
    sync::{Arc, atomic::Ordering, mpsc},
    sync::{Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(super) struct Device {
    pub index: u32,
    pub name: CString,
    pub label: String,
    pub volume: pa::pa_cvolume,
    pub muted: bool,
    pub input: bool,
    pub card: Option<u32>,
    pub ports: Vec<Port>,
}
impl Device {
    pub fn percent(&self) -> u32 {
        let max = self.volume.values[..usize::from(self.volume.channels)]
            .iter()
            .copied()
            .max()
            .unwrap_or(0);
        ((u64::from(max) * 100 + u64::from(pa::PA_VOLUME_NORM) / 2) / u64::from(pa::PA_VOLUME_NORM))
            as u32
    }
}
#[derive(Clone)]
pub(super) struct Port {
    pub name: CString,
    pub label: String,
    pub available: tarot_core::audio::Availability,
    pub active: bool,
}
#[derive(Clone)]
pub(super) struct CardProfile {
    pub name: CString,
    pub label: String,
    pub available: bool,
    pub active: bool,
    pub outputs: u32,
    pub inputs: u32,
}
#[derive(Clone)]
pub(super) struct Card {
    pub index: u32,
    pub name: CString,
    pub label: String,
    pub profiles: Vec<CardProfile>,
}
#[derive(Default)]
struct Removed {
    devices: HashSet<(bool, u32)>,
    cards: HashSet<u32>,
}
impl Removed {
    fn is_empty(&self) -> bool {
        self.devices.is_empty() && self.cards.is_empty()
    }
    fn extend(&mut self, other: Self) {
        self.devices.extend(other.devices);
        self.cards.extend(other.cards);
    }
}
#[derive(Default)]
pub(super) struct Defaults {
    pub output: Option<CString>,
    pub input: Option<CString>,
}
pub(super) struct Inventory {
    pub outputs: Vec<Device>,
    pub inputs: Vec<Device>,
    pub cards: Vec<Card>,
    pub activity: tarot_core::audio::Activity,
    pub defaults: Defaults,
    pub removed: HashSet<(bool, u32)>,
    pub removed_cards: HashSet<u32>,
}
struct Signals {
    shared: Arc<Shared>,
    state: mpsc::SyncSender<()>,
    removed: Mutex<Removed>,
    overflow: AtomicBool,
}
struct Reply<T> {
    sender: mpsc::SyncSender<Result<T>>,
    value: T,
}

// The guard never outlives its Connection and serializes all API access with callbacks.
struct Lock(*mut pa::pa_threaded_mainloop);
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            pa::pa_threaded_mainloop_unlock(self.0);
        }
    }
}
pub(super) struct Connection {
    mainloop: *mut pa::pa_threaded_mainloop,
    context: *mut pa::pa_context,
    signals: Box<Signals>,
    started: bool,
}
impl Connection {
    fn lock(&self) -> Lock {
        // SAFETY: self owns a live loop until Drop finishes; callers retain self.
        unsafe {
            pa::pa_threaded_mainloop_lock(self.mainloop);
        }
        Lock(self.mainloop)
    }
    pub fn connect(shared: Arc<Shared>, server: Option<&str>) -> Result<Self> {
        let server = server
            .map(CString::new)
            .transpose()
            .map_err(|_| anyhow!("Invalid audio server"))?;
        let (tx, rx) = mpsc::sync_channel(8);
        // SAFETY: allocations are checked and transferred to Connection immediately.
        let mut this = unsafe {
            let mainloop = pa::pa_threaded_mainloop_new();
            ensure!(!mainloop.is_null(), "Cannot allocate audio loop");
            let context = pa::pa_context_new(
                pa::pa_threaded_mainloop_get_api(mainloop),
                c"Tarot audio controls".as_ptr(),
            );
            if context.is_null() {
                pa::pa_threaded_mainloop_free(mainloop);
                bail!("Cannot allocate audio connection");
            }
            Self {
                mainloop,
                context,
                signals: Box::new(Signals {
                    shared,
                    state: tx,
                    removed: Mutex::new(Removed::default()),
                    overflow: AtomicBool::new(false),
                }),
                started: false,
            }
        };
        // SAFETY: callbacks point to this stable Box. The event thread is not running yet.
        unsafe {
            let data = ptr::from_mut(&mut *this.signals).cast();
            pa::pa_context_set_state_callback(this.context, Some(state_changed), data);
            pa::pa_context_set_subscribe_callback(this.context, Some(subscription), data);
            ensure!(
                pa::pa_threaded_mainloop_start(this.mainloop) == 0,
                "Cannot start audio loop"
            );
            this.started = true;
        }
        {
            let _lock = this.lock();
            // SAFETY: context/loop are live and locked; the CString lives through the call.
            unsafe {
                ensure!(
                    pa::pa_context_connect(
                        this.context,
                        server.as_ref().map_or(ptr::null(), |s| s.as_ptr()),
                        pa::PA_CONTEXT_NOAUTOSPAWN,
                        ptr::null()
                    ) == 0,
                    "Audio service unavailable"
                );
            }
        }
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            ensure!(
                !this.signals.shared.stop.load(Ordering::Acquire),
                "Audio controls stopped"
            );
            let state = {
                let _lock = this.lock();
                unsafe { pa::pa_context_get_state(this.context) }
            };
            match state {
                pa::pa_context_state_t::Ready => break,
                pa::pa_context_state_t::Failed | pa::pa_context_state_t::Terminated => {
                    bail!("Audio service unavailable")
                }
                _ => {}
            }
            ensure!(Instant::now() < deadline, "Audio connection timed out");
            let _ = rx.recv_timeout(Duration::from_millis(100));
        }
        this.request((), |c, data| unsafe {
            pa::pa_context_subscribe(
                c,
                pa::PA_SUBSCRIPTION_MASK_SINK
                    | pa::PA_SUBSCRIPTION_MASK_SOURCE
                    | pa::PA_SUBSCRIPTION_MASK_SINK_INPUT
                    | pa::PA_SUBSCRIPTION_MASK_SOURCE_OUTPUT
                    | pa::PA_SUBSCRIPTION_MASK_CARD
                    | pa::PA_SUBSCRIPTION_MASK_SERVER,
                Some(success),
                data,
            )
        })?;
        Ok(this)
    }
    pub fn ready(&self) -> bool {
        let _lock = self.lock();
        // SAFETY: context is live and locked.
        unsafe { pa::pa_context_get_state(self.context) == pa::pa_context_state_t::Ready }
    }
    fn request<T>(
        &self,
        value: T,
        start: impl FnOnce(*mut pa::pa_context, *mut c_void) -> *mut pa::pa_operation,
    ) -> Result<T> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut reply = Box::new(Reply { sender, value });
        let operation = {
            let _lock = self.lock();
            unsafe {
                ensure!(
                    pa::pa_context_get_state(self.context) == pa::pa_context_state_t::Ready,
                    "Audio service disconnected"
                );
            }
            start(self.context, ptr::from_mut(&mut *reply).cast())
        };
        ensure!(!operation.is_null(), "Audio request could not be started");
        let deadline = Instant::now() + Duration::from_secs(3);
        let result = loop {
            if self.signals.shared.stop.load(Ordering::Acquire) {
                break Err(anyhow!("Audio controls stopped"));
            }
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(result) => break result,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break Err(anyhow!("Audio request disconnected"));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if !self.ready() {
                break Err(anyhow!("Audio service disconnected"));
            }
            if Instant::now() >= deadline {
                break Err(anyhow!("Audio request timed out"));
            }
        };
        // SAFETY: cancelling under the loop lock prevents further calls using
        // reply, even on timeout/disconnect. Drop userdata before releasing lock.
        let _lock = self.lock();
        unsafe {
            if pa::pa_operation_get_state(operation) == pa::pa_operation_state_t::Running {
                pa::pa_operation_cancel(operation);
            }
            pa::pa_operation_unref(operation);
        }
        drop(reply);
        result
    }
    fn take_removed(&self) -> Result<Removed> {
        let _lock = self.lock();
        ensure!(
            !self.signals.overflow.load(Ordering::Acquire),
            "Audio device changes exceed limits"
        );
        Ok(std::mem::take(&mut *self.signals.removed.lock().unwrap()))
    }
    pub fn snapshot(&self) -> Result<Inventory> {
        let mut removed = self.take_removed()?;
        // A removal can arrive between the server/sink/source replies. Retry a
        // bounded number of times and retire every observed old identity before
        // allowing any control dispatch from this inventory.
        for _ in 0..3 {
            let defaults = self.request(Defaults::default(), |c, data| unsafe {
                pa::pa_context_get_server_info(c, Some(server_info), data)
            })?;
            let outputs = self.request(Vec::new(), |c, data| unsafe {
                pa::pa_context_get_sink_info_list(c, Some(sink_info), data)
            })?;
            let inputs = self.request(Vec::new(), |c, data| unsafe {
                pa::pa_context_get_source_info_list(c, Some(source_info), data)
            })?;
            let cards = self.request(Vec::new(), |c, data| unsafe {
                pa::pa_context_get_card_info_list(c, Some(card_info), data)
            })?;
            let playback = self.request(StreamCount::default(), |c, data| unsafe {
                pa::pa_context_get_sink_input_info_list(c, Some(playback_info), data)
            })?;
            let recording = self.request(StreamCount::default(), |c, data| unsafe {
                pa::pa_context_get_source_output_info_list(c, Some(recording_info), data)
            })?;
            let changes = self.take_removed()?;
            if changes.is_empty() {
                return Ok(Inventory {
                    outputs,
                    inputs,
                    cards,
                    activity: tarot_core::audio::Activity {
                        playback: playback.active,
                        recording: recording.active,
                    },
                    defaults,
                    removed: removed.devices,
                    removed_cards: removed.cards,
                });
            }
            removed.extend(changes);
            ensure!(
                removed.devices.len() <= 128 && removed.cards.len() <= 64,
                "Audio device changes exceed limits"
            );
        }
        bail!("Audio devices kept changing during query")
    }

    pub fn volume(&self, sink: &Device, percent: u8) -> Result<()> {
        ensure!(percent <= 100, "Volume must be between 0 and 100 percent");
        let mut volume = sink.volume;
        // SAFETY: a validated local copy with 1..=PA_CHANNELS_MAX channels.
        unsafe {
            pa::pa_cvolume_scale(
                &mut volume,
                (u32::from(percent) * pa::PA_VOLUME_NORM + 50) / 100,
            );
        }
        self.request((), |c, data| unsafe {
            let set = if sink.input {
                pa::pa_context_set_source_volume_by_index
            } else {
                pa::pa_context_set_sink_volume_by_index
            };
            set(c, sink.index, &volume, Some(success), data)
        })
    }
    pub fn mute(&self, sink: &Device, muted: bool) -> Result<()> {
        self.request((), |c, data| unsafe {
            let set = if sink.input {
                pa::pa_context_set_source_mute_by_index
            } else {
                pa::pa_context_set_sink_mute_by_index
            };
            set(c, sink.index, i32::from(muted), Some(success), data)
        })
    }
    pub fn default(&self, sink: &Device) -> Result<()> {
        self.request((), |c, data| unsafe {
            let set = if sink.input {
                pa::pa_context_set_default_source
            } else {
                pa::pa_context_set_default_sink
            };
            set(c, sink.name.as_ptr(), Some(success), data)
        })
    }
    pub fn port(&self, device: &Device, port: &CString) -> Result<()> {
        self.request((), |c, data| unsafe {
            let set = if device.input {
                pa::pa_context_set_source_port_by_index
            } else {
                pa::pa_context_set_sink_port_by_index
            };
            set(c, device.index, port.as_ptr(), Some(success), data)
        })
    }
    pub fn profile(&self, card: &Card, profile: &CString) -> Result<()> {
        self.request((), |c, data| unsafe {
            pa::pa_context_set_card_profile_by_index(
                c,
                card.index,
                profile.as_ptr(),
                Some(success),
                data,
            )
        })
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        // SAFETY: callbacks are removed while serialized, and data lives until
        // after context destruction. Stopping is done with no loop lock held.
        unsafe {
            if self.started {
                pa::pa_threaded_mainloop_lock(self.mainloop);
            }
            pa::pa_context_set_state_callback(self.context, None, ptr::null_mut());
            pa::pa_context_set_subscribe_callback(self.context, None, ptr::null_mut());
            pa::pa_context_disconnect(self.context);
            pa::pa_context_unref(self.context);
            if self.started {
                pa::pa_threaded_mainloop_unlock(self.mainloop);
                pa::pa_threaded_mainloop_stop(self.mainloop);
            }
            pa::pa_threaded_mainloop_free(self.mainloop);
        }
    }
}
extern "C" fn state_changed(_: *mut pa::pa_context, data: *mut c_void) {
    // SAFETY: data is the stable Signals Box retained until callbacks are removed.
    let signals = unsafe { &*data.cast::<Signals>() };
    let _ = signals.state.try_send(());
    signals.shared.wake();
}
extern "C" fn subscription(
    _: *mut pa::pa_context,
    event: pa::pa_subscription_event_type_t,
    index: u32,
    data: *mut c_void,
) {
    // SAFETY: same registered Signals lifetime and threaded-loop serialization.
    let signals = unsafe { &*data.cast::<Signals>() };
    if event & pa::PA_SUBSCRIPTION_EVENT_TYPE_MASK == pa::PA_SUBSCRIPTION_EVENT_REMOVE {
        let kind = event & pa::PA_SUBSCRIPTION_EVENT_FACILITY_MASK;
        if kind == pa::PA_SUBSCRIPTION_EVENT_SINK || kind == pa::PA_SUBSCRIPTION_EVENT_SOURCE {
            let mut removed = signals.removed.lock().unwrap();
            if removed.devices.len() < 128 {
                removed
                    .devices
                    .insert((kind == pa::PA_SUBSCRIPTION_EVENT_SOURCE, index));
            } else {
                signals.overflow.store(true, Ordering::Release);
            }
        } else if kind == pa::PA_SUBSCRIPTION_EVENT_CARD {
            let mut removed = signals.removed.lock().unwrap();
            if removed.cards.len() < 64 {
                removed.cards.insert(index);
            } else {
                signals.overflow.store(true, Ordering::Release);
            }
        }
    }
    signals.shared.wake();
}

extern "C" fn success(_: *mut pa::pa_context, ok: i32, data: *mut c_void) {
    // SAFETY: request owns this Reply until completion/cancellation under the lock.
    let reply = unsafe { &*data.cast::<Reply<()>>() };
    let _ = reply.sender.try_send(if ok != 0 {
        Ok(())
    } else {
        Err(anyhow!("Audio change was rejected"))
    });
}
extern "C" fn server_info(
    _: *mut pa::pa_context,
    info: *const pa::pa_server_info,
    data: *mut c_void,
) {
    // SAFETY: info is call-scoped library memory; Reply remains owned by request.
    let reply = unsafe { &*data.cast::<Reply<Defaults>>() };
    let result = if info.is_null() {
        Err(anyhow!("Audio server query failed"))
    } else {
        // SAFETY: both strings are call-scoped libpulse data, copied before return.
        let info = unsafe { &*info };
        Ok(Defaults {
            output: copy_optional_name(info.default_sink_name),
            input: copy_optional_name(info.default_source_name),
        })
    };
    let _ = reply.sender.try_send(result);
}
// The sink/source ABI structs are distinct types. Expand the same bounded copy
// against each actual type instead of assuming layout equivalence or transmuting.
macro_rules! device_callback {
    ($callback:ident, $info:ty, $input:tt) => {
        extern "C" fn $callback(
            _: *mut pa::pa_context,
            info: *const $info,
            eol: i32,
            data: *mut c_void,
        ) {
            // SAFETY: the threaded loop serializes callbacks; request retains this
            // Box until operation completion/cancellation under that loop lock.
            let reply = unsafe { &mut *data.cast::<Reply<Vec<Device>>>() };
            if eol != 0 {
                let result = if eol > 0 {
                    Ok(std::mem::take(&mut reply.value))
                } else {
                    Err(anyhow!("Audio device query failed"))
                };
                let _ = reply.sender.try_send(result);
                return;
            }
            if info.is_null() {
                return;
            }
            let info = unsafe { &*info };
            if is_monitor!($input, info) {
                return;
            }
            if info.name.is_null()
                || info.volume.channels == 0
                || usize::from(info.volume.channels) > info.volume.values.len()
            {
                let _ = reply.sender.try_send(Err(anyhow!("Invalid audio device")));
                return;
            }
            if reply.value.len() >= 64
                || info.n_ports > 64
                || (info.n_ports != 0 && info.ports.is_null())
            {
                let _ = reply
                    .sender
                    .try_send(Err(anyhow!("Audio inventory exceeds limits")));
                return;
            }
            let Some(name) = copy_optional_name(info.name) else {
                let _ = reply
                    .sender
                    .try_send(Err(anyhow!("Invalid audio device name")));
                return;
            };
            let mut ports = Vec::new();
            for index in 0..info.n_ports as usize {
                // SAFETY: libpulse owns the declared array during this callback.
                let ptr = unsafe { *info.ports.add(index) };
                if ptr.is_null() {
                    continue;
                }
                let port = unsafe { &*ptr };
                let Some(port_name) = copy_optional_name(port.name) else {
                    continue;
                };
                if ports.iter().any(|p: &Port| p.name == port_name) {
                    continue;
                }
                ports.push(Port {
                    name: port_name,
                    label: if port.description.is_null() {
                        "Audio route".into()
                    } else {
                        copy_label(port.description)
                    },
                    available: match port.available {
                        1 => tarot_core::audio::Availability::No,
                        2 => tarot_core::audio::Availability::Yes,
                        _ => tarot_core::audio::Availability::Unknown,
                    },
                    active: ptr == info.active_port,
                });
            }
            reply.value.push(Device {
                index: info.index,
                name,
                label: if info.description.is_null() {
                    "Audio device".into()
                } else {
                    copy_label(info.description)
                },
                volume: info.volume,
                muted: info.mute != 0,
                input: $input,
                card: (info.card != pa::PA_INVALID_INDEX).then_some(info.card),
                ports,
            });
        }
    };
}
macro_rules! is_monitor {
    (true, $info:ident) => {
        $info.monitor_of_sink != pa::PA_INVALID_INDEX
    };
    (false, $info:ident) => {
        false
    };
}
device_callback!(sink_info, pa::pa_sink_info, false);
device_callback!(source_info, pa::pa_source_info, true);
extern "C" fn card_info(
    _: *mut pa::pa_context,
    info: *const pa::pa_card_info,
    eol: i32,
    data: *mut c_void,
) {
    // SAFETY: same locked request-owned Reply lifetime as the device callbacks.
    let reply = unsafe { &mut *data.cast::<Reply<Vec<Card>>>() };
    if eol != 0 {
        let result = if eol > 0 {
            Ok(std::mem::take(&mut reply.value))
        } else {
            Err(anyhow!("Audio card query failed"))
        };
        let _ = reply.sender.try_send(result);
        return;
    }
    if info.is_null() {
        return;
    }
    let info = unsafe { &*info };
    if reply.value.len() >= 64 || info.n_profiles > 64 {
        let _ = reply
            .sender
            .try_send(Err(anyhow!("Audio modes exceed limits")));
        return;
    }
    let Some(name) = copy_optional_name(info.name) else {
        let _ = reply
            .sender
            .try_send(Err(anyhow!("Invalid audio card name")));
        return;
    };
    let mut profiles = Vec::new();
    // profiles2 carries availability. A missing modern array is honestly empty;
    // obsolete profiles without that policy are never guessed to be selectable.
    if !info.profiles2.is_null() {
        for index in 0..info.n_profiles as usize {
            let ptr = unsafe { *info.profiles2.add(index) };
            if ptr.is_null() {
                break;
            }
            let profile = unsafe { &*ptr };
            let Some(profile_name) = copy_optional_name(profile.name) else {
                continue;
            };
            if profiles
                .iter()
                .any(|p: &CardProfile| p.name == profile_name)
            {
                continue;
            }
            profiles.push(CardProfile {
                name: profile_name,
                label: if profile.description.is_null() {
                    "Audio mode".into()
                } else {
                    copy_label(profile.description)
                },
                available: profile.available != 0,
                active: ptr == info.active_profile2,
                outputs: profile.n_sinks,
                inputs: profile.n_sources,
            });
        }
    }
    let description = if info.proplist.is_null() {
        ptr::null()
    } else {
        unsafe { pa::pa_proplist_gets(info.proplist, c"device.description".as_ptr()) }
    };
    reply.value.push(Card {
        index: info.index,
        name,
        label: if description.is_null() {
            "Audio device".into()
        } else {
            copy_label(description)
        },
        profiles,
    });
}
#[derive(Default)]
struct StreamCount {
    seen: u32,
    active: u32,
}
macro_rules! stream_callback {
    ($callback:ident, $info:ty) => {
        extern "C" fn $callback(
            _: *mut pa::pa_context,
            info: *const $info,
            eol: i32,
            data: *mut c_void,
        ) {
            // SAFETY: the locked request owns this Reply through completion or
            // cancellation. Only the typed call-scoped corked flag is read.
            let reply = unsafe { &mut *data.cast::<Reply<StreamCount>>() };
            if eol != 0 {
                let result = if eol > 0 {
                    Ok(std::mem::take(&mut reply.value))
                } else {
                    Err(anyhow!("Audio stream query failed"))
                };
                let _ = reply.sender.try_send(result);
            } else if !info.is_null() {
                if reply.value.seen >= 256 {
                    let _ = reply
                        .sender
                        .try_send(Err(anyhow!("Audio streams exceed limits")));
                    return;
                }
                reply.value.seen += 1;
                if unsafe { (*info).corked } == 0 {
                    reply.value.active += 1;
                }
            }
        }
    };
}
stream_callback!(playback_info, pa::pa_sink_input_info);
stream_callback!(recording_info, pa::pa_source_output_info);

fn copy_optional_name(value: *const c_char) -> Option<CString> {
    if value.is_null() {
        return None;
    }
    // SAFETY: call-scoped libpulse NUL-terminated string; retain a bounded copy.
    let value = unsafe { CStr::from_ptr(value) };
    (!value.is_empty() && value.to_bytes().len() <= 1024).then(|| value.to_owned())
}
fn copy_label(value: *const c_char) -> String {
    // SAFETY: called only with a nonnull call-scoped libpulse description string.
    unsafe { CStr::from_ptr(value) }
        .to_string_lossy()
        .chars()
        .filter(|c| !c.is_control())
        .take(96)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_stream_counts_ignore_metadata_and_mute_but_bound_corked_entries_too() {
        macro_rules! check {
            ($callback:ident, $info:ty) => {{
                let (sender, receiver) = mpsc::sync_channel(1);
                let mut reply = Box::new(Reply {
                    sender,
                    value: StreamCount::default(),
                });
                let data = ptr::from_mut(&mut *reply).cast();
                // SAFETY: these C info types contain zero-valid primitives,
                // enums and pointers. No string/sample data is accessed.
                let mut info: $info = unsafe { std::mem::zeroed() };
                info.name = ptr::dangling();
                info.mute = 1;
                $callback(ptr::null_mut(), &info, 0, data);
                info.corked = 1;
                $callback(ptr::null_mut(), &info, 0, data);
                $callback(ptr::null_mut(), ptr::null(), 1, data);
                let result = receiver.recv().unwrap().unwrap();
                assert_eq!((result.seen, result.active), (2, 1));
                for _ in 0..257 {
                    $callback(ptr::null_mut(), &info, 0, data);
                }
                assert!(receiver.recv().unwrap().is_err());
                assert_eq!(reply.value.seen, 256);
            }};
        }
        check!(playback_info, pa::pa_sink_input_info);
        check!(recording_info, pa::pa_source_output_info);
    }

    #[test]
    fn source_callback_copies_ports_and_excludes_playback_monitors() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut reply = Box::new(Reply {
            sender,
            value: Vec::<Device>::new(),
        });
        let data = ptr::from_mut(&mut *reply).cast();
        let name = CString::new("source.test").unwrap();
        let description = CString::new("Microphone\nfixture").unwrap();
        let route = CString::new("route.test").unwrap();
        let route_label = CString::new("Built-in route").unwrap();
        // SAFETY: zero is valid for the primitive/enum fields in these C info
        // types; pointer fields remain null until assigned to live local data.
        let mut info: pa::pa_source_info = unsafe { std::mem::zeroed() };
        let mut port: pa::pa_source_port_info = unsafe { std::mem::zeroed() };
        port.name = route.as_ptr();
        port.description = route_label.as_ptr();
        port.available = 1;
        let mut ports = [&mut port as *mut _];
        info.name = name.as_ptr();
        info.description = description.as_ptr();
        info.volume.channels = 2;
        info.volume.values[0] = pa::PA_VOLUME_NORM;
        info.volume.values[1] = pa::PA_VOLUME_NORM / 2;
        info.monitor_of_sink = 22;
        info.n_ports = 1;
        info.ports = ports.as_mut_ptr();
        info.active_port = ports[0];
        source_info(ptr::null_mut(), &info, 0, data);
        assert!(reply.value.is_empty());
        info.monitor_of_sink = pa::PA_INVALID_INDEX;
        source_info(ptr::null_mut(), &info, 0, data);
        source_info(ptr::null_mut(), ptr::null(), 1, data);
        drop((name, description, route, route_label));
        let devices = receiver.recv().unwrap().unwrap();
        assert_eq!(devices.len(), 1);
        let device = &devices[0];
        assert!(device.input);
        assert_eq!(device.name.to_bytes(), b"source.test");
        assert_eq!(device.label, "Microphonefixture");
        assert_eq!(device.percent(), 100);
        assert_eq!(device.ports[0].name.to_bytes(), b"route.test");
        assert_eq!(device.ports[0].label, "Built-in route");
        assert!(device.ports[0].active);
        assert_eq!(
            device.ports[0].available,
            tarot_core::audio::Availability::No
        );
    }

    #[test]
    fn device_callback_rejects_excessive_or_missing_port_arrays() {
        for count in [1, 65] {
            let (sender, receiver) = mpsc::sync_channel(1);
            let mut reply = Box::new(Reply {
                sender,
                value: Vec::<Device>::new(),
            });
            let data = ptr::from_mut(&mut *reply).cast();
            let name = CString::new("sink.test").unwrap();
            // SAFETY: as above; zero-valued scalar fields and null pointers are
            // representable. The bounds check must reject before any dereference.
            let mut info: pa::pa_sink_info = unsafe { std::mem::zeroed() };
            info.name = name.as_ptr();
            info.volume.channels = 1;
            info.n_ports = count;
            sink_info(ptr::null_mut(), &info, 0, data);
            assert!(receiver.recv().unwrap().is_err());
        }
    }
    #[test]
    fn card_callback_copies_modern_availability_and_active_mode_without_legacy_guessing() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut reply = Box::new(Reply {
            sender,
            value: Vec::<Card>::new(),
        });
        let data = ptr::from_mut(&mut *reply).cast();
        let name = CString::new("card.fixture").unwrap();
        let on = CString::new("hifi").unwrap();
        let off = CString::new("off").unwrap();
        let label = CString::new("Disabled\nroute").unwrap();
        // SAFETY: primitive zero-valued C fields and null pointers are valid;
        // pointers below refer to local objects retained for both callbacks.
        let mut info: pa::pa_card_info = unsafe { std::mem::zeroed() };
        let mut profile: pa::pa_card_profile_info2 = unsafe { std::mem::zeroed() };
        let mut off_profile: pa::pa_card_profile_info2 = unsafe { std::mem::zeroed() };
        profile.name = on.as_ptr();
        profile.description = label.as_ptr();
        profile.available = 0;
        profile.n_sinks = 2;
        profile.n_sources = 1;
        off_profile.name = off.as_ptr();
        off_profile.available = 1;
        let mut profiles = [&mut profile as *mut _, &mut off_profile as *mut _];
        info.name = name.as_ptr();
        info.n_profiles = 2;
        // Modern information absent: report no selectable guessed profiles.
        card_info(ptr::null_mut(), &info, 0, data);
        assert!(reply.value[0].profiles.is_empty());
        info.profiles2 = profiles.as_mut_ptr();
        info.active_profile2 = profiles[1];
        card_info(ptr::null_mut(), &info, 0, data);
        card_info(ptr::null_mut(), ptr::null(), 1, data);
        drop((name, on, off, label));
        let cards = receiver.recv().unwrap().unwrap();
        assert_eq!(cards[1].name.to_bytes(), b"card.fixture");
        assert_eq!(cards[1].profiles[0].label, "Disabledroute");
        assert!(!cards[1].profiles[0].available);
        assert_eq!(cards[1].profiles[0].outputs, 2);
        assert_eq!(cards[1].profiles[0].inputs, 1);
        assert!(cards[1].profiles[1].active && cards[1].profiles[1].available);
        assert_eq!(cards[1].profiles[1].outputs, 0);
    }
    #[test]
    fn card_callback_rejects_excessive_profile_counts_before_reading_array() {
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut reply = Box::new(Reply {
            sender,
            value: Vec::<Card>::new(),
        });
        let data = ptr::from_mut(&mut *reply).cast();
        // SAFETY: representable primitive fields, no pointer is dereferenced.
        let mut info: pa::pa_card_info = unsafe { std::mem::zeroed() };
        info.n_profiles = 65;
        card_info(ptr::null_mut(), &info, 0, data);
        assert!(receiver.recv().unwrap().is_err());
    }
}
