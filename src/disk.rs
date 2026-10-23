//! Disk metrics source using sysinfo
//!
//! A system has one disk per mount point, so as a [`MetricSource`] every reading
//! carries its mount point in the name — `disk./.total_bytes`,
//! `disk.C:\.used_bytes`. Split on the *last* dot to recover the reading, on the
//! first to group by mount point, or use
//! [`MetricSnapshot::with_prefix`](crate::metric::MetricSnapshot::with_prefix)
//! to take a whole group at once.
//!
//! Published per mount point:
//!
//! | name | unit |
//! | --- | --- |
//! | `disk.<mount>.total_bytes` | [`Bytes`](Unit::Bytes) |
//! | `disk.<mount>.used_bytes` | [`Bytes`](Unit::Bytes) |
//! | `disk.<mount>.available_bytes` | [`Bytes`](Unit::Bytes) |
//! | `disk.<mount>.read_bytes_per_sec` | [`BytesPerSecond`](Unit::BytesPerSecond) |
//! | `disk.<mount>.write_bytes_per_sec` | [`BytesPerSecond`](Unit::BytesPerSecond) |
//! | `disk.<mount>.read_iops` | [`None`](Unit::None) |
//! | `disk.<mount>.write_iops` | [`None`](Unit::None) |
//! | `disk.<mount>.filesystem` | [`None`](Unit::None), a [`Text`](crate::MetricValue::Text) reading |

use crate::metric::{Metric, MetricSource, Unit};
use crate::types::{CollectionConfig, DiskMetrics};
use sysinfo::{DiskRefreshKind, Disks};

pub struct DiskSource {
    disks: Disks,
    mount_points: Vec<String>,
}

impl DiskSource {
    pub fn new(config: &CollectionConfig) -> Self {
        // Not `mut`: the list is only ever refreshed through `self.disks` below.
        let disks = Disks::new_with_refreshed_list_specifics(DiskRefreshKind::everything());
        Self {
            disks,
            mount_points: config.disk_mount_points.clone(),
        }
    }

    pub fn refresh(&mut self) -> Vec<DiskMetrics> {
        self.disks
            .refresh_specifics(false, DiskRefreshKind::everything());

        let mut metrics = Vec::new();

        for disk in self.disks.list() {
            let mount_point = disk.mount_point().to_string_lossy().to_string();

            // Filter by configured mount points if specified
            if !self.mount_points.is_empty()
                && !self
                    .mount_points
                    .iter()
                    .any(|mp| mount_point.starts_with(mp))
            {
                continue;
            }

            let total = disk.total_space();
            let available = disk.available_space();
            let used = total.saturating_sub(available);

            // sysinfo doesn't provide IO stats directly on all platforms
            // These would need platform-specific code (I/O counters)
            metrics.push(DiskMetrics {
                total_bytes: total,
                used_bytes: used,
                available_bytes: available,
                read_bytes_per_sec: 0,
                write_bytes_per_sec: 0,
                read_iops: 0,
                write_iops: 0,
                mount_point,
                filesystem: disk.file_system().to_string_lossy().to_string(),
            });
        }

        metrics
    }
}

impl Default for DiskSource {
    fn default() -> Self {
        let config = CollectionConfig::default();
        Self::new(&config)
    }
}

impl MetricSource for DiskSource {
    fn read(&mut self) -> Vec<Metric> {
        let mut out = Vec::new();

        for disk in self.refresh() {
            let mount = &disk.mount_point;
            // Borrows `mount_point` only, so the `filesystem` reading below can
            // still be moved out of `disk`.
            let named = |reading: &str| format!("disk.{mount}.{reading}");

            out.push(Metric::new(
                named("total_bytes"),
                disk.total_bytes,
                Unit::Bytes,
            ));
            out.push(Metric::new(
                named("used_bytes"),
                disk.used_bytes,
                Unit::Bytes,
            ));
            out.push(Metric::new(
                named("available_bytes"),
                disk.available_bytes,
                Unit::Bytes,
            ));
            out.push(Metric::new(
                named("read_bytes_per_sec"),
                disk.read_bytes_per_sec,
                Unit::BytesPerSecond,
            ));
            out.push(Metric::new(
                named("write_bytes_per_sec"),
                disk.write_bytes_per_sec,
                Unit::BytesPerSecond,
            ));
            out.push(Metric::new(named("read_iops"), disk.read_iops, Unit::None));
            out.push(Metric::new(
                named("write_iops"),
                disk.write_iops,
                Unit::None,
            ));
            out.push(Metric::new(
                named("filesystem"),
                disk.filesystem,
                Unit::None,
            ));
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every mount that reports storage should publish the full set, with the
    /// mount point in each name.
    #[test]
    fn every_mount_publishes_its_whole_set() {
        let mut disk = DiskSource::default();
        let metrics = disk.read();
        let mounts = disk.refresh().len();

        assert_eq!(metrics.len(), mounts * 8, "eight readings per mount point");

        for metric in &metrics {
            assert!(
                metric.name.starts_with("disk."),
                "{} is outside the disk namespace",
                metric.name
            );
        }

        // The instance is recoverable, which is the point of the convention.
        if let Some(one) = metrics.first() {
            let mount = one
                .name
                .strip_prefix("disk.")
                .and_then(|rest| rest.split('.').next())
                .expect("a mount point follows the namespace");
            assert!(!mount.is_empty());
        }
    }

    #[test]
    fn filesystem_is_published_as_text() {
        let mut disk = DiskSource::default();
        for metric in disk.read() {
            if metric.name.ends_with(".filesystem") {
                assert!(
                    metric.value.as_str().is_some(),
                    "{} must be text",
                    metric.name
                );
            }
        }
    }
}
