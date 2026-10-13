use std::time::Duration;

/// Configuration for RPC Gateway providers and resilience mechanisms.
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub block_cache_ttl: Duration,
    pub circuit_breaker_failure_threshold: u32,
    pub circuit_breaker_success_threshold: u32,
    pub circuit_breaker_recovery_timeout: Duration,
    pub initial_cooldown_secs: u64,
    pub max_cooldown_secs: u64,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            block_cache_ttl: Duration::from_millis(500),
            circuit_breaker_failure_threshold: 3,
            circuit_breaker_success_threshold: 2,
            circuit_breaker_recovery_timeout: Duration::from_secs(10),
            initial_cooldown_secs: 10,
            max_cooldown_secs: 120,
        }
    }
}

impl GatewayConfig {
    /// Set block height micro-cache TTL.
    #[must_use]
    pub fn with_block_cache_ttl(mut self, ttl: Duration) -> Self {
        self.block_cache_ttl = ttl;
        self
    }

    /// Set circuit breaker failure and recovery thresholds.
    #[must_use]
    pub fn with_circuit_breaker(
        mut self,
        failure_threshold: u32,
        success_threshold: u32,
        recovery_timeout: Duration,
    ) -> Self {
        self.circuit_breaker_failure_threshold = failure_threshold;
        self.circuit_breaker_success_threshold = success_threshold;
        self.circuit_breaker_recovery_timeout = recovery_timeout;
        self
    }

    /// Set cooldown backoff bounds.
    #[must_use]
    pub fn with_cooldown_bounds(mut self, initial_secs: u64, max_secs: u64) -> Self {
        self.initial_cooldown_secs = initial_secs;
        self.max_cooldown_secs = max_secs;
        self
    }
}
