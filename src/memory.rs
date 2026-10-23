//! Memory metrics source using sysinfo
//!
//! As a [`MetricSource`] this publishes:
//!
//! | name | unit |
//! | --- | --- |
//! | `memory.total_bytes` | [`Bytes`](Unit::Bytes) |
//! | `memory.used_bytes` | [`Bytes`](Unit::Bytes) |
//! | `memory.available_bytes` | [`Bytes`](Unit::Bytes) |
//! | `memory.used_percent` | [`Percent`](Unit::Percent) |
//! | `memory.swap_total_bytes` | [`Bytes`](Unit::Bytes) |
//! | `memory.swap_used_bytes` | [`Bytes`](Unit::Bytes) |

use crate::metric::{Metric, MetricSource, Unit};
use crate::types::MemoryMetrics;
use sysinfo::{MemoryRefreshKind, RefreshKind, System};

pub struct MemorySource {
    system: System,
}

impl MemorySource {
    pub fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
        );
        system.refresh_memory();
        Self { system }
    }

    pub fn refresh(&mut self) -> MemoryMetrics {
        self.system.refresh_memory();

        MemoryMetrics {
            total_bytes: self.system.total_memory(),
            used_bytes: self.system.used_memory(),
            available_bytes: self.system.available_memory(),
            swap_total_bytes: self.system.total_swap(),
            swap_used_bytes: self.system.used_swap(),
        }
    }
}

impl Default for MemorySource {
    fn default() -> Self {
        Self::new()
    }
}

/// Used memory as a percentage of total.
///
/// Split out to be tested directly: a machine always reports a total, so the
/// guard against a zero total is unreachable through [`MemorySource::refresh`].
/// Without it an empty total would divide to NaN, which no consumer can compare
/// or display sanely.
fn used_percent(used_bytes: u64, total_bytes: u64) -> f64 {
    match total_bytes {
        0 => 0.0,
        total => used_bytes as f64 / total as f64 * 100.0,
    }
}

impl MetricSource for MemorySource {
    fn read(&mut self) -> Vec<Metric> {
        let metrics = self.refresh();

        let used_percent = used_percent(metrics.used_bytes, metrics.total_bytes);

        vec![
            Metric::new("memory.total_bytes", metrics.total_bytes, Unit::Bytes),
            Metric::new("memory.used_bytes", metrics.used_bytes, Unit::Bytes),
            Metric::new(
                "memory.available_bytes",
                metrics.available_bytes,
                Unit::Bytes,
            ),
            Metric::new("memory.used_percent", used_percent, Unit::Percent),
            Metric::new(
                "memory.swap_total_bytes",
                metrics.swap_total_bytes,
                Unit::Bytes,
            ),
            Metric::new(
                "memory.swap_used_bytes",
                metrics.swap_used_bytes,
                Unit::Bytes,
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_every_documented_reading() {
        let mut memory = MemorySource::new();
        let names: Vec<String> = memory.read().into_iter().map(|m| m.name).collect();

        assert_eq!(
            names,
            [
                "memory.total_bytes",
                "memory.used_bytes",
                "memory.available_bytes",
                "memory.used_percent",
                "memory.swap_total_bytes",
                "memory.swap_used_bytes",
            ]
        );
    }

    #[test]
    fn used_percent_agrees_with_used_over_total() {
        let mut memory = MemorySource::new();
        let metrics = memory.read();

        let value = |name: &str| {
            metrics
                .iter()
                .find(|m| m.name == name)
                .and_then(|m| m.value.as_f64())
                .expect("published above")
        };

        let total = value("memory.total_bytes");
        if total > 0.0 {
            let expected = value("memory.used_bytes") / total * 100.0;
            assert!((value("memory.used_percent") - expected).abs() < 0.01);
        }
    }

    #[test]
    fn no_total_reads_as_zero_percent_not_nan() {
        assert_eq!(used_percent(5, 0), 0.0);
        assert!(!used_percent(5, 0).is_nan());
    }

    #[test]
    fn used_percent_is_a_percentage_of_total() {
        assert_eq!(used_percent(25, 100), 25.0);
        assert_eq!(used_percent(0, 100), 0.0);
    }
}
