//! Bounded development counters. CPU render cost excludes screenshot readback.

use serde::Serialize;
use std::time::Duration;

#[derive(Default, Serialize)]
pub struct RenderMetrics {
    pub rendered_frames: u64,
    pub cpu_total_us: u64,
    pub cpu_max_us: u64,
    pub drm_presentations: u64,
    pub dmabuf_imports: u64,
    pub dmabuf_rejections: u64,
}

impl RenderMetrics {
    pub fn record(&mut self, elapsed: Duration) {
        let micros = elapsed.as_micros().min(u128::from(u64::MAX)) as u64;
        self.rendered_frames = self.rendered_frames.saturating_add(1);
        self.cpu_total_us = self.cpu_total_us.saturating_add(micros);
        self.cpu_max_us = self.cpu_max_us.max(micros);
    }
}
