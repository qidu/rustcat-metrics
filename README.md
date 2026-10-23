# rustcat-metrics

Read system resources — CPU, memory, disk, network — **and any data of your own** as metrics.

A small library with no async runtime and no global state. Sources are created once
and refreshed repeatedly, so platform handles are opened a single time.

## Install

```toml
[dependencies]
rustcat-metrics = "0.1"
```

Only some of it? Turn off the defaults and pick what you need — the generic layer
is always compiled:

```toml
[dependencies]
rustcat-metrics = { version = "0.1", default-features = false }
```

## Two ways in

### Typed sources

Each system resource has its own type returning a struct of that resource's
readings. Use this when you want a specific number and know where it comes from.

```rust
use rustcat_metrics::CpuSource;

let mut cpu = CpuSource::new();
loop {
    let usage = cpu.refresh().global_usage;
    println!("cpu: {usage:.1}%");
    std::thread::sleep(std::time::Duration::from_secs(1));
}
```

### The generic layer

Everything the typed sources read is also available as a named [`Metric`], and so
is anything you have. One consumer can then handle every reading the same way,
whether it came from this crate or from your application.

```rust
use rustcat_metrics::{Collector, CpuSource, FnSource, Metric, Unit};

let mut collector = Collector::new()
    .with(CpuSource::new())
    .with(FnSource::new(|| {
        vec![Metric::new("app.open_documents", 7u64, Unit::None)]
    }));

let snapshot = collector.snapshot();
println!("cpu: {:?}", snapshot.value_of("cpu.global_usage"));
println!("docs open: {:?}", snapshot.value_of("app.open_documents"));
```

`FnSource` wraps a closure, for data already at hand. For anything more involved,
implement `MetricSource` — one method:

```rust
use rustcat_metrics::{Metric, MetricSource, Unit};
use std::sync::{Arc, Mutex};

struct QueueDepth {
    queue: Arc<Mutex<Vec<Job>>>,
}

impl MetricSource for QueueDepth {
    fn read(&mut self) -> Vec<Metric> {
        let depth = self.queue.lock().expect("queue poisoned").len() as u64;
        vec![Metric::new("build.queue_depth", depth, Unit::None)]
    }
}
```

## What the sources publish

Every source puts its namespace first and its instance second, so a metric name is
unique across the whole snapshot: `net.en0.rx_bytes_per_sec`, `disk./.used_bytes`.

| Source | Metrics |
| --- | --- |
| `CpuSource` | `cpu.global_usage`, `cpu.core_count`, `cpu.frequency`, `cpu.temperature`, `cpu.core.<n>.usage` |
| `MemorySource` | `memory.total_bytes`, `memory.used_bytes`, `memory.available_bytes`, `memory.used_percent`, `memory.swap_total_bytes`, `memory.swap_used_bytes` |
| `DiskSource` | per mount: `disk.<mount>.total_bytes`, `.used_bytes`, `.available_bytes`, `.read_bytes_per_sec`, `.write_bytes_per_sec`, `.read_iops`, `.write_iops`, `.filesystem` |
| `NetworkSource` | per interface: `net.<iface>.rx_bytes_per_sec`, `.tx_bytes_per_sec`, `.rx_packets_per_sec`, `.tx_packets_per_sec`, `.rx_errors_per_sec`, `.tx_errors_per_sec` |

A rate is computed from the difference between reads, so the first read of a
counter source reports zero and the real numbers start with the second.

Pick up a whole multi-instance group without knowing its members in advance:

```rust
for metric in snapshot.with_prefix("net.") {
    println!("{} = {:?}{}", metric.name, metric.value, metric.unit.suffix());
}
```

## Values and units

`MetricValue` is `Count(u64)`, `Gauge(f64)` or `Text(String)`. Counts are kept
separate from gauges so an exact byte total survives serialization — `f64` loses
integers past 2^53. `Unit` (`None`, `Percent`, `Bytes`, `BytesPerSecond`,
`Hertz`, `Celsius`) tells a consumer how to render a reading without knowing which
source produced it.

## Features

| Feature | Default | Gates |
| --- | --- | --- |
| `cpu` | yes | `cpu` module and `CpuSource` |
| `memory` | yes | `memory` module and `MemorySource` |
| `disk` | yes | `disk` module and `DiskSource` |
| `network` | yes | `network` module and `NetworkSource` |

```sh
cargo run --example test                                  # everything
cargo run --example test --no-default-features --features cpu
```

## Platform support

System readings come from [`sysinfo`](https://crates.io/crates/sysinfo):
Linux, macOS, Windows and the BSDs. Some readings do not exist everywhere and are
reported as absent rather than guessed — `cpu.frequency` and `cpu.temperature` are
omitted where the platform does not provide them.

## License

MIT — see [LICENSE](LICENSE).
