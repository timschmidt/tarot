//! Bind physical protection/power observations to the opt-in idle publisher.
use crate::{CalloopData, Compositor};
use anyhow::Result;
use smithay::reexports::calloop::{LoopHandle, ping::make_ping};
use std::sync::{Arc, Mutex};
use tarot_core::session_idle::{Conditions, Publisher, Readiness};

#[derive(Default, serde::Serialize)]
pub struct State {
    pub enabled: bool,
    pub readiness: Readiness,
    pub publisher: Publisher,
    pub allow_external_power: bool,
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
        let power = &self.shell.power.battery;
        let readiness = Conditions {
            enabled: self.session_idle.enabled,
            protected: self.protected(),
            output_off: self.idle.off() && self.idle.output_off,
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
            allow_external_power: self.session_idle.allow_external_power,
        }
        .readiness();
        self.session_idle.readiness = readiness;
        worker.ready(readiness == Readiness::Ready);
    }
}
