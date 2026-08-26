#![feature(default_field_values)]
use std::collections::{VecDeque, vec_deque};
use std::time::{Duration, Instant};

#[derive(Default)]
pub struct RateMonitor {
    window: Duration = Duration::from_secs(1),
    timestamps: VecDeque<Instant>,
}

impl RateMonitor {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            timestamps: VecDeque::new(),
        }
    }

    /// Record one event.
    pub fn record(&mut self) {
        let now = Instant::now();

        self.timestamps.push_back(now);
        self.remove_expired(now);
    }

    /// Return the current rate in events/second.
    pub fn rate(&mut self) -> f64 {
        let now = Instant::now();

        self.remove_expired(now);

        self.timestamps.len() as f64 / self.window.as_secs_f64()
    }

    fn remove_expired(&mut self, now: Instant) {
        let cutoff = now - self.window;

        while let Some(&timestamp) = self.timestamps.front() {
            if timestamp >= cutoff {
                break;
            }

            self.timestamps.pop_front();
        }
    }
}
