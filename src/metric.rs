//! The generic metric layer: any reading, from any source, in one shape.
//!
//! The typed sources — [`CpuSource`](crate::cpu::CpuSource),
//! [`MemorySource`](crate::memory::MemorySource) and the rest — describe
//! *system* resources. This module is the other half of the crate: it lets any
//! data at all be read as a **metric**, so one consumer handles every reading
//! the same way whether it came from this crate or from the host application.
//!
//! Three pieces:
//!
//! - [`Metric`]: a named reading with a value and a [`Unit`]. Every built-in
//!   source implements [`MetricSource`], so its readings arrive in this shape
//!   too — each source module documents the names it publishes.
//! - [`MetricSource`]: the trait a producer implements. [`FnSource`] wraps a
//!   closure, for data that is already at hand.
//! - [`Collector`]: a set of sources sampled together into a
//!   [`MetricSnapshot`].
//!
//! A source that reports the same reading for several things — a disk per mount
//! point, a network interface per device — puts the instance in the metric name
//! (`disk./.used_bytes`, `net.en0.rx_bytes_per_sec`) rather than carrying a
//! separate label set. That keeps a metric a plain name, value and unit, which
//! is what makes the escape hatch pleasant to use.
//!
//! ```
//! use rustcat_metrics::metric::FnSource;
//! use rustcat_metrics::{Collector, Metric, MetricValue, Unit};
//!
//! let queue_depth = 3u64;
//! let mut collector = Collector::new().with(FnSource::new(move || {
//!     vec![Metric::new("build.queue_depth", queue_depth, Unit::None)]
//! }));
//!
//! let snapshot = collector.snapshot();
//! let reading = snapshot.get("build.queue_depth").expect("just published");
//! assert_eq!(reading.value, MetricValue::Count(3));
//! ```

use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// What a [`MetricValue`] counts, so a consumer can format a reading without
/// knowing which source produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// A bare number: counts, ratios, indices.
    None,
    /// A percentage, 0-100.
    Percent,
    /// A size in bytes.
    Bytes,
    /// A transfer rate in bytes per second.
    BytesPerSecond,
    /// A frequency in hertz.
    Hertz,
    /// A temperature in degrees Celsius.
    Celsius,
}

impl Unit {
    /// The suffix a human-facing rendering should append; `""` for
    /// [`Unit::None`].
    pub fn suffix(self) -> &'static str {
        match self {
            Unit::None => "",
            Unit::Percent => "%",
            Unit::Bytes => "B",
            Unit::BytesPerSecond => "B/s",
            Unit::Hertz => "Hz",
            Unit::Celsius => "°C",
        }
    }
}

/// A metric's value.
///
/// [`Count`](MetricValue::Count) and [`Gauge`](MetricValue::Gauge) are separate
/// variants rather than one `f64` so an exact count stays exact: a byte total is
/// 64-bit and does not survive a trip through `f64` unchanged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetricValue {
    /// An exact unsigned count.
    Count(u64),
    /// A measured or computed number.
    Gauge(f64),
    /// A value that is not a number: a state, a label, a version.
    Text(String),
}

impl MetricValue {
    /// The value as a number, or `None` for [`MetricValue::Text`].
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            MetricValue::Count(count) => Some(*count as f64),
            MetricValue::Gauge(gauge) => Some(*gauge),
            MetricValue::Text(_) => None,
        }
    }

    /// The value as text, or `None` for the numeric variants.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            MetricValue::Text(text) => Some(text),
            MetricValue::Count(_) | MetricValue::Gauge(_) => None,
        }
    }
}

impl From<u64> for MetricValue {
    fn from(value: u64) -> Self {
        MetricValue::Count(value)
    }
}

impl From<f64> for MetricValue {
    fn from(value: f64) -> Self {
        MetricValue::Gauge(value)
    }
}

impl From<String> for MetricValue {
    fn from(value: String) -> Self {
        MetricValue::Text(value)
    }
}

impl From<&str> for MetricValue {
    fn from(value: &str) -> Self {
        MetricValue::Text(value.to_owned())
    }
}

/// One named reading.
///
/// `name` is the identity a consumer looks a reading up by, so it should be
/// stable across reads — see the module docs for the naming convention.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    /// Stable identifier, e.g. `"cpu.global_usage"`. Dots group a namespace.
    pub name: String,
    /// The reading itself.
    pub value: MetricValue,
    /// What the value is measured in.
    pub unit: Unit,
}

