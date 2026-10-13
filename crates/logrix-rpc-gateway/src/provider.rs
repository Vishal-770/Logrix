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
    /// Exponential cooldown multiplier in seconds
    cooldown_secs: Arc<AtomicU64>,
    initial_cooldown_secs: u64,
    max_cooldown_secs: u64,
    /// Moving average latency in milliseconds
    avg_latency_ms: Arc<AtomicU64>,
    /// Highest block height observed on this provider
    latest_block: Arc<AtomicU64>,
}

impl ManagedProvider {
    pub fn new(
        name: impl Into<String>,
        url: impl Into<String>,
        priority: u32,
        chain_id: ChainId,
    ) -> Self {
        Self::new_with_config(
            name,
            url,
            priority,
            chain_id,
            &crate::config::GatewayConfig::default(),
            None,
        )
    }

    pub fn new_with_config(
        name: impl Into<String>,
        url: impl Into<String>,
        priority: u32,
        chain_id: ChainId,
        cfg: &crate::config::GatewayConfig,
        evm_cfg: Option<logrix_chain_evm::EvmClientConfig>,
    ) -> Self {
        let url_str = url.into();
        let name_str = name.into();
        let client = match evm_cfg {
            Some(ec) => EvmChainClient::new_supervised_with_config(chain_id, &url_str, ec),
            None => EvmChainClient::new_supervised(chain_id, &url_str),
        };
        let breaker = Arc::new(CircuitBreaker::new(
            cfg.circuit_breaker_failure_threshold,
            cfg.circuit_breaker_success_threshold,
            cfg.circuit_breaker_recovery_timeout,
        ));

        Self {
            name: name_str,
            url: url_str,
            priority,
            client,
            breaker,
            cooldown_secs: Arc::new(AtomicU64::new(cfg.initial_cooldown_secs)),
            initial_cooldown_secs: cfg.initial_cooldown_secs,
            max_cooldown_secs: cfg.max_cooldown_secs,
            avg_latency_ms: Arc::new(AtomicU64::new(50)),
            latest_block: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Return redacted URL suitable for safe tracing and logging.
    pub fn masked_url(&self) -> String {
        redact_rpc_url(&self.url)
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
        // Reset exponential cooldown back to configured initial
        self.cooldown_secs
            .store(self.initial_cooldown_secs, Ordering::Relaxed);

        // Exponential moving average for latency: 80% old + 20% new
        let ms = latency.as_millis() as u64;
        let old = self.avg_latency_ms.load(Ordering::Relaxed);
        let updated = ((old * 4) + ms) / 5;
        self.avg_latency_ms.store(updated, Ordering::Relaxed);
    }

    /// Record a failed call or 429 rate limit.
    pub async fn record_failure(&self) {
        self.breaker.on_failure().await;

        // Increment exponential cooldown: double up to max
        let current = self.cooldown_secs.load(Ordering::Relaxed);
        let next = (current * 2).min(self.max_cooldown_secs);
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

    /// Record newly observed block height from this provider.
    pub fn record_block_height(&self, height: u64) {
        let current = self.latest_block.load(Ordering::Relaxed);
        if height > current {
            self.latest_block.store(height, Ordering::Relaxed);
        }
    }

    /// Retrieve the highest block height observed from this provider.
    pub fn latest_block(&self) -> u64 {
        self.latest_block.load(Ordering::Relaxed)
    }
}

/// Redact embedded basic auth or path keys from RPC endpoint URLs.
#[must_use]
pub fn redact_rpc_url(raw_url: &str) -> String {
    let mut sanitized = raw_url.to_string();
    if let Some(proto_end) = sanitized.find("://") {
        let after_proto = proto_end + 3;
        if let Some(at_idx) = sanitized[after_proto..].find('@') {
            let at_abs = after_proto + at_idx;
            sanitized = format!(
                "{}***@{}",
                &sanitized[..after_proto],
                &sanitized[at_abs + 1..]
            );
        }
    }
    sanitized
}
