//! Keep the explicitly configured input method alive with bounded crash recovery.
use crate::{CalloopData, Compositor, apps, text};
use smithay::reexports::calloop::{
    LoopHandle,
    timer::{TimeoutAction, Timer},
};
use std::{
    ffi::OsString,
    io::Write,
    os::{fd::OwnedFd, unix::net::UnixStream},
    path::PathBuf,
    process::ExitStatus,
    time::{Duration, Instant},
};

#[derive(Default)]
struct Backoff {
    failures: u32,
}
impl Backoff {
    fn failed(&mut self, alive_for: Duration) -> Option<Duration> {
        if alive_for >= Duration::from_secs(30) {
            self.failures = 0;
        }
        self.failures += 1;
        (self.failures <= 5).then(|| Duration::from_millis(250 * (1 << (self.failures - 1))))
    }
}

pub struct ManagedInputMethod {
    program: PathBuf,
    arguments: Vec<OsString>,
    handle: LoopHandle<'static, CalloopData>,
    pub generation: u64,
    pub pid: Option<u32>,
    pidfd: Option<OwnedFd>,
    control: Option<UnixStream>,
    client: Option<smithay::reexports::wayland_server::Client>,
    started: Instant,
    backoff: Backoff,
    pub restart_pending: bool,
    pub error: Option<String>,
}

impl ManagedInputMethod {
    pub fn status(&self) -> serde_json::Value {
        serde_json::json!({"pid":self.pid,"generation":self.generation,"restart_pending":self.restart_pending,"consecutive_failures":self.backoff.failures,"error":self.error})
    }
    pub fn control(&mut self, message: u8) {
        if let Some(control) = &mut self.control {
            let _ = control.write(&[message]); // Nonblocking and bounded; no text is transmitted.
        }
    }
}

impl Drop for ManagedInputMethod {
    fn drop(&mut self) {
        self.control(b'Q');
        if let Some(pidfd) = &self.pidfd {
            let _ = rustix::process::pidfd_send_signal(pidfd, rustix::process::Signal::TERM);
        }
    }
}

pub fn start(
    state: &mut Compositor,
    handle: LoopHandle<'static, CalloopData>,
    program: PathBuf,
    arguments: Vec<OsString>,
) {
    state.shell.keyboard_available = true;
    state.input_method = Some(ManagedInputMethod {
        program,
        arguments,
        handle,
        generation: 0,
        pid: None,
        pidfd: None,
        control: None,
        client: None,
        started: Instant::now(),
        backoff: Backoff::default(),
        restart_pending: false,
        error: None,
    });
    attempt(state);
}

fn attempt(state: &mut Compositor) {
    let Some(mut managed) = state.input_method.take() else {
        return;
    };
    managed.generation = managed.generation.wrapping_add(1);
    managed.restart_pending = false;
    let failure = match text::spawn_input_method(state, &managed.program, &managed.arguments) {
        Ok(spawned) => {
            managed.pid = Some(spawned.child.id());
            managed.pidfd =
                rustix::process::Pid::from_raw(spawned.child.id() as i32).and_then(|pid| {
                    rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty()).ok()
                });
            managed.control = Some(spawned.control);
            managed.client = Some(spawned.client);
            managed.started = Instant::now();
            managed.error = None;
            if let Some(sender) = &state.app_events {
                apps::monitor_child(
                    managed.handle.clone(),
                    spawned.child,
                    apps::ChildTag::InputMethod(managed.generation),
                    sender.clone(),
                );
            }
            None
        }
        Err(error) => Some(error.to_string()),
    };
    state.input_method = Some(managed);
    if let Some(error) = failure {
        failed(state, error, Duration::ZERO);
    }
}

fn failed(state: &mut Compositor, error: String, alive_for: Duration) {
    let Some(managed) = &mut state.input_method else {
        return;
    };
    managed.pid = None;
    managed.pidfd = None;
    managed.control = None;
    if let Some(client) = managed.client.take() {
        state.display_handle.backend_handle().kill_client(
            client.id(),
            smithay::reexports::wayland_server::backend::DisconnectReason::ConnectionClosed,
        );
    }
    managed.error = Some(error.clone());
    let Some(delay) = managed.backoff.failed(alive_for) else {
        tracing::warn!(%error,"input method repeatedly failed; automatic restart stopped");
        return;
    };
    managed.restart_pending = true;
    let generation = managed.generation;
    tracing::warn!(%error, delay_ms=delay.as_millis(),"input method unavailable; restart scheduled");
    if let Err(error) =
        managed
            .handle
            .insert_source(Timer::from_duration(delay), move |_, _, data| {
                if data.state.input_method.as_ref().is_some_and(|managed| {
                    managed.generation == generation && managed.restart_pending
                }) {
                    attempt(&mut data.state);
                }
                TimeoutAction::Drop
            })
    {
        managed.restart_pending = false;
        managed.error = Some(format!("Cannot schedule keyboard restart: {error}"));
    }
}

pub fn exited(state: &mut Compositor, generation: u64, result: Result<ExitStatus, String>) {
    let Some(managed) = &state.input_method else {
        return;
    };
    if managed.generation != generation {
        return;
    }
    let elapsed = managed.started.elapsed();
    let error = match result {
        Ok(status) => format!("Input method exited: {status}"),
        Err(error) => error,
    };
    failed(state, error, elapsed);
}

pub fn toggle(state: &mut Compositor) {
    let visible = state.native_keyboard_visible();
    let Some(managed) = &mut state.input_method else {
        return;
    };
    if managed.pid.is_some() {
        managed.control(if visible { b'H' } else { b'S' });
    } else {
        managed.generation = managed.generation.wrapping_add(1);
        managed.backoff = Backoff::default();
        managed.restart_pending = false;
        attempt(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_failures_back_off_and_stop_until_a_healthy_interval() {
        let mut backoff = Backoff::default();
        for delay in [250, 500, 1000, 2000, 4000] {
            assert_eq!(
                backoff.failed(Duration::ZERO),
                Some(Duration::from_millis(delay))
            );
        }
        assert_eq!(backoff.failed(Duration::ZERO), None);
        assert_eq!(
            backoff.failed(Duration::from_secs(30)),
            Some(Duration::from_millis(250))
        );
    }
}
