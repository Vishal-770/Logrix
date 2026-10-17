use std::time::Duration;

/// Configuration for CircuitBreaker behavior and thresholds.
#[derive(Debug, Clone)]
pub struct CircuitConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub reset_timeout: Duration,
    pub half_open_max_probes: Option<u32>,
    pub failure_window: Option<Duration>,
}

impl Default for CircuitConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            reset_timeout: Duration::from_secs(30),
            half_open_max_probes: None,
            failure_window: None,
        }
    }
}

impl CircuitConfig {
    pub fn new(failure_threshold: u32, success_threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            reset_timeout,
            half_open_max_probes: None,
            failure_window: None,
        }
    }

    pub fn builder() -> CircuitConfigBuilder {
        CircuitConfigBuilder::default()
    }

    pub fn effective_half_open_probes(&self) -> u32 {
        self.half_open_max_probes.unwrap_or(self.success_threshold)
    }
}

/// Fluent builder for constructing `CircuitConfig`.
#[derive(Debug, Default, Clone)]
pub struct CircuitConfigBuilder {
    config: CircuitConfig,
}

impl CircuitConfigBuilder {
    pub fn failure_threshold(mut self, threshold: u32) -> Self {
        self.config.failure_threshold = threshold;
        self
    }

    pub fn success_threshold(mut self, threshold: u32) -> Self {
        self.config.success_threshold = threshold;
        self
    }

    pub fn reset_timeout(mut self, timeout: Duration) -> Self {
        self.config.reset_timeout = timeout;
        self
    }

    pub fn half_open_max_probes(mut self, max_probes: u32) -> Self {
        self.config.half_open_max_probes = Some(max_probes);
        self
    }

    pub fn failure_window(mut self, window: Duration) -> Self {
        self.config.failure_window = Some(window);
        self
    }

    pub fn build(self) -> CircuitConfig {
        self.config
    }
}
