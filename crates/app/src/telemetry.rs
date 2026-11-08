//! Rolling history of sensor readings for the charts.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use msi_acpi::Snapshot;

/// Visible time window of the telemetry chart.
pub const WINDOW: Duration = Duration::from_secs(300);
pub const POLL: Duration = Duration::from_millis(1500);

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub at: Instant,
    pub cpu_temp: Option<u8>,
    pub gpu_temp: Option<u8>,
    pub cpu_rpm: u32,
    pub gpu_rpm: u32,
}

#[derive(Debug, Default)]
pub struct History {
    samples: VecDeque<Sample>,
}

impl History {
    pub fn push(&mut self, s: &Snapshot) {
        let now = Instant::now();
        self.samples.push_back(Sample {
            at: now,
            cpu_temp: s.cpu.temp_c,
            gpu_temp: s.gpu.temp_c,
            cpu_rpm: s.cpu.rpm,
            gpu_rpm: s.gpu.rpm,
        });
        while self.samples.front().is_some_and(|f| now.duration_since(f.at) > WINDOW + POLL) {
            self.samples.pop_front();
        }
    }

    pub fn samples(&self) -> &VecDeque<Sample> {
        &self.samples
    }

    /// Sample nearest to `age` seconds ago.
    pub fn at_age(&self, now: Instant, age: f32) -> Option<&Sample> {
        self.samples.iter().min_by(|a, b| {
            let da = (now.duration_since(a.at).as_secs_f32() - age).abs();
            let db = (now.duration_since(b.at).as_secs_f32() - age).abs();
            da.total_cmp(&db)
        })
    }

    /// Most recent `n` fan readings (oldest first), for sparklines.
    pub fn recent_rpm(&self, n: usize, gpu: bool) -> Vec<u32> {
        let skip = self.samples.len().saturating_sub(n);
        self.samples.iter().skip(skip).map(|s| if gpu { s.gpu_rpm } else { s.cpu_rpm }).collect()
    }
}
