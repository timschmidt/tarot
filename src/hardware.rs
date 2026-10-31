//! Compositor-owned buttons; no hardware or media key reaches a client or credential UI.
use crate::{CalloopData, Compositor};
use smithay::{
    backend::input::{Device, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent},
    reexports::calloop::{
        LoopHandle, RegistrationToken,
        timer::{TimeoutAction, Timer},
    },
};
use std::time::{Duration, Instant};
use tarot_core::{
    audio::KeyQueue,
    hardware::{Action, Buttons, Key},
    idle::Phase,
    media,
};

#[derive(Default)]
pub struct State {
    pub enabled: bool,
    buttons: Buttons,
    audio: KeyQueue,
    volume_identity: Option<(u64, u64)>,
    feedback_until: Option<Duration>,
    timer: Option<(Instant, RegistrationToken)>,
}
impl State {
    pub fn cancel(&mut self) {
        self.buttons.cancel();
        self.audio.cancel();
        self.volume_identity = None;
    }
}
impl Compositor {
    pub fn hardware_input<I: InputBackend>(&mut self, event: &InputEvent<I>) -> bool {
        if !self.hardware.enabled {
            return false;
        }
        if let InputEvent::DeviceRemoved { device } = event {
            self.hardware
                .buttons
                .remove(&device.id(), self.start_time.elapsed());
            return false;
        }
        let InputEvent::Keyboard { event } = event else {
            return false;
        };
        if let Some(action) = media::Action::from_xkb(event.key_code().raw()) {
            // Headset buttons control playback even while locked or dark,
            // without waking the display.
            if event.state() == KeyState::Pressed
                && !self.idle.seat_inactive
                && let Some(control) = self.shell.media.key(action)
            {
                self.media_control(control);
                self.ui_dirty = true;
                self.redraw = true;
            }
            return true;
        }
        let Some(key) = Key::from_xkb(event.key_code().raw()) else {
            return false;
        };
        let action = self.hardware.buttons.event(
            &event.device().id(),
            key,
            event.state() == KeyState::Pressed,
            self.start_time.elapsed(),
        );
        if let Some(action) = action {
            self.hardware_action(action);
        }
        true
    }
    fn hardware_action(&mut self, action: Action) {
        if self.idle.seat_inactive {
            self.hardware.cancel();
            return;
        }
        let now = self.start_time.elapsed();
        if action == Action::TogglePanel {
            let waking = self.idle.policy.as_ref().is_some_and(|p| {
                matches!(p.phase, Phase::Off | Phase::Waking(_)) || p.manual_off_waiting()
            });
            if waking {
                self.idle_system_wake();
            } else {
                if let Err(error) = self.request_lock() {
                    self.fatal_error = Some(error.to_string());
                    self.loop_signal.stop();
                    return;
                }
                if self.shell.display.panel_power_available
                    && let Some(policy) = &mut self.idle.policy
                {
                    policy.request_off(now);
                }
                self.clear_protected_focus();
                self.refresh_lock_focus();
                self.redraw = true;
            }
            self.shell.volume_feedback = None;
            self.hardware.feedback_until = None;
            return;
        }
        // Volume keys remain controls while the display is off. They never
        // wake, type into, or authenticate the protected surface.
        if self
            .idle
            .policy
            .as_ref()
            .is_none_or(|p| p.phase == Phase::Awake)
        {
            self.idle_activity();
        }
        self.hardware.volume_identity = self.shell.audio.default_identity();
        self.hardware.audio.input(action, &self.shell.audio);
        self.hardware.feedback_until = Some(now + Duration::from_secs(2));
        self.ui_dirty = true;
        self.redraw = true;
    }
    pub fn hardware_tick(&mut self) {
        if !self.hardware.enabled {
            return;
        }
        let now = self.start_time.elapsed();
        if self.idle.seat_inactive {
            self.hardware.cancel();
        }
        if self.hardware.volume_identity.is_some()
            && self.hardware.volume_identity != self.shell.audio.default_identity()
        {
            self.hardware.buttons.cancel_volume();
            self.hardware.audio.cancel();
            self.hardware.volume_identity = None;
        }
        if let Some(action) = self.hardware.buttons.repeat(now) {
            self.hardware_action(action);
        }
        if !self.idle.seat_inactive
            && self.audio.as_ref().is_some_and(|s| !s.busy())
            && let Some((epoch, action)) = self.hardware.audio.next(&self.shell.audio)
        {
            match self
                .audio
                .as_ref()
                .unwrap()
                .default_action(action.clone(), epoch)
            {
                Ok(()) => {
                    self.hardware.audio.sent(action.clone());
                    self.shell.audio.pending = Some(action);
                }
                Err(_) => self.hardware.audio.reject(),
            }
        }
        let feedback = if self.hardware.feedback_until.is_some_and(|t| now < t) {
            Some(self.hardware.audio.feedback(&self.shell.audio))
        } else {
            self.hardware.feedback_until = None;
            None
        };
        if self.shell.volume_feedback != feedback {
            self.shell.volume_feedback = feedback;
            self.ui_dirty = true;
            self.redraw = true;
        }
    }
    pub fn schedule_hardware(
        &mut self,
        handle: &LoopHandle<'static, CalloopData>,
    ) -> anyhow::Result<()> {
        if !self.hardware.enabled {
            return Ok(());
        }
        let deadline = self
            .hardware
            .buttons
            .deadline()
            .into_iter()
            .chain(self.hardware.feedback_until)
            .min()
            .and_then(|d| self.start_time.checked_add(d));
        if self.hardware.timer.as_ref().map(|(d, _)| *d) == deadline {
            return Ok(());
        }
        if let Some((_, token)) = self.hardware.timer.take() {
            handle.remove(token);
        }
        if let Some(deadline) = deadline {
            let token = handle
                .insert_source(
                    Timer::from_duration(
                        deadline
                            .saturating_duration_since(Instant::now())
                            .max(Duration::from_millis(1)),
                    ),
                    |_, _, data| {
                        data.state.hardware.timer = None;
                        data.state.hardware_tick();
                        TimeoutAction::Drop
                    },
                )
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            self.hardware.timer = Some((deadline, token));
        }
        Ok(())
    }
}
