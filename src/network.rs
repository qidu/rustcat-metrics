//! Network metrics source using sysinfo
//!
//! A system has one set of counters per interface, so as a [`MetricSource`] every
//! reading carries its interface in the name — `net.en0.rx_bytes_per_sec`. See
//! [`crate::disk`] for the same convention spelled out.
//!
//! Published per interface:
//!
//! | name | unit |
//! | --- | --- |
//! | `net.<interface>.rx_bytes_per_sec` | [`BytesPerSecond`](Unit::BytesPerSecond) |
//! | `net.<interface>.tx_bytes_per_sec` | [`BytesPerSecond`](Unit::BytesPerSecond) |
//! | `net.<interface>.rx_packets_per_sec` | [`None`](Unit::None) |
//! | `net.<interface>.tx_packets_per_sec` | [`None`](Unit::None) |
//! | `net.<interface>.rx_errors_per_sec` | [`None`](Unit::None) |
//! | `net.<interface>.tx_errors_per_sec` | [`None`](Unit::None) |
//!
//! The first read of an interface has no earlier counter to difference against,
//! so it publishes zero rates and the real numbers start with the second read.

use crate::metric::{Metric, MetricSource, Unit};
use crate::sampler::{RollingSampler, SamplerConfig};
use crate::types::{CollectionConfig, NetworkMetrics};
use std::collections::HashMap;
use sysinfo::Networks;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct NetworkCounters {
    rx_bytes: u64,
    tx_bytes: u64,
    rx_packets: u64,
    tx_packets: u64,
    rx_errors: u64,
    tx_errors: u64,
}

impl std::ops::Sub for NetworkCounters {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        Self {
            rx_bytes: self.rx_bytes.saturating_sub(other.rx_bytes),
            tx_bytes: self.tx_bytes.saturating_sub(other.tx_bytes),
            rx_packets: self.rx_packets.saturating_sub(other.rx_packets),
            tx_packets: self.tx_packets.saturating_sub(other.tx_packets),
            rx_errors: self.rx_errors.saturating_sub(other.rx_errors),
            tx_errors: self.tx_errors.saturating_sub(other.tx_errors),
        }
    }
}

impl std::ops::Div<f64> for NetworkCounters {
    type Output = Self;

    fn div(self, rhs: f64) -> Self::Output {
        Self {
            rx_bytes: (self.rx_bytes as f64 / rhs) as u64,
            tx_bytes: (self.tx_bytes as f64 / rhs) as u64,
            rx_packets: (self.rx_packets as f64 / rhs) as u64,
            tx_packets: (self.tx_packets as f64 / rhs) as u64,
            rx_errors: (self.rx_errors as f64 / rhs) as u64,
            tx_errors: (self.tx_errors as f64 / rhs) as u64,
        }
    }
}

pub struct NetworkSource {
    networks: Networks,
    interfaces: Vec<String>,
    samplers: HashMap<String, RollingSampler<NetworkCounters>>,
}

impl NetworkSource {
    pub fn new(config: &CollectionConfig) -> Self {
        let networks = Networks::new_with_refreshed_list();
        Self {
            networks,
            interfaces: config.network_interfaces.clone(),
            samplers: HashMap::new(),
        }
    }

    pub fn refresh(&mut self) -> Vec<NetworkMetrics> {
        self.networks.refresh(false);

        let mut metrics = Vec::new();

        for (interface_name, data) in self.networks.list() {
            // Filter by configured interfaces if specified
            if !self.interfaces.is_empty() && !self.interfaces.contains(interface_name) {
                continue;
            }

            let current = NetworkCounters {
                rx_bytes: data.received(),
                tx_bytes: data.transmitted(),
                rx_packets: data.packets_received(),
                tx_packets: data.packets_transmitted(),
                rx_errors: data.errors_on_received(),
                tx_errors: data.errors_on_transmitted(),
            };

            let sampler = self
                .samplers
                .entry(interface_name.clone())
                .or_insert_with(|| RollingSampler::new(SamplerConfig::default()));

            sampler.record(current);

            let rate = sampler.rate_per_sec().unwrap_or_default();

            metrics.push(NetworkMetrics {
                interface_name: interface_name.clone(),
                rx_bytes_per_sec: rate.rx_bytes,
                tx_bytes_per_sec: rate.tx_bytes,
                rx_packets_per_sec: rate.rx_packets,
                tx_packets_per_sec: rate.tx_packets,
                rx_errors_per_sec: rate.rx_errors,
                tx_errors_per_sec: rate.tx_errors,
                speed_mbps: None, // sysinfo doesn't provide link speed
            });
        }

        metrics
    }
}

impl MetricSource for NetworkSource {
    fn read(&mut self) -> Vec<Metric> {
        let mut out = Vec::new();
        for metrics in self.refresh() {
            let iface = &metrics.interface_name;
            let named = |reading: &str| format!("net.{iface}.{reading}");
            out.push(Metric::new(
                named("rx_bytes_per_sec"),
                metrics.rx_bytes_per_sec,
                Unit::BytesPerSecond,
            ));
            out.push(Metric::new(
                named("tx_bytes_per_sec"),
                metrics.tx_bytes_per_sec,
                Unit::BytesPerSecond,
            ));
            out.push(Metric::new(
                named("rx_packets_per_sec"),
                metrics.rx_packets_per_sec,
                Unit::None,
            ));
            out.push(Metric::new(
                named("tx_packets_per_sec"),
                metrics.tx_packets_per_sec,
                Unit::None,
            ));
            out.push(Metric::new(
                named("rx_errors_per_sec"),
                metrics.rx_errors_per_sec,
                Unit::None,
            ));
            out.push(Metric::new(
                named("tx_errors_per_sec"),
                metrics.tx_errors_per_sec,
                Unit::None,
            ));
        }
        out
    }
}

impl Default for NetworkSource {
    fn default() -> Self {
        let config = CollectionConfig::default();
        Self::new(&config)
    }
}
