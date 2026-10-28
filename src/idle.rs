//! Event-driven idle policy and visible-surface idle inhibitors.
use crate::{CalloopData, Compositor};
use smithay::{
    delegate_idle_inhibit,
    reexports::{
        calloop::{
            LoopHandle, RegistrationToken,
            timer::{TimeoutAction, Timer},
        },
        wayland_server::{Resource, protocol::wl_surface::WlSurface},
    },
    wayland::idle_inhibit::IdleInhibitHandler,
};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use tarot_core::idle::{Phase, Policy};

#[derive(Default)]
pub struct State {
    pub render_states: Option<smithay::backend::renderer::element::RenderElementStates>,
    pub seat_inactive: bool,
    pub policy: Option<Policy>,
    pub output_off: bool,
    pub off_count: u64,
    pub wake_count: u64,
    inhibitors: HashMap<WlSurface, usize>,
    timer: Option<(Instant, RegistrationToken)>,
}
impl State {
    pub fn barrier(&self) -> Option<u64> {
        self.policy.as_ref().and_then(Policy::barrier)
    }
    pub fn off(&self) -> bool {
        self.policy.as_ref().is_some_and(|p| p.phase == Phase::Off)
    }
    pub fn blocks_unlock(&self) -> bool {
        self.policy.as_ref().is_some_and(Policy::blocks_unlock)
    }
    pub fn status(&self, ordinary: bool) -> serde_json::Value {
        serde_json::json!({"policy":self.policy,"output_off":self.output_off,"off_count":self.off_count,"wake_count":self.wake_count,"inhibitor_surfaces":ordinary.then_some(self.inhibitors.len())})
    }
}
impl IdleInhibitHandler for Compositor {
    fn inhibit(&mut self, surface: WlSurface) {
        *self.idle.inhibitors.entry(surface).or_default() += 1;
    }
    fn uninhibit(&mut self, surface: WlSurface) {
        if let Some(count) = self.idle.inhibitors.get_mut(&surface) {
            *count -= 1;
            if *count == 0 {
                self.idle.inhibitors.remove(&surface);
            }
        }
    }
}
delegate_idle_inhibit!(Compositor);

