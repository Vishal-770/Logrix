use logrix_chain_evm::EvmChainClient;
use logrix_core::domain::ChainId;
use logrix_resilience::CircuitBreaker;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Managed RPC Provider with circuit breaker, priority tier, and rolling latency stats.
#[derive(Clone)]
pub struct ManagedProvider {
    name: String,
    url: String,
    priority: u32,
    client: EvmChainClient,
    breaker: Arc<CircuitBreaker>,
    /// Exponential cooldown multiplier in seconds: 10, 20, 40, max 120
    cooldown_secs: Arc<AtomicU64>,
    /// Moving average latency in milliseconds
    avg_latency_ms: Arc<AtomicU64>,
}

impl ManagedProvider {
    pub fn new(
        name: impl Into<String>,
        url: impl Into<String>,
        priority: u32,
        chain_id: ChainId,
    ) -> Self {
        let url_str = url.into();
        let name_str = name.into();
        let client = EvmChainClient::new(chain_id, &url_str);
        // Failure threshold: 3, success threshold to close: 2, recovery timeout: 10s
        let breaker = Arc::new(CircuitBreaker::new(3, 2, Duration::from_secs(10)));

        Self {
            name: name_str,
            url: url_str,
            priority,
            client,
            breaker,
            cooldown_secs: Arc::new(AtomicU64::new(10)),
            avg_latency_ms: Arc::new(AtomicU64::new(50)), // seed initial 50ms
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn priority(&self) -> u32 {
        self.priority
    }

    pub fn client(&self) -> &EvmChainClient {
        &self.client
    }

    pub fn breaker(&self) -> &CircuitBreaker {
        &self.breaker
    }

    /// Check if this provider is currently available to serve requests.
    pub async fn is_healthy(&self) -> bool {
        self.breaker.can_execute().await
    }

    /// Record a successful call with measured roundtrip latency.
    pub async fn record_success(&self, latency: Duration) {
        self.breaker.on_success().await;
        // Reset exponential cooldown back to initial 10s
        self.cooldown_secs.store(10, Ordering::Relaxed);

        // Exponential moving average for latency: 80% old + 20% new
        let ms = latency.as_millis() as u64;
        let old = self.avg_latency_ms.load(Ordering::Relaxed);
        let updated = ((old * 4) + ms) / 5;
        self.avg_latency_ms.store(updated, Ordering::Relaxed);
    }

    /// Record a failed call or 429 rate limit.
    pub async fn record_failure(&self) {
        self.breaker.on_failure().await;

        // Increment exponential cooldown: 10 -> 20 -> 40 -> 80 -> max 120s
        let current = self.cooldown_secs.load(Ordering::Relaxed);
        let next = (current * 2).min(120);
        self.cooldown_secs.store(next, Ordering::Relaxed);
    }

    /// Get current rolling average latency in milliseconds.
    pub fn avg_latency_ms(&self) -> u64 {
        self.avg_latency_ms.load(Ordering::Relaxed)
    }

    /// Get current cooldown duration with full jitter to prevent thundering herd recovery storms.
    pub fn cooldown_duration(&self) -> Duration {
        use rand::Rng;
        let base_secs = self.cooldown_secs.load(Ordering::Relaxed);
        let mut rng = rand::thread_rng();
        // Decorrelated Full Jitter: [base/2, base] + random millisecond offset
        let min_millis = (base_secs * 1000) / 2;
        let max_millis = base_secs * 1000;
        let jittered_millis = rng.gen_range(min_millis..=max_millis);
        Duration::from_millis(jittered_millis)
    }
}
