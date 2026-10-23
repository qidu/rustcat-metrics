//! Read system resources — and any data of your own — as metrics.
//!
//! Two ways in, depending on what the caller has:
//!
//! - **Typed sources** for the system resources: [`CpuSource`], [`MemorySource`],
//!   [`DiskSource`] and [`NetworkSource`]. Each returns a struct of that
//!   resource's readings, and each is created once and refreshed repeatedly, so
//!   the platform handles are opened a single time.
//! - **The generic layer** in [`metric`], for everything else: [`Metric`],
//!   [`MetricSource`] and [`Collector`]. The typed sources implement
//!   [`MetricSource`] too, so a consumer that only wants one shape can stay in
//!   the generic layer throughout.
//!
//! Every metric module is behind a feature of the same name — `cpu`, `memory`,
//! `disk`, `network`, all on by default — so a build can take only what it uses.
//! The generic layer is always compiled, and is all a crate needs to read and
//! publish metrics of its own.
//!
//! # Examples
//!
//! A typed source, refreshed in a loop:
//!
//! ```no_run
//! # #[cfg(feature = "cpu")]
//! # fn main() {
//! use rustcat_metrics::cpu::CpuSource;
//!
//! let mut cpu = CpuSource::new();
//! loop {
//!     let usage = cpu.refresh().global_usage;
//!     println!("cpu: {usage:.1}%");
//!     # break;
//! }
//! # }
//! # #[cfg(not(feature = "cpu"))]
//! # fn main() {}
//! ```
//!
//! Your own data, gathered with the system's into one snapshot:
//!
//! ```
//! use rustcat_metrics::metric::FnSource;
//! use rustcat_metrics::{Collector, Metric, Unit};
//!
//! let mut collector = Collector::new().with(FnSource::new(|| {
//!     vec![Metric::new("app.open_documents", 7u64, Unit::None)]
//! }));
//!
//! let snapshot = collector.snapshot();
//! assert_eq!(snapshot.value_of("app.open_documents"), Some(7.0));
//! ```

#[cfg(feature = "cpu")]
pub mod cpu;
#[cfg(feature = "disk")]
pub mod disk;
#[cfg(feature = "memory")]
pub mod memory;
#[cfg(feature = "network")]
pub mod network;

pub mod metric;
pub mod sampler;
pub mod types;

#[cfg(feature = "cpu")]
pub use cpu::CpuSource;
#[cfg(feature = "disk")]
pub use disk::DiskSource;
#[cfg(feature = "memory")]
pub use memory::MemorySource;
#[cfg(feature = "network")]
pub use network::NetworkSource;

pub use metric::{Collector, FnSource, Metric, MetricSnapshot, MetricSource, MetricValue, Unit};
pub use sampler::{CounterSource, RollingCounter, RollingSampler, SamplerConfig};
pub use types::*;
