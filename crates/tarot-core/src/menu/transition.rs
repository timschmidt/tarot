//! Host-clock menu transitions. A model without a host clock applies commands
//! synchronously; every compositor backend registers and advances this clock.
use super::Target;
use serde::Serialize;
use std::time::Duration;

// Menu-specific adaptations, pending historical/physical timing calibration.
pub const OPEN_MS: u64 = 160;
pub const CLOSE_MS: u64 = 120;
const TRAVEL: f32 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Visual {
    pub opacity: f32,
    pub scrim: f32,
    pub offset_y: f32,
    pub moving: bool,
    pub closing: bool,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Motion {
    clock: Option<Duration>,
    started: Duration,
    duration: Duration,
    from: f32,
    to: f32,
    scrim_from: f32,
    scrim_to: f32,
    reduced: bool,
    pub(super) closing: bool,
    pub(super) pending: Option<Target>,
}

impl Motion {
    pub(super) fn clock_active(&self) -> bool {
        self.clock.is_some()
    }
    pub(super) fn track(&mut self, now: Duration, reduced: bool) {
        self.clock = Some(self.clock.map_or(now, |old| old.max(now)));
        if self.reduced != reduced {
            let visual = self.visual();
            self.reduced = reduced;
            if visual.moving {
                self.from = visual.opacity;
                self.scrim_from = visual.scrim;
                self.restart();
            }
        }
    }
    fn progress(&self) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        (self
            .clock
            .unwrap_or_default()
            .saturating_sub(self.started)
            .as_secs_f32()
            / self.duration.as_secs_f32())
        .clamp(0.0, 1.0)
    }
    pub(super) fn visual(&self) -> Visual {
        let progress = self.progress();
        let ease = 1.0 - (1.0 - progress).powi(3);
        let opacity = self.from + (self.to - self.from) * ease;
        Visual {
            opacity,
            scrim: self.scrim_from + (self.scrim_to - self.scrim_from) * ease,
            offset_y: if self.reduced {
                0.0
            } else {
                -TRAVEL * (1.0 - opacity)
            },
            moving: progress < 1.0,
            closing: self.closing,
        }
    }
    fn restart(&mut self) {
        self.started = self.clock.unwrap_or_default();
        let base = if self.reduced {
            crate::design::REDUCED_MOTION_FADE_MS
        } else if self.closing {
            CLOSE_MS
        } else {
            OPEN_MS
        };
        let distance = (self.to - self.from)
            .abs()
            .max((self.scrim_to - self.scrim_from).abs());
        self.duration = if self.clock.is_some() {
            Duration::from_millis(base).mul_f32(distance)
        } else {
            Duration::ZERO
        };
    }
    pub(super) fn open(&mut self) {
        let visual = self.visual();
        // Reversing dismissal keeps its current pose. A root switch replaces
        // the panel while retaining the existing single scrim.
        self.from = if self.closing { visual.opacity } else { 0.0 };
        self.scrim_from = visual.scrim;
        self.to = 1.0;
        self.scrim_to = 1.0;
        self.closing = false;
        self.pending = None;
        self.restart();
    }
    pub(super) fn close(&mut self, pending: Option<Target>) -> bool {
        if self.clock.is_none() {
            return false;
        }
        let visual = self.visual();
        self.from = visual.opacity;
        self.scrim_from = visual.scrim;
        self.to = 0.0;
        self.scrim_to = 0.0;
        self.closing = true;
        self.pending = pending;
        self.restart();
        !self.duration.is_zero()
    }
    pub(super) fn reset(&mut self) {
        self.from = 0.0;
        self.to = 0.0;
        self.scrim_from = 0.0;
        self.scrim_to = 0.0;
        self.duration = Duration::ZERO;
        self.closing = false;
        self.pending = None;
    }
    pub(super) fn settle_open(&mut self) {
        self.from = 1.0;
        self.to = 1.0;
        self.scrim_from = 1.0;
        self.scrim_to = 1.0;
        self.duration = Duration::ZERO;
        self.closing = false;
        self.pending = None;
    }
}
