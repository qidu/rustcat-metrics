//! Rolling counter / sampler for rate calculations.

use std::collections::VecDeque;
use std::time::Instant;

/// Configuration for rolling window sampling
#[derive(Debug, Clone)]
pub struct SamplerConfig {
    pub window_size: usize,
    pub min_samples: usize,
}

impl Default for SamplerConfig {
    fn default() -> Self {
        Self {
            window_size: 60, // 60 samples
            min_samples: 2,  // Need at least 2 for rate
        }
    }
}

/// Rolling sampler for calculating rates (bytes/sec, packets/sec, etc.)
#[derive(Debug, Clone)]
pub struct RollingSampler<T> {
    samples: VecDeque<(Instant, T)>,
    config: SamplerConfig,
}

impl<T: Copy + std::ops::Sub<Output = T> + std::ops::Div<f64, Output = T> + Default>
    RollingSampler<T>
{
    pub fn new(config: SamplerConfig) -> Self {
        Self {
            samples: VecDeque::with_capacity(config.window_size),
            config,
        }
    }

    /// Record a new sample
    pub fn record(&mut self, value: T) {
        let now = Instant::now();
        self.samples.push_back((now, value));

        // Trim old samples beyond window
        while self.samples.len() > self.config.window_size {
            self.samples.pop_front();
        }
    }

    /// Calculate rate per second (current - oldest) / elapsed_seconds
    pub fn rate_per_sec(&self) -> Option<T> {
        if self.samples.len() < self.config.min_samples {
            return None;
        }

        let (oldest_time, oldest_value) = self.samples.front()?;
        let (newest_time, newest_value) = self.samples.back()?;

        let elapsed = newest_time.duration_since(*oldest_time).as_secs_f64();
        if elapsed <= 0.0 {
            return None;
        }

        let delta = *newest_value - *oldest_value;
        Some(delta / elapsed)
    }

    /// Get latest value
    pub fn latest(&self) -> Option<T> {
        self.samples.back().map(|(_, v)| *v)
    }

    /// Clear all samples
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// Counter source trait for types that provide counter values
pub trait CounterSource {
    type Counter: Copy
        + std::ops::Sub<Output = Self::Counter>
        + std::ops::Div<f64, Output = Self::Counter>
        + Default;

    fn read_counter(&self) -> Option<Self::Counter>;
}

/// Rolling counter that wraps a CounterSource
pub struct RollingCounter<S: CounterSource> {
    source: S,
    sampler: RollingSampler<S::Counter>,
}

impl<S: CounterSource> RollingCounter<S> {
    pub fn new(source: S, config: SamplerConfig) -> Self {
        Self {
            source,
            sampler: RollingSampler::new(config),
        }
    }

    /// Tick: read counter and record sample
    pub fn tick(&mut self) -> Option<S::Counter> {
        self.source.read_counter().map(|v| {
            self.sampler.record(v);
            v
        })
    }

    /// Get current rate per second
    pub fn rate_per_sec(&self) -> Option<S::Counter> {
        self.sampler.rate_per_sec()
    }

    pub fn latest(&self) -> Option<S::Counter> {
        self.sampler.latest()
    }
}
