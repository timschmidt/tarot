//! Bind physical protection/power observations to the opt-in idle publisher.
use crate::{CalloopData, Compositor};
use anyhow::Result;
use smithay::reexports::calloop::{
    LoopHandle, RegistrationToken,
    ping::make_ping,
    timer::{TimeoutAction, Timer},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tarot_core::session_idle::{Conditions, Publisher, Readiness};

#[derive(Default, serde::Serialize)]
pub struct State {
    pub enabled: bool,
    pub readiness: Readiness,
    pub publisher: Publisher,
    /// Diagnostic launcher override; the persistent setting can also allow it.
    pub allow_external_power: bool,
    /// Off counter and session time when its protected power-off was proven.
    pub proven_off: Option<(u64, Duration)>,
    #[serde(skip)]
    delay_timer: Option<(Duration, RegistrationToken)>,
    #[serde(skip)]
    handle: Option<LoopHandle<'static, CalloopData>>,
}

pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
    bus: Option<String>,
    physical: bool,
    allow_external_power: bool,
) -> Result<tarot::session_idle::Worker> {
    anyhow::ensure!(
        physical || bus.is_some(),
        "Nested idle publishing needs a private bus"
    );
    let (ping, source) = make_ping()?;
    let latest = Arc::new(Mutex::new(None::<Publisher>));
    let updates = latest.clone();
    handle
        .insert_source(source, move |(), _, data| {
            if let Some(value) = updates.lock().unwrap().take() {
                data.state.session_idle.publisher = value;
            }
        })
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let address = bus.unwrap_or_else(|| {
        std::env::var("DBUS_SYSTEM_BUS_ADDRESS")
            .unwrap_or_else(|_| "unix:path=/run/dbus/system_bus_socket".into())
    });
    state.session_idle.handle = Some(handle.clone());
    let worker = tarot::session_idle::start(address, physical, move |value| {
        *latest.lock().unwrap() = Some(value);
        ping.ping();
    })?;
    state.session_idle.enabled = true;
    state.session_idle.allow_external_power = allow_external_power;
    Ok(worker)
}

impl Compositor {
    pub fn session_idle_tick(&mut self) {
        let Some(worker) = &self.session_idle_worker else {
            return;
        };
        // This is a historical proof of the still-disabled panel. A new
        // trusted owner/generation or orientation while off does not expose
        // an ordinary frame and must not invalidate that physical transaction.
        let proof = self.idle.off_presentation.is_some_and(|proof| {
            tarot_core::session_idle::protected_off_proof(
                proof.phase,
                proof.off_count,
                self.idle.off_count,
            )
        });
        let output_off = self.idle.off() && self.idle.output_off;
        let now = self.start_time.elapsed();
        let off_count = self.idle.off_count;
        if !(output_off && proof) {
            self.session_idle.proven_off = None;
        } else if self
            .session_idle
            .proven_off
            .is_none_or(|(count, _)| count != off_count)
        {
            self.session_idle.proven_off = Some((off_count, now));
        }
        let settings = self.shell.display.sleep;
        // The delay restarts with each protected power-off; any wake retires it.
        let due = self.session_idle.proven_off.map(|(_, since)| {
            since.saturating_add(Duration::from_secs(settings.after_off_seconds.into()))
        });
        let power = &self.shell.power.battery;
        let readiness = Conditions {
            enabled: self.session_idle.enabled && settings.automatic,
            protected: self.protected(),
            output_off,
            protected_off_proof: proof,
            seat_active: !self.idle.seat_inactive,
            sleep_armed: self.sleep.enabled
                && self.sleep.available
                && self.sleep.armed()
                && !self.sleep.blocks_unlock()
                && !self.sleep.gate.preparing,
            audio_observed: self.shell.audio.available && self.shell.audio_inhibition.available,
            audio_block_ready: !self.shell.audio.activity.active()
                || self.shell.audio_inhibition.held,
            external_power: (power.available && power.present)
                .then_some(power.external_power)
                .flatten(),
            allow_external_power: self.session_idle.allow_external_power
                || settings.on_external_power,
            delay_elapsed: due.is_some_and(|due| now >= due),
        }
        .readiness();
        self.session_idle.readiness = readiness;
        worker.ready(readiness == Readiness::Ready);
        let display = &mut self.shell.display;
        display.sleep_available = true;
        display.sleep_readiness = readiness;
        display.sleep_manager_action = self.session_idle.publisher.manager_action.clone();
        display.sleep_error = self.session_idle.publisher.error.is_some();
        self.schedule_sleep_delay((readiness == Readiness::Delay).then_some(due).flatten());
    }

    /// Wake the loop when a pending delay expires; tick runs after dispatch.
    fn schedule_sleep_delay(&mut self, due: Option<Duration>) {
        let state = &mut self.session_idle;
        if state.delay_timer.as_ref().map(|(at, _)| *at) == due {
            return;
        }
        let Some(handle) = state.handle.clone() else {
            return;
        };
        if let Some((_, token)) = state.delay_timer.take() {
            handle.remove(token);
        }
        let Some(due) = due else {
            return;
        };
        let wait = due
            .saturating_sub(self.start_time.elapsed())
            .max(Duration::from_millis(1));
        match handle.insert_source(Timer::from_duration(wait), |_, _, data| {
            data.state.session_idle.delay_timer = None;
            TimeoutAction::Drop
        }) {
            Ok(token) => state.delay_timer = Some((due, token)),
            Err(error) => {
                // Readiness stays Delay; a later event retries the timer.
                tracing::warn!(error = %error.error, "scheduling sleep delay");
            }
        }
    }
}