impl Metric {
    pub fn new(name: impl Into<String>, value: impl Into<MetricValue>, unit: Unit) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            unit,
        }
    }

    /// This reading with its value replaced, for a source that recomputes one
    /// metric per read.
    pub fn with_value(&self, value: impl Into<MetricValue>) -> Self {
        Self {
            name: self.name.clone(),
            value: value.into(),
            unit: self.unit,
        }
    }
}

/// Anything that can be read as a set of metrics.
///
/// Implementors are sampled repeatedly, so [`read`](MetricSource::read) should
/// report the current value each time rather than accumulating.
pub trait MetricSource {
    /// Read the source's current metrics.
    ///
    /// An empty `Vec` is a valid reading — the source has nothing to report
    /// right now — and is not an error. A source that genuinely cannot read
    /// should say so with a [`MetricValue::Text`] metric rather than by
    /// vanishing, so a consumer can tell "nothing to report" from "broken".
    fn read(&mut self) -> Vec<Metric>;
}

/// A [`MetricSource`] backed by a closure.
///
/// For data the caller already has: a queue depth, a file's mtime, a value from
/// another thread. The closure is called on every read.
pub struct FnSource<F> {
    read: F,
}

impl<F: FnMut() -> Vec<Metric>> FnSource<F> {
    pub fn new(read: F) -> Self {
        Self { read }
    }
}

impl<F: FnMut() -> Vec<Metric>> MetricSource for FnSource<F> {
    fn read(&mut self) -> Vec<Metric> {
        (self.read)()
    }
}

/// A set of sources sampled together.
///
/// Sources are sampled in insertion order, so a collector's output order is
/// stable across reads — which is what lets a consumer zip two snapshots
/// together instead of matching on names.
///
/// Sources are boxed behind `Send` because a collector is meant to be moved to
/// whatever thread does the sampling; without the bound the collector itself
/// would not be `Send` and could not leave the thread that built it.
#[derive(Default)]
pub struct Collector {
    sources: Vec<Box<dyn MetricSource + Send>>,
}

impl Collector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a source, returning `self` so calls chain.
    pub fn with<S: MetricSource + Send + 'static>(mut self, source: S) -> Self {
        self.add(source);
        self
    }

    /// Add a source.
    pub fn add<S: MetricSource + Send + 'static>(&mut self, source: S) {
        self.sources.push(Box::new(source));
    }

    /// How many sources are registered.
    pub fn len(&self) -> usize {
        self.sources.len()
    }

    /// Whether any source is registered. A collector with none samples to an
    /// empty snapshot rather than failing.
    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// Read every source once, in insertion order.
    pub fn sample(&mut self) -> Vec<Metric> {
        self.sources.iter_mut().flat_map(|s| s.read()).collect()
    }

    /// Read every source once, stamped with the time the reading was taken.
    pub fn snapshot(&mut self) -> MetricSnapshot {
        MetricSnapshot {
            timestamp: SystemTime::now(),
            metrics: self.sample(),
        }
    }
}

/// Every metric read at one moment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSnapshot {
    pub timestamp: SystemTime,
    pub metrics: Vec<Metric>,
}

/// The reading published under this name, or `None` if nothing published it.
///
/// The same lookup as [`MetricSnapshot::get`], for a bare set of readings: what
/// [`Collector::sample`] returns, or what a single source publishes.
///
/// ```
/// use rustcat_metrics::{metric, Metric, Unit};
///
/// let readings = [Metric::new("cpu.global_usage", 12.5, Unit::Percent)];
/// assert_eq!(metric::find(&readings, "cpu.global_usage").unwrap().value, 12.5.into());
/// assert!(metric::find(&readings, "cpu.absent").is_none());
/// ```
pub fn find<'a>(readings: &'a [Metric], name: &str) -> Option<&'a Metric> {
    readings.iter().find(|metric| metric.name == name)
}

impl MetricSnapshot {
    /// The reading published under this name, or `None` if nothing published it.
    pub fn get(&self, name: &str) -> Option<&Metric> {
        find(&self.metrics, name)
    }

    /// The reading under this name, as a number.
    ///
    /// `None` covers all three ways a number can be absent — no such metric, a
    /// [`Text`](MetricValue::Text) value, and a missing source — because a
    /// caller that wants to distinguish them has [`get`](Self::get).
    pub fn value_of(&self, name: &str) -> Option<f64> {
        self.get(name)?.value.as_f64()
    }

