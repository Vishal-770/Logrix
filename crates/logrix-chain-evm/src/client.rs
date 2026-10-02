use crate::chunker::AdaptiveChunker;
use alloy_primitives::{Address, Bytes, B256};
use async_trait::async_trait;
use logrix_core::{
    domain::{BlockEnvelope, BlockRef, ChainId, EventLog},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::ChainPort,
};
use logrix_resilience::{RateLimiter, RetryPolicy};
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// High-performance JSON-RPC client implementing `ChainPort`.
///
/// Features adaptive AIMD chunking, token bucket rate limiting, and exponential retry.
#[derive(Clone)]
pub struct EvmChainClient {
    chain_id: ChainId,
    rpc_url: String,
    http_client: Client,
    rpc_id: Arc<AtomicU64>,
    chunker: Arc<AdaptiveChunker>,
    rate_limiter: Arc<RateLimiter>,
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

    /// Access the internal adaptive block chunker.
    pub fn chunker(&self) -> &AdaptiveChunker {
        &self.chunker
    }

    /// Perform a raw JSON-RPC call with token bucket rate limiting and retry backoff.
    async fn call_rpc(&self, method: &str, params: Value) -> LogrixResult<Value> {
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
                    } else if status.as_u16() == 429 {
                        self.chunker.record_failure();
                        if attempt < self.retry_policy.max_attempts {
                            attempt += 1;
                            let delay = self.retry_policy.delay_for_attempt(attempt);
                            warn!(attempt, "HTTP 429 Rate limited, backing off");
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        return Err(LogrixError::rate_limited(
                            ErrorSource::ChainRpc,
                            "HTTP 429 Too Many Requests from RPC gateway",
                            Some(Duration::from_secs(2)),
                        ));
                    } else {
                        if attempt < self.retry_policy.max_attempts {
                            attempt += 1;
                            let delay = self.retry_policy.delay_for_attempt(attempt);
                            tokio::time::sleep(delay).await;
                            continue;
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

#[async_trait]
impl ChainPort for EvmChainClient {
    async fn get_latest_block_number(&self) -> LogrixResult<u64> {
        let res = self.call_rpc("eth_blockNumber", json!([])).await?;
        let hex_str = res.as_str().ok_or_else(|| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                "eth_blockNumber did not return string",
            )
        })?;

        let stripped = hex_str.trim_start_matches("0x");
        u64::from_str_radix(stripped, 16).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                format!("Failed to parse hex block number '{hex_str}': {e}"),
            )
        })
    }

    async fn get_block_by_number(&self, number: u64) -> LogrixResult<Option<BlockRef>> {
        let hex_block = format!("0x{:x}", number);
        let res = self
            .call_rpc("eth_getBlockByNumber", json!([hex_block, false]))
            .await?;

        if res.is_null() {
            return Ok(None);
        }

        let hash_str = res.get("hash").and_then(|v| v.as_str()).ok_or_else(|| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                "Block missing hash field",
            )
        })?;

        let hash: B256 = hash_str.parse().map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                format!("Invalid block hash '{hash_str}': {e}"),
            )
        })?;

        Ok(Some(BlockRef::new(number, hash)))
    }

    async fn fetch_logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &[Address],
    ) -> LogrixResult<Vec<EventLog>> {
        let mut filter = json!({
            "fromBlock": format!("0x{:x}", from_block),
            "toBlock": format!("0x{:x}", to_block),
        });

        if !addresses.is_empty() {
            let addr_strs: Vec<String> = addresses.iter().map(|a| format!("{:#x}", a)).collect();
            filter["address"] = json!(addr_strs);
        }

        let res = self.call_rpc("eth_getLogs", json!([filter])).await?;
        let logs_array = res.as_array().ok_or_else(|| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                "eth_getLogs did not return an array",
            )
        })?;

        let mut event_logs = Vec::with_capacity(logs_array.len());
        for item in logs_array {
            let address_str = item
                .get("address")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let address: Address = address_str.parse().unwrap_or_default();

            let topics_val = item.get("topics").and_then(|v| v.as_array());
            let mut topics = Vec::new();
            if let Some(t_arr) = topics_val {
                for t in t_arr {
                    if let Some(ts) = t.as_str() {
                        if let Ok(b) = ts.parse::<B256>() {
                            topics.push(b);
                        }
                    }
                }
            }

            let data_str = item.get("data").and_then(|v| v.as_str()).unwrap_or("0x");
            let data_bytes = alloy_primitives::hex::decode(data_str.trim_start_matches("0x"))
                .unwrap_or_default();

            let tx_hash_str = item
                .get("transactionHash")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let tx_hash: B256 = tx_hash_str.parse().unwrap_or_default();

            let block_hash_str = item
                .get("blockHash")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let block_hash: B256 = block_hash_str.parse().unwrap_or_default();

            let block_num_hex = item
                .get("blockNumber")
                .and_then(|v| v.as_str())
                .unwrap_or("0x0");
            let block_number =
                u64::from_str_radix(block_num_hex.trim_start_matches("0x"), 16).unwrap_or(0);

            let log_idx_hex = item
                .get("logIndex")
                .and_then(|v| v.as_str())
                .unwrap_or("0x0");
            let log_index =
                u64::from_str_radix(log_idx_hex.trim_start_matches("0x"), 16).unwrap_or(0);

            let tx_idx_hex = item
                .get("transactionIndex")
                .and_then(|v| v.as_str())
                .unwrap_or("0x0");
            let tx_index =
                u64::from_str_radix(tx_idx_hex.trim_start_matches("0x"), 16).unwrap_or(0);

            event_logs.push(EventLog {
                address,
                topics,
                data: Bytes::from(data_bytes),
                tx_hash,
                log_index,
                tx_index,
                block_number,
                block_hash,
            });
        }

        debug!(
            from_block,
            to_block,
            count = event_logs.len(),
            "Fetched logs from EVM RPC"
        );
        Ok(event_logs)
    }

    async fn fetch_block_envelope(
        &self,
        number: u64,
        addresses: &[Address],
    ) -> LogrixResult<Option<BlockEnvelope>> {
        let hex_block = format!("0x{:x}", number);
        let block_res = self
            .call_rpc("eth_getBlockByNumber", json!([hex_block, false]))
            .await?;

        if block_res.is_null() {
            return Ok(None);
        }

        let hash_str = block_res
            .get("hash")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let parent_hash_str = block_res
            .get("parentHash")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let timestamp_hex = block_res
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0");

        let hash: B256 = hash_str.parse().unwrap_or_default();
        let parent_hash: B256 = parent_hash_str.parse().unwrap_or_default();
        let timestamp =
            u64::from_str_radix(timestamp_hex.trim_start_matches("0x"), 16).unwrap_or(0);

        let logs = self.fetch_logs(number, number, addresses).await?;

        Ok(Some(BlockEnvelope::new(
            self.chain_id,
            number,
            hash,
            parent_hash,
            timestamp,
            logs,
        )))
    }
}
