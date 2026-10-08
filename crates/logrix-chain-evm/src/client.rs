use crate::chunker::AdaptiveChunker;
use logrix_core::{
    domain::ChainId,
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};
use logrix_resilience::{RateLimiter, RetryPolicy};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::warn;

/// High-performance JSON-RPC client implementing `ChainPort`.
///
/// Features adaptive AIMD chunking, token bucket rate limiting, and exponential retry.
#[derive(Clone)]
pub struct EvmChainClient {
    chain_id: ChainId,
    pub(crate) rpc_url: String,
    pub(crate) http_client: Client,
    pub(crate) rpc_id: Arc<AtomicU64>,
    pub(crate) chunker: Arc<AdaptiveChunker>,
    pub(crate) rate_limiter: Arc<RateLimiter>,
    retry_policy: RetryPolicy,
}

impl EvmChainClient {
    pub fn new(chain_id: ChainId, rpc_url: impl Into<String>) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(15))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(20)
            .build()
            .expect("build reqwest client");

        Self {
            chain_id,
            rpc_url: rpc_url.into(),
            http_client,
            rpc_id: Arc::new(AtomicU64::new(1)),
            chunker: Arc::new(AdaptiveChunker::new(500, 5, 2000)),
            // 100 max capacity, 50 tokens/sec refill rate
            rate_limiter: Arc::new(RateLimiter::new(100, 50.0)),
            retry_policy: RetryPolicy::new(3, Duration::from_millis(200), Duration::from_secs(5)),
        }
    }

    /// Supervised client for Multi-Provider Gateways: fails fast to allow instant fallback.
    pub fn new_supervised(chain_id: ChainId, rpc_url: impl Into<String>) -> Self {
        let mut client = Self::new(chain_id, rpc_url);
        client.retry_policy =
            RetryPolicy::new(1, Duration::from_millis(50), Duration::from_millis(100));
        client
    }

    /// Access the internal chain ID.
    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Access the internal adaptive block chunker.
    pub fn chunker(&self) -> &AdaptiveChunker {
        &self.chunker
    }

    /// Perform a raw JSON-RPC call with token bucket rate limiting and retry backoff.
    pub(crate) async fn call_rpc(&self, method: &str, params: Value) -> LogrixResult<Value> {
        let mut attempt = 0;

        loop {
            // Apply client-side rate limiting
            self.rate_limiter.acquire(1.0).await;

            let id = self.rpc_id.fetch_add(1, Ordering::Relaxed);
            let body = json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            });

            let start = Instant::now();
            let res = self
                .http_client
                .post(&self.rpc_url)
                .json(&body)
                .send()
                .await;

            match res {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let json_res: Value = resp.json().await.map_err(|e| {
                            LogrixError::new(
                                ErrorClass::Transient,
                                ErrorSource::ChainRpc,
                                format!("Failed to parse RPC response: {e}"),
                            )
                        })?;

                        if let Some(err) = json_res.get("error") {
                            let err_msg = err.to_string();
                            // Detect payload size / limit errors
                            if err_msg.contains("more than")
                                || err_msg.contains("exceeded")
                                || err_msg.contains("range")
                            {
                                self.chunker.record_failure();
                            }

                            if attempt < self.retry_policy.max_attempts {
                                attempt += 1;
                                let delay = self.retry_policy.delay_for_attempt(attempt);
                                warn!(
                                    attempt,
                                    method,
                                    error = %err_msg,
                                    "RPC returned error, retrying"
                                );
                                tokio::time::sleep(delay).await;
                                continue;
                            }

                            return Err(LogrixError::new(
                                ErrorClass::Transient,
                                ErrorSource::ChainRpc,
                                format!("RPC server returned error: {err_msg}"),
                            ));
                        }

                        if let Some(result) = json_res.get("result") {
                            self.chunker.record_success(start.elapsed());
                            return Ok(result.clone());
                        }

                        return Err(LogrixError::new(
                            ErrorClass::Permanent,
                            ErrorSource::ChainRpc,
                            "RPC response missing result field",
                        ));
                    } else {
                        self.chunker.record_failure();
                        let retry_after = extract_retry_after(resp.headers());
                        if attempt < self.retry_policy.max_attempts {
                            attempt += 1;
                            let delay = retry_after
                                .unwrap_or_else(|| self.retry_policy.delay_for_attempt(attempt));
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        if status.as_u16() == 429 {
                            return Err(LogrixError::rate_limited(
                                ErrorSource::ChainRpc,
                                "HTTP 429 Too Many Requests from RPC gateway",
                                retry_after.or(Some(Duration::from_secs(2))),
                            ));
                        }
                        return Err(LogrixError::new(
                            ErrorClass::Transient,
                            ErrorSource::ChainRpc,
                            format!("RPC HTTP error status: {status}"),
                        ));
                    }
                }
                Err(e) => {
                    self.chunker.record_failure();
                    if attempt < self.retry_policy.max_attempts {
                        attempt += 1;
                        let delay = self.retry_policy.delay_for_attempt(attempt);
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(LogrixError::new(
                        ErrorClass::Transient,
                        ErrorSource::ChainRpc,
                        format!("HTTP request failed: {e}"),
                    ));
                }
            }
        }
    }
}

fn extract_retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(Duration::from_secs)
}