    /// Every reading whose name begins with this prefix, in published order.
    ///
    /// The way to pick up a multi-instance group without knowing the instances
    /// in advance: `snapshot.with_prefix("net.")`.
    pub fn with_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a Metric> {
        self.metrics
            .iter()
            .filter(move |metric| metric.name.starts_with(prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(metrics: Vec<Metric>) -> FnSource<impl FnMut() -> Vec<Metric>> {
        FnSource::new(move || metrics.clone())
    }

    #[test]
    fn collector_samples_every_source_in_order() {
        let mut collector = Collector::new()
            .with(source(vec![Metric::new("a", 1u64, Unit::None)]))
            .with(source(vec![Metric::new("b", 2.5, Unit::Percent)]));

        let names: Vec<String> = collector
            .sample()
            .into_iter()
            .map(|metric| metric.name)
            .collect();
        assert_eq!(names, ["a", "b"]);
    }

    #[test]
    fn empty_collector_samples_to_nothing() {
        let mut collector = Collector::new();
        assert!(collector.is_empty());
        assert!(collector.sample().is_empty());
        assert!(collector.snapshot().metrics.is_empty());
    }

    #[test]
    fn snapshot_looks_readings_up_by_name() {
        let mut collector =
            Collector::new().with(source(vec![Metric::new("cpu.load", 42.0, Unit::Percent)]));
        let snapshot = collector.snapshot();

        assert_eq!(snapshot.value_of("cpu.load"), Some(42.0));
        assert_eq!(snapshot.value_of("cpu.missing"), None);
        assert_eq!(snapshot.get("cpu.load").unwrap().unit, Unit::Percent);
    }

    #[test]
    fn a_text_reading_has_no_numeric_value() {
        let mut collector = Collector::new().with(source(vec![Metric::new(
            "build.status",
            "passing",
            Unit::None,
        )]));
        let snapshot = collector.snapshot();

        assert_eq!(snapshot.value_of("build.status"), None);
        assert_eq!(
            snapshot.get("build.status").unwrap().value.as_str(),
            Some("passing")
        );
    }

    #[test]
    fn prefix_selects_a_group_without_knowing_its_members() {
        let mut collector = Collector::new().with(source(vec![
            Metric::new("net.en0.rx", 10u64, Unit::BytesPerSecond),
            Metric::new("net.en0.tx", 20u64, Unit::BytesPerSecond),
            Metric::new("cpu.load", 1.0, Unit::Percent),
        ]));
        let snapshot = collector.snapshot();

        let net: Vec<&str> = snapshot
            .with_prefix("net.")
            .map(|m| m.name.as_str())
            .collect();
        assert_eq!(net, ["net.en0.rx", "net.en0.tx"]);
    }

    #[test]
    fn find_reads_a_bare_set_of_readings() {
        let readings = [
            Metric::new("cpu.global_usage", 12.5, Unit::Percent),
            Metric::new("cpu.core_count", 8u64, Unit::None),
        ];

        assert_eq!(
            find(&readings, "cpu.core_count").unwrap().value,
            8u64.into()
        );
        assert!(find(&readings, "cpu.absent").is_none());
    }

    #[test]
    fn exact_counts_survive_serialization() {
        // A byte total past 2^53 is the case a single `f64` value would lose.
        let big = Metric::new("memory.total", 9_007_199_254_740_993u64, Unit::Bytes);
        let json = serde_json::to_string(&big).unwrap();
        let back: Metric = serde_json::from_str(&json).unwrap();
        assert_eq!(back, big);
    }

    /// The untagged representation has to keep a gauge a gauge: a whole-number
    /// `f64` serializes as `5.0`, which must not come back as a `Count`.
    #[test]
    fn gauges_and_counts_round_trip_to_their_own_variants() {
        for metric in [
            Metric::new("gauge", 5.0, Unit::Percent),
            Metric::new("count", 5u64, Unit::None),
            Metric::new("text", "hello", Unit::None),
        ] {
            let json = serde_json::to_string(&metric).unwrap();
            let back: Metric = serde_json::from_str(&json).unwrap();
            assert_eq!(back, metric, "round trip through {json}");
        }
    }

    #[test]
    fn units_render_the_suffix_a_reader_expects() {
        assert_eq!(Unit::Percent.suffix(), "%");
        assert_eq!(Unit::Celsius.suffix(), "°C");
        assert_eq!(Unit::None.suffix(), "");
    }
}
