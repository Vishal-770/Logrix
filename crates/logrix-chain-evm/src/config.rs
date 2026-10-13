use std::time::Duration;

/// Configuration options for EVM JSON-RPC client connections.
#[derive(Debug, Clone)]
pub struct EvmClientConfig {
    pub timeout: Duration,
    pub pool_idle_timeout: Duration,
    pub pool_max_idle_per_host: usize,
    pub chunk_initial: u64,
    pub chunk_min: u64,
    pub chunk_max: u64,
    pub chunk_additive_step: u64,
    pub chunk_success_threshold: u64,
    pub chunk_target_latency: Duration,
    pub rate_limit_capacity: u32,
    pub rate_limit_refill_rate: f64,
    pub max_retries: u32,
    pub retry_initial_backoff: Duration,
    pub retry_max_backoff: Duration,
}

impl Default for EvmClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(15),
            pool_idle_timeout: Duration::from_secs(90),
            pool_max_idle_per_host: 20,
            chunk_initial: 500,
            chunk_min: 5,
            chunk_max: 2000,
            chunk_additive_step: 50,
            chunk_success_threshold: 3,
            chunk_target_latency: Duration::from_millis(800),
            rate_limit_capacity: 100,
            rate_limit_refill_rate: 50.0,
            max_retries: 3,
            retry_initial_backoff: Duration::from_millis(200),
            retry_max_backoff: Duration::from_secs(5),
        }
    }
}

impl EvmClientConfig {
    /// Create default configuration with custom chunk bounds.
    #[must_use]
    pub fn with_chunk_bounds(mut self, initial: u64, min: u64, max: u64) -> Self {
        self.chunk_initial = initial;
        self.chunk_min = min;
        self.chunk_max = max;
        self
    }

    /// Set HTTP timeout duration.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set rate limiter parameters.
    #[must_use]
    pub fn with_rate_limit(mut self, capacity: u32, refill_rate: f64) -> Self {
        self.rate_limit_capacity = capacity;
        self.rate_limit_refill_rate = refill_rate;
        self
    }

    /// Set retry parameters.
    #[must_use]
    pub fn with_retries(mut self, max: u32, initial: Duration, max_backoff: Duration) -> Self {
        self.max_retries = max;
        self.retry_initial_backoff = initial;
        self.retry_max_backoff = max_backoff;
        self
    }
}
