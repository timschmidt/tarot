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
    ffi::{CStr, CString, c_char, c_void},
    ptr,
    sync::{Arc, atomic::Ordering, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub(super) struct Sink {
    pub index: u32,
    pub name: CString,
    pub label: String,
    pub volume: pa::pa_cvolume,
    pub muted: bool,
}
impl Sink {
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
struct Signals {
    shared: Arc<Shared>,
    state: mpsc::SyncSender<()>,
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
                signals: Box::new(Signals { shared, state: tx }),
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
                pa::PA_SUBSCRIPTION_MASK_SINK | pa::PA_SUBSCRIPTION_MASK_SERVER,
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
    pub fn snapshot(&self) -> Result<(Vec<Sink>, Option<CString>)> {
        let default = self.request(None, |c, data| unsafe {
            pa::pa_context_get_server_info(c, Some(server_info), data)
        })?;
        let sinks = self.request(Vec::new(), |c, data| unsafe {
            pa::pa_context_get_sink_info_list(c, Some(sink_info), data)
        })?;
        Ok((sinks, default))
    }
    pub fn volume(&self, sink: &Sink, percent: u8) -> Result<()> {
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
            pa::pa_context_set_sink_volume_by_name(
                c,
                sink.name.as_ptr(),
                &volume,
                Some(success),
                data,
            )
        })
    }
    pub fn mute(&self, sink: &Sink, muted: bool) -> Result<()> {
        self.request((), |c, data| unsafe {
            pa::pa_context_set_sink_mute_by_name(
                c,
                sink.name.as_ptr(),
                i32::from(muted),
                Some(success),
                data,
            )
        })
    }
    pub fn default(&self, sink: &Sink) -> Result<()> {
        self.request((), |c, data| unsafe {
            pa::pa_context_set_default_sink(c, sink.name.as_ptr(), Some(success), data)
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
    _: pa::pa_subscription_event_type_t,
    _: u32,
    data: *mut c_void,
) {
    // SAFETY: same registered Signals lifetime and threaded-loop serialization.
    unsafe { &*data.cast::<Signals>() }.shared.wake();
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
    let reply = unsafe { &*data.cast::<Reply<Option<CString>>>() };
    let result = if info.is_null() {
        Err(anyhow!("Audio server query failed"))
    } else {
        let name = unsafe { (*info).default_sink_name };
        Ok(if name.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(name) }.to_owned())
        })
    };
    let _ = reply.sender.try_send(result);
}
extern "C" fn sink_info(
    _: *mut pa::pa_context,
    info: *const pa::pa_sink_info,
    eol: i32,
    data: *mut c_void,
) {
    // SAFETY: libpulse serializes list callbacks and provides valid info for eol=0.
    // The worker does not access this Box until completion/cancellation holds lock.
    let reply = unsafe { &mut *data.cast::<Reply<Vec<Sink>>>() };
    if eol != 0 {
        let result = if eol > 0 {
            Ok(std::mem::take(&mut reply.value))
        } else {
            Err(anyhow!("Audio output query failed"))
        };
        let _ = reply.sender.try_send(result);
        return;
    }
    if info.is_null() {
        return;
    }
    let info = unsafe { &*info };
    if info.name.is_null()
        || info.volume.channels == 0
        || usize::from(info.volume.channels) > info.volume.values.len()
    {
        return;
    }
    if reply.value.len() >= 64 {
        let _ = reply
            .sender
            .try_send(Err(anyhow!("Too many audio outputs")));
        return;
    }
    let name = unsafe { CStr::from_ptr(info.name) }.to_owned();
    let label = if info.description.is_null() {
        "Audio output".into()
    } else {
        copy_label(info.description)
    };
    reply.value.push(Sink {
        index: info.index,
        name,
        label,
        volume: info.volume,
        muted: info.mute != 0,
    });
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
