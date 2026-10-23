use crate::chunker::AdaptiveChunker;
use crate::config::EvmClientConfig;
use logrix_core::domain::ChainId;
use logrix_resilience::{RateLimiter, RetryPolicy};
use reqwest::Client;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

/// High-performance JSON-RPC client implementing `ChainPort`.
///
/// Features adaptive AIMD chunking, token bucket rate limiting, and exponential retry.
#[derive(Clone)]
pub struct EvmChainClient {
    pub(crate) chain_id: ChainId,
    pub(crate) rpc_url: String,
    pub(crate) http_client: Client,
    pub(crate) rpc_id: Arc<AtomicU64>,
    pub(crate) chunker: Arc<AdaptiveChunker>,
    pub(crate) rate_limiter: Arc<RateLimiter>,
    pub(crate) retry_policy: RetryPolicy,
}

impl EvmChainClient {
    /// Create client with default production configuration.
    pub fn new(chain_id: ChainId, rpc_url: impl Into<String>) -> Self {
        Self::new_with_config(chain_id, rpc_url, EvmClientConfig::default())
    }

    /// Create client with custom fine-grained configuration options.
    pub fn new_with_config(
        chain_id: ChainId,
        rpc_url: impl Into<String>,
        cfg: EvmClientConfig,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(cfg.timeout)
            .pool_idle_timeout(cfg.pool_idle_timeout)
            .pool_max_idle_per_host(cfg.pool_max_idle_per_host)
            .build()
            .expect("build reqwest client");

        let chunker = AdaptiveChunker::with_growth_params(
            cfg.chunk_initial,
            cfg.chunk_min,
            cfg.chunk_max,
            cfg.chunk_additive_step,
            cfg.chunk_success_threshold,
            cfg.chunk_target_latency,
        );

        Self {
            chain_id,
            rpc_url: rpc_url.into(),
            http_client,
            rpc_id: Arc::new(AtomicU64::new(1)),
            chunker: Arc::new(chunker),
            rate_limiter: Arc::new(RateLimiter::new(
                cfg.rate_limit_capacity,
                cfg.rate_limit_refill_rate,
            )),
            retry_policy: RetryPolicy::new(
                cfg.max_retries,
                cfg.retry_initial_backoff,
                cfg.retry_max_backoff,
            ),
        }
    }

    /// Supervised client for Multi-Provider Gateways: fails fast to allow instant fallback.
    pub fn new_supervised(chain_id: ChainId, rpc_url: impl Into<String>) -> Self {
        Self::new_supervised_with_config(chain_id, rpc_url, EvmClientConfig::default())
    }

    /// Supervised client with custom base configuration.
    pub fn new_supervised_with_config(
        chain_id: ChainId,
        rpc_url: impl Into<String>,
        mut cfg: EvmClientConfig,
    ) -> Self {
        cfg.max_retries = 1;
        cfg.retry_initial_backoff = Duration::from_millis(50);
        cfg.retry_max_backoff = Duration::from_millis(100);
        Self::new_with_config(chain_id, rpc_url, cfg)
    }

    /// Access the internal chain ID.
    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Access the internal adaptive block chunker.
    pub fn chunker(&self) -> &AdaptiveChunker {
        &self.chunker
    }

    /// Execute a single eth_getLogs JSON-RPC call.
    pub(crate) async fn fetch_rpc_logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &[alloy_primitives::Address],
    ) -> logrix_core::error::LogrixResult<Vec<logrix_core::domain::EventLog>> {
        let mut filter = serde_json::json!({
            "fromBlock": format!("0x{:x}", from_block),
            "toBlock": format!("0x{:x}", to_block),
        });

        if !addresses.is_empty() {
            let addr_strs: Vec<String> = addresses.iter().map(|a| format!("{:#x}", a)).collect();
            filter["address"] = serde_json::json!(addr_strs);
        }

        let res = self
            .call_rpc("eth_getLogs", serde_json::json!([filter]))
            .await?;
        let logs_array = res.as_array().ok_or_else(|| {
            logrix_core::error::LogrixError::new(
                logrix_core::error::ErrorClass::Permanent,
                logrix_core::error::ErrorSource::ChainRpc,
                "eth_getLogs did not return an array",
            )
        })?;

        let mut event_logs = Vec::with_capacity(logs_array.len());
        for item in logs_array {
            event_logs.push(crate::parser::try_parse_rpc_log(item)?);
        }
        tracing::debug!(
            from_block,
            to_block,
            count = event_logs.len(),
            "Fetched logs from EVM RPC"
        );
        Ok(event_logs)
    }
}
