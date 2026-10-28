//! Native frame callbacks follow a monotonic clock, including frames with no new damage.

use std::time::Duration;

#[derive(Debug)]
pub struct FrameClock {
    interval: Duration,
    last: Option<Duration>,
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::from_refresh_millihz(60_000)
    }
}

impl FrameClock {
    pub fn from_refresh_millihz(refresh: u32) -> Self {
        Self {
            interval: Duration::from_nanos(1_000_000_000_000 / u64::from(refresh.max(1))),
            last: None,
        }
    }

    /// None permits callbacks now; Some supplies the next allowed monotonic time.
    pub fn request(&mut self, now: Duration) -> Option<Duration> {
        if let Some(last) = self.last
            && now < last + self.interval
        {
            return Some(last + self.interval);
        }
        self.last = Some(now);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undamaged_callback_requests_cannot_create_an_unthrottled_loop() {
        let mut clock = FrameClock::default();
        assert_eq!(clock.request(Duration::ZERO), None);
        for micros in 1..1000 {
            assert_eq!(
                clock.request(Duration::from_micros(micros)),
                Some(Duration::from_nanos(16_666_666))
            );
        }
        assert_eq!(clock.request(Duration::from_millis(17)), None);
        assert!(clock.request(Duration::from_millis(17)).is_some());
    }

    #[test]
    fn idle_time_does_not_produce_a_burst_of_catchup_callbacks() {
        let mut clock = FrameClock::from_refresh_millihz(60_000);
        clock.request(Duration::ZERO);
        assert_eq!(clock.request(Duration::from_secs(30)), None);
        assert_eq!(
            clock.request(Duration::from_secs(30)),
            Some(Duration::from_secs(30) + Duration::from_nanos(16_666_666))
        );
    }
}
