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

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
pub const DEFAULT_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
pub const DEFAULT_POOL_MAX_IDLE_PER_HOST: usize = 20;
pub const DEFAULT_CHUNK_INITIAL: u64 = 500;
pub const DEFAULT_CHUNK_MIN: u64 = 5;
pub const DEFAULT_CHUNK_MAX: u64 = 2000;
pub const DEFAULT_CHUNK_ADDITIVE_STEP: u64 = 50;
pub const DEFAULT_CHUNK_SUCCESS_THRESHOLD: u64 = 3;
pub const DEFAULT_CHUNK_TARGET_LATENCY: Duration = Duration::from_millis(800);
pub const DEFAULT_RATE_LIMIT_CAPACITY: u32 = 100;
pub const DEFAULT_RATE_LIMIT_REFILL_RATE: f64 = 50.0;
pub const DEFAULT_MAX_RETRIES: u32 = 3;
pub const DEFAULT_RETRY_INITIAL_BACKOFF: Duration = Duration::from_millis(200);
pub const DEFAULT_RETRY_MAX_BACKOFF: Duration = Duration::from_secs(5);

impl Default for EvmClientConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            pool_idle_timeout: DEFAULT_POOL_IDLE_TIMEOUT,
            pool_max_idle_per_host: DEFAULT_POOL_MAX_IDLE_PER_HOST,
            chunk_initial: DEFAULT_CHUNK_INITIAL,
            chunk_min: DEFAULT_CHUNK_MIN,
            chunk_max: DEFAULT_CHUNK_MAX,
            chunk_additive_step: DEFAULT_CHUNK_ADDITIVE_STEP,
            chunk_success_threshold: DEFAULT_CHUNK_SUCCESS_THRESHOLD,
            chunk_target_latency: DEFAULT_CHUNK_TARGET_LATENCY,
            rate_limit_capacity: DEFAULT_RATE_LIMIT_CAPACITY,
            rate_limit_refill_rate: DEFAULT_RATE_LIMIT_REFILL_RATE,
            max_retries: DEFAULT_MAX_RETRIES,
            retry_initial_backoff: DEFAULT_RETRY_INITIAL_BACKOFF,
            retry_max_backoff: DEFAULT_RETRY_MAX_BACKOFF,
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

    /// Set adaptive growth parameters.
    #[must_use]
    pub fn with_growth_params(
        mut self,
        additive_step: u64,
        success_threshold: u64,
        target_latency: Duration,
    ) -> Self {
        self.chunk_additive_step = additive_step;
        self.chunk_success_threshold = success_threshold;
        self.chunk_target_latency = target_latency;
        self
    }

    /// Set HTTP timeout duration.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set connection pool parameters.
    #[must_use]
    pub fn with_pool_config(mut self, idle_timeout: Duration, max_idle_per_host: usize) -> Self {
        self.pool_idle_timeout = idle_timeout;
        self.pool_max_idle_per_host = max_idle_per_host;
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
