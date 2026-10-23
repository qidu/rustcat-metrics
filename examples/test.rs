//! Smoke test for every metric source the crate was built with.
//!
//! The first half uses the typed API — each source returns its own struct. The
//! second half feeds the same sources plus one of your own through the generic
//! [`Collector`], which flattens everything into named [`Metric`]s.
//!
//! Each section is feature-gated, so any combination works:
//! `cargo run --example test --no-default-features --features cpu,memory`

use rustcat_metrics::{Collector, FnSource, Metric, Unit};

/// Stand-in for "any data of your own" — a counter and a gauge the crate knows
/// nothing about.
fn battery_source() -> FnSource<impl FnMut() -> Vec<Metric>> {
    let mut ticks = 0u64;
    FnSource::new(move || {
        ticks += 1;
        vec![
            Metric::new(
                "battery.charge_percent",
                87.5 - f64::from(ticks as u32 % 5),
                Unit::Percent,
            ),
            Metric::new("battery.cycles", 312 + ticks, Unit::None),
        ]
    })
}

fn main() {
    println!("=== RustCat Metrics Test ===\n");

    #[cfg(feature = "cpu")]
    {
        use rustcat_metrics::cpu::CpuSource;

        println!("--- CPU ---");
        let mut cpu = CpuSource::new();
        let cpu_metrics = cpu.refresh();
        println!("Global Usage: {:.1}%", cpu_metrics.global_usage);
        println!("Core Count: {}", cpu_metrics.core_count);
        if let Some(freq) = cpu_metrics.frequency_mhz {
            println!("Frequency: {} MHz", freq);
        }
        match cpu_metrics.temperature_celsius {
            Some(t) => println!("Temperature: {:.1}°C", t),
            None => println!("Temperature: unavailable on this platform"),
        }
        println!("Per-core: {:?}", cpu_metrics.per_core);
    }

    #[cfg(feature = "memory")]
    {
        use rustcat_metrics::memory::MemorySource;

        println!("\n--- Memory ---");
        let mut memory = MemorySource::new();
        let mem_metrics = memory.refresh();
        println!(
            "Total: {} bytes ({:.2} GB)",
            mem_metrics.total_bytes,
            mem_metrics.total_bytes as f64 / 1024.0 / 1024.0 / 1024.0
        );
        println!(
            "Used: {} bytes ({:.2} GB)",
            mem_metrics.used_bytes,
            mem_metrics.used_bytes as f64 / 1024.0 / 1024.0 / 1024.0
        );
        println!(
            "Available: {} bytes ({:.2} GB)",
            mem_metrics.available_bytes,
            mem_metrics.available_bytes as f64 / 1024.0 / 1024.0 / 1024.0
        );
        println!("Swap Total: {} bytes", mem_metrics.swap_total_bytes);
        println!("Swap Used: {} bytes", mem_metrics.swap_used_bytes);
    }

    #[cfg(feature = "disk")]
    {
        use rustcat_metrics::{disk::DiskSource, types::CollectionConfig};

        println!("\n--- Disk ---");
        let config = CollectionConfig::default();
        let mut disk = DiskSource::new(&config);
        for d in disk.refresh() {
            println!(
                "Mount: {} | Total: {} GB | Used: {} GB | Avail: {} GB | FS: {}",
                d.mount_point,
                d.total_bytes / 1024 / 1024 / 1024,
                d.used_bytes / 1024 / 1024 / 1024,
                d.available_bytes / 1024 / 1024 / 1024,
                d.filesystem
            );
        }
    }

    #[cfg(feature = "network")]
    {
        use rustcat_metrics::{network::NetworkSource, types::CollectionConfig};

        println!("\n--- Network ---");
        let config = CollectionConfig::default();
        let mut network = NetworkSource::new(&config);
        for n in network.refresh() {
            println!(
                "Interface: {} | RX: {} B/s | TX: {} B/s | Packets RX: {} | TX: {}",
                n.interface_name,
                n.rx_bytes_per_sec,
                n.tx_bytes_per_sec,
                n.rx_packets_per_sec,
                n.tx_packets_per_sec
            );
        }
    }

    println!("\n--- Generic layer ---");
    let mut collector = Collector::new().with(battery_source());
    #[cfg(feature = "cpu")]
    collector.add(rustcat_metrics::cpu::CpuSource::new());
    #[cfg(feature = "memory")]
    collector.add(rustcat_metrics::memory::MemorySource::new());

    println!("Sources: {}", collector.len());
    let snapshot = collector.snapshot();
    for metric in &snapshot.metrics {
        match &metric.value {
            rustcat_metrics::MetricValue::Text(text) => {
                println!("{} = {}", metric.name, text);
            }
            value => println!(
                "{} = {}{}",
                metric.name,
                value.as_f64().unwrap_or_default(),
                metric.unit.suffix()
            ),
        }
    }
    println!(
        "Battery readings: {}",
        snapshot.with_prefix("battery.").count()
    );
    println!("CPU usage: {:?}", snapshot.value_of("cpu.global_usage"));

    println!("\n=== Test Complete ===");
}