impl Compositor {
    fn visible_idle_inhibitor(&mut self) -> bool {
        if self.protected() || self.idle.seat_inactive {
            return false;
        }
        self.idle.inhibitors.keys().any(|surface| {
            let Some(placement) = self.presented_surfaces.get(surface) else {
                return false;
            };
            if placement.direct || !self.presentation.visible(placement.window, self.layout) {
                return false;
            }
            if !self
                .idle
                .render_states
                .as_ref()
                .and_then(|states| {
                    states.element_render_state(
                        smithay::backend::renderer::element::Id::from_wayland_resource(surface),
                    )
                })
                .is_some_and(|state| state.visible_area > 0)
            {
                return false;
            }
            // A cached placement alone is insufficient after a null commit.
            smithay::backend::renderer::utils::with_renderer_surface_state(surface, |state| {
                state.buffer().is_some()
            })
            .unwrap_or(false)
        })
    }
    pub fn idle_tick(&mut self) {
        // The protocol remains available with automatic idle disabled. Retire
        // dead surface handles in that mode too, including abrupt client loss.
        self.idle.inhibitors.retain(|surface, _| surface.is_alive());
        if self.idle.policy.is_none() {
            return;
        }
        let now = self.start_time.elapsed();
        let inhibited = self.visible_idle_inhibitor() || self.sleep.blocks_unlock();
        self.idle.policy.as_mut().unwrap().inhibit(inhibited, now);
        if self
            .idle
            .policy
            .as_ref()
            .unwrap()
            .lock_due(now, self.protected())
            && let Err(error) = self.request_lock()
        {
            self.fatal_error = Some(error.to_string());
            self.loop_signal.stop();
            return;
        }
        if self.idle.policy.as_mut().unwrap().begin_off(now) {
            // Even an already locked panel requires a newly queued protected
            // frame. Background authentication must not race the transition.
            self.clear_protected_focus();
            self.refresh_lock_focus();
            self.redraw = true;
        }
    }
    pub fn schedule_idle(
        &mut self,
        handle: &LoopHandle<'static, CalloopData>,
    ) -> anyhow::Result<()> {
        let deadline = self
            .idle
            .policy
            .as_ref()
            .and_then(|p| p.deadline(self.protected()))
            .and_then(|d| self.start_time.checked_add(d));
        if self
            .idle
            .timer
            .as_ref()
            .is_some_and(|(old, _)| deadline.is_some_and(|next| *old <= next))
        {
            return Ok(());
        }
        if let Some((_, token)) = self.idle.timer.take() {
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
                        data.state.idle.timer = None;
                        data.state.idle_tick();
                        TimeoutAction::Drop
                    },
                )
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            self.idle.timer = Some((deadline, token));
        }
        Ok(())
    }
    pub fn idle_activity(&mut self) {
        let before = self.idle.barrier();
        if let Some(policy) = &mut self.idle.policy {
            policy.activity(self.start_time.elapsed());
        }
        if before != self.idle.barrier() || self.idle.output_off {
            self.redraw = true;
        }
    }
    pub fn idle_system_wake(&mut self) {
        if let Some(policy) = &mut self.idle.policy {
            policy.system_activity(self.start_time.elapsed());
        }
        self.redraw = true;
    }
    pub fn idle_frame_presented(&mut self, ticket: Option<u64>, generation: Option<u64>) {
        let Some(ticket) = ticket else {
            return;
        };
        if generation.is_some() && generation == self.lock_generation() {
            if let Some(policy) = &mut self.idle.policy {
                policy.presented(ticket);
            }
        } else if self.idle.barrier().is_some() {
            self.redraw = true;
        }
    }
    /// Wake contacts/keys never become locker input. Their trailing releases
    /// remain suppressed after the fresh protected wake frame completes.
    pub fn idle_input<I: smithay::backend::input::InputBackend>(
        &mut self,
        event: &smithay::backend::input::InputEvent<I>,
    ) -> bool {
        use smithay::backend::input::{
            ButtonState, InputEvent, KeyState, KeyboardKeyEvent, PointerButtonEvent, TouchEvent,
        };
        if self.idle.policy.is_none() {
            return false;
        }
        let consume = self.idle.policy.as_ref().is_some_and(|p| {
            matches!(p.phase, Phase::Off | Phase::Waking(_)) || p.manual_off_waiting()
        });
        let pointer_fresh = self.pointer_buttons.is_empty()
            || self
                .pointer_buttons
                .iter()
                .any(|b| !self.suppressed_pointer_buttons.contains(b));
        let activity = matches!(event,InputEvent::Keyboard{event} if event.state()==KeyState::Pressed && !self.intercepted_keys.contains(&event.key_code().raw()))
            || matches!(event,InputEvent::PointerButton{event} if event.state()==ButtonState::Pressed && !self.suppressed_pointer_buttons.contains(&event.button_code()))
            || matches!(event,InputEvent::PointerMotion{..}|InputEvent::PointerMotionAbsolute{..}|InputEvent::PointerAxis{..} if pointer_fresh)
            || matches!(event,InputEvent::TouchDown{..} if !consume || !self.touches.has_contacts())
            || matches!(event,InputEvent::TouchMotion{event} if self.touches.target(i32::from(event.slot()))!=tarot_core::touch::Target::Suppressed);
        if activity {
            self.idle_activity();
        }
        if !consume {
            return false;
        }
        match event {
            InputEvent::Keyboard { event } => {
                if event.state() == KeyState::Pressed {
                    self.intercepted_keys.insert(event.key_code().raw());
                } else {
                    self.intercepted_keys.remove(&event.key_code().raw());
                }
            }
            InputEvent::PointerButton { event } => {
                let code = event.button_code();
                if event.state() == ButtonState::Pressed {
                    self.pointer_buttons.insert(code);
                    self.suppressed_pointer_buttons.insert(code);
                } else {
                    self.pointer_buttons.remove(&code);
                    self.suppressed_pointer_buttons.remove(&code);
                }
            }
            InputEvent::TouchDown { event } => {
                self.touches.begin(i32::from(event.slot()), false);
                self.touches.cancel();
            }
            InputEvent::TouchUp { event } => {
                self.touches.end(i32::from(event.slot()));
            }
            InputEvent::TouchCancel { .. } => {
                self.touches.reset();
            }
            InputEvent::TouchMotion { .. }
            | InputEvent::PointerMotion { .. }
            | InputEvent::PointerMotionAbsolute { .. }
            | InputEvent::PointerAxis { .. } => {}
            _ => return false,
        }
        true
    }
}
