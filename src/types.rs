//! Shared types and data structures for metrics.

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// A single metric sample with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample<T> {
    pub value: T,
    pub timestamp: SystemTime,
}

/// CPU usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuMetrics {
    pub global_usage: f32,  // 0.0 - 100.0
    pub per_core: Vec<f32>, // Per-core usage
    pub core_count: usize,
    pub frequency_mhz: Option<u64>,
    pub temperature_celsius: Option<f32>,
}

/// Memory metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub swap_total_bytes: u64,
    pub swap_used_bytes: u64,
}

/// Disk metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMetrics {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub read_bytes_per_sec: u64,
    pub write_bytes_per_sec: u64,
    pub read_iops: u64,
    pub write_iops: u64,
    pub mount_point: String,
    pub filesystem: String,
}

/// Network metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub interface_name: String,
    pub rx_bytes_per_sec: u64,
    pub tx_bytes_per_sec: u64,
    pub rx_packets_per_sec: u64,
    pub tx_packets_per_sec: u64,
    pub rx_errors_per_sec: u64,
    pub tx_errors_per_sec: u64,
    pub speed_mbps: Option<u64>,
}

/// All metrics combined for a single snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub timestamp: SystemTime,
    pub cpu: Option<CpuMetrics>,
    pub memory: Option<MemoryMetrics>,
    pub disks: Vec<DiskMetrics>,
    pub networks: Vec<NetworkMetrics>,
}

impl SystemSnapshot {
    pub fn new() -> Self {
        Self {
            timestamp: SystemTime::now(),
            cpu: None,
            memory: None,
            disks: Vec::new(),
            networks: Vec::new(),
        }
    }
}

impl Default for SystemSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for metric collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionConfig {
    pub cpu: bool,
    pub memory: bool,
    pub disk: bool,
    pub network: bool,
    pub interval_ms: u64,
    pub disk_mount_points: Vec<String>,
    pub network_interfaces: Vec<String>,
}

impl Default for CollectionConfig {
    fn default() -> Self {
        Self {
            cpu: true,
            memory: true,
            disk: true,
            network: true,
            interval_ms: 1000,
            disk_mount_points: vec!["/".to_string(), "C:\\".to_string()],
            network_interfaces: Vec::new(), // empty = all
        }
    }
}
