//! Rolling sensor history (up to 24 h) for the charts and CSV export.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use msi_acpi::Snapshot;

pub const POLL: Duration = Duration::from_millis(1500);
/// Chart ranges offered in the UI.
pub const RANGES: [Duration; 3] =
    [Duration::from_secs(5 * 60), Duration::from_secs(60 * 60), Duration::from_secs(24 * 3600)];
/// History is kept for the longest range (≈ 58 k samples, ~2 MB).
const KEEP: Duration = Duration::from_secs(24 * 3600);

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub at: Instant,
    pub wall: SystemTime,
    pub cpu_temp: Option<u8>,
    pub gpu_temp: Option<u8>,
    pub cpu_rpm: u32,
    pub gpu_rpm: u32,
    pub cpu_duty: u8,
    pub gpu_duty: u8,
}

impl Sample {
    pub fn from_snapshot(s: &Snapshot, at: Instant, wall: SystemTime) -> Self {
        Self {
            at,
            wall,
            cpu_temp: s.cpu.temp_c,
            gpu_temp: s.gpu.temp_c,
            cpu_rpm: s.cpu.rpm,
            gpu_rpm: s.gpu.rpm,
            cpu_duty: s.cpu.duty,
            gpu_duty: s.gpu.duty,
        }
    }
}

/// One chart column: x in 0..=1 (1 = now) and the hottest reading in that column.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f32,
    pub cpu: Option<u8>,
    pub gpu: Option<u8>,
}

#[derive(Debug, Default)]
pub struct History {
    samples: VecDeque<Sample>,
}

impl History {
    pub fn push(&mut self, s: &Snapshot) {
        self.push_sample(Sample::from_snapshot(s, Instant::now(), SystemTime::now()));
    }

    fn push_sample(&mut self, sample: Sample) {
        let now = sample.at;
        self.samples.push_back(sample);
        while self.samples.front().is_some_and(|f| now.duration_since(f.at) > KEEP) {
            self.samples.pop_front();
        }
    }

    /// Samples within `window` of `now`, reduced to at most `columns` points. Each column keeps
    /// its peak temperatures so short spikes stay visible on long ranges.
    pub fn series(&self, now: Instant, window: Duration, columns: usize) -> Vec<Point> {
        let span = window.as_secs_f32();
        let columns = columns.max(1);
        let mut out: Vec<Point> = Vec::new();
        let mut current: Option<(usize, Point)> = None;
        let start = self.samples.partition_point(|s| now.saturating_duration_since(s.at) > window);
        for s in self.samples.range(start..) {
            let x = 1.0 - now.saturating_duration_since(s.at).as_secs_f32() / span;
            let col = ((x * columns as f32) as usize).min(columns - 1);
            match &mut current {
                Some((c, p)) if *c == col => {
                    p.x = x;
                    p.cpu = p.cpu.max(s.cpu_temp);
                    p.gpu = p.gpu.max(s.gpu_temp);
                }
                _ => {
                    if let Some((_, p)) = current.take() {
                        out.push(p);
                    }
                    current = Some((col, Point { x, cpu: s.cpu_temp, gpu: s.gpu_temp }));
                }
            }
        }
        out.extend(current.map(|(_, p)| p));
        out
    }

    /// Sample nearest to `age` seconds ago.
    pub fn at_age(&self, now: Instant, age: f32) -> Option<&Sample> {
        let target = now.checked_sub(Duration::from_secs_f32(age.max(0.0)))?;
        let i = self.samples.partition_point(|s| s.at < target);
        let after = self.samples.get(i);
        let before = i.checked_sub(1).and_then(|j| self.samples.get(j));
        match (before, after) {
            (Some(b), Some(a)) => {
                Some(if target.duration_since(b.at) <= a.at.duration_since(target) { b } else { a })
            }
            (b, a) => b.or(a),
        }
    }

    /// Most recent `n` fan readings (oldest first), for sparklines.
    pub fn recent_rpm(&self, n: usize, gpu: bool) -> Vec<u32> {
        let skip = self.samples.len().saturating_sub(n);
        self.samples.iter().skip(skip).map(|s| if gpu { s.gpu_rpm } else { s.cpu_rpm }).collect()
    }

    /// The whole history as CSV (UTC timestamps, empty cells for sleeping sensors).
    pub fn to_csv(&self) -> String {
        let mut out =
            String::from("time_utc,cpu_temp_c,gpu_temp_c,cpu_rpm,gpu_rpm,cpu_duty_pct,gpu_duty_pct\n");
        let cell = |v: Option<u8>| v.map(|v| v.to_string()).unwrap_or_default();
        for s in &self.samples {
            let _ = writeln!(
                out,
                "{},{},{},{},{},{},{}",
                rfc3339(s.wall),
                cell(s.cpu_temp),
                cell(s.gpu_temp),
                s.cpu_rpm,
                s.gpu_rpm,
                s.cpu_duty,
                s.gpu_duty
            );
        }
        out
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` for a UTC instant.
pub fn rfc3339(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil-from-days (Howard Hinnant), valid for all dates after 1970.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(at: Instant, cpu: u8) -> Sample {
        Sample {
            at,
            wall: UNIX_EPOCH,
            cpu_temp: Some(cpu),
            gpu_temp: None,
            cpu_rpm: 3000,
            gpu_rpm: 0,
            cpu_duty: 50,
            gpu_duty: 38,
        }
    }

    #[test]
    fn formats_utc_timestamps() {
        assert_eq!(rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(UNIX_EPOCH + Duration::from_secs(951_782_400)), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(UNIX_EPOCH + Duration::from_secs(1_794_096_000 + 3661)), "2026-11-08T01:01:01Z");
    }

    #[test]
    fn long_ranges_keep_spikes_when_decimating() {
        let base = Instant::now();
        let mut h = History::default();
        for i in 0..600u64 {
            let t = if i == 300 { 99 } else { 60 };
            h.push_sample(sample(base + Duration::from_secs(i), t));
        }
        let now = base + Duration::from_secs(599);
        let pts = h.series(now, Duration::from_secs(600), 50);
        assert!(pts.len() <= 50, "{}", pts.len());
        assert!(pts.iter().any(|p| p.cpu == Some(99)), "the one-sample spike survives");
        assert!(pts.windows(2).all(|w| w[0].x <= w[1].x));
        assert!(h.series(now, Duration::from_secs(60), 400).len() <= 61, "only the last minute");
    }

    #[test]
    fn drops_samples_older_than_a_day() {
        let base = Instant::now();
        let mut h = History::default();
        h.push_sample(sample(base, 50));
        h.push_sample(sample(base + KEEP + Duration::from_secs(5), 51));
        assert_eq!(h.samples.len(), 1);
    }

    #[test]
    fn csv_has_header_and_empty_cells_for_asleep_sensors() {
        let mut h = History::default();
        h.push_sample(sample(Instant::now(), 72));
        let csv = h.to_csv();
        let mut lines = csv.lines();
        assert_eq!(
            lines.next(),
            Some("time_utc,cpu_temp_c,gpu_temp_c,cpu_rpm,gpu_rpm,cpu_duty_pct,gpu_duty_pct")
        );
        assert_eq!(lines.next(), Some("1970-01-01T00:00:00Z,72,,3000,0,50,38"));
    }
}
