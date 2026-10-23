//! CPU metrics source using sysinfo
//!
//! As a [`MetricSource`] this publishes:
//!
//! | name | unit | notes |
//! | --- | --- | --- |
//! | `cpu.global_usage` | [`Percent`](Unit::Percent) | across all cores |
//! | `cpu.core_count` | [`None`](Unit::None) | a count |
//! | `cpu.frequency` | [`Hertz`](Unit::Hertz) | omitted when unknown |
//! | `cpu.temperature` | [`Celsius`](Unit::Celsius) | omitted when no sensor |
//! | `cpu.core.N.usage` | [`Percent`](Unit::Percent) | one per core, `N` from 0 |

use crate::metric::{Metric, MetricSource, Unit};
use crate::types::CpuMetrics;
use sysinfo::{Components, CpuRefreshKind, RefreshKind, System};

/// Thermal sensor labels are free-form and differ per platform/vendor, so match
/// on substrings rather than exact names. Covers macOS (`PMU tdie0`), Windows
/// (`CPU Package`, `Core Max`) and Linux (`Package id 0`, `Core 0`).
const CPU_SENSOR_HINTS: [&str; 6] = ["cpu", "core", "package", "tdie", "soc", "peci"];

pub struct CpuSource {
    system: System,
    components: Components,
}

impl CpuSource {
    pub fn new() -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()),
        );

        // CPU usage is a delta between two refreshes. Without a real interval
        // between them the first reading is meaningless, so take two samples
        // the minimum distance apart and let the first `refresh()` be accurate.
        system.refresh_cpu_all();
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        system.refresh_cpu_all();

        Self {
            system,
            components: Components::new_with_refreshed_list(),
        }
    }

    pub fn refresh(&mut self) -> CpuMetrics {
        self.system.refresh_cpu_all();

        let global_usage = self.system.global_cpu_usage();
        let per_core: Vec<f32> = self.system.cpus().iter().map(|c| c.cpu_usage()).collect();
        let core_count = self.system.cpus().len();
        // Apple Silicon reports 0 here; expose that as "unknown" rather than 0 MHz.
        let frequency_mhz = self
            .system
            .cpus()
            .first()
            .map(|c| c.frequency())
            .filter(|f| *f > 0);

        CpuMetrics {
            global_usage,
            per_core,
            core_count,
            frequency_mhz,
            temperature_celsius: self.cpu_temperature(),
        }
    }

    /// Hottest CPU-related thermal sensor, or `None` if the platform exposes
    /// no sensor we recognise.
    fn cpu_temperature(&mut self) -> Option<f32> {
        self.components.refresh(true);

        self.components
            .list()
            .iter()
            .filter(|c| {
                let label = c.label().to_ascii_lowercase();
                CPU_SENSOR_HINTS.iter().any(|hint| label.contains(hint))
            })
            .filter_map(|c| c.temperature())
            .reduce(f32::max)
    }
}

impl Default for CpuSource {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricSource for CpuSource {
    fn read(&mut self) -> Vec<Metric> {
        let metrics = self.refresh();

        // The three scalars first, then the per-core group: a consumer reading
        // in order gets the summary before the detail.
        let mut out = vec![
            Metric::new(
                "cpu.global_usage",
                f64::from(metrics.global_usage),
                Unit::Percent,
            ),
            Metric::new("cpu.core_count", metrics.core_count as u64, Unit::None),
        ];

        if let Some(frequency) = metrics.frequency_mhz {
            out.push(Metric::new("cpu.frequency", frequency, Unit::Hertz));
        }
        if let Some(temperature) = metrics.temperature_celsius {
            out.push(Metric::new(
                "cpu.temperature",
                f64::from(temperature),
                Unit::Celsius,
            ));
        }

        out.extend(metrics.per_core.iter().enumerate().map(|(core, usage)| {
            Metric::new(
                format!("cpu.core.{core}.usage"),
                f64::from(*usage),
                Unit::Percent,
            )
        }));

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_the_scalars_then_one_metric_per_core() {
        let mut cpu = CpuSource::new();
        let metrics = cpu.read();

        assert!(metrics.iter().any(|m| m.name == "cpu.global_usage"));
        assert!(metrics.iter().any(|m| m.name == "cpu.core_count"));

        let cores = metrics
            .iter()
            .filter(|m| m.name.starts_with("cpu.core."))
            .count();
        assert_eq!(
            cores,
            metrics
                .iter()
                .find(|m| m.name == "cpu.core_count")
                .and_then(|m| m.value.as_f64())
                .expect("core count is published") as usize,
            "one cpu.core.N.usage per reported core"
        );
    }

    #[test]
    fn usage_is_a_percentage_in_range() {
        let mut cpu = CpuSource::new();
        for metric in cpu.read() {
            if metric.unit != Unit::Percent {
                continue;
            }
            let value = metric.value.as_f64().unwrap();
            assert!(
                (0.0..=100.0).contains(&value),
                "{} read {value}, outside 0-100",
                metric.name
            );
        }
    }
}
