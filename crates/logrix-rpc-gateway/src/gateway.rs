use crate::budget::CuBudgetTracker;
use crate::bulk::BulkStreamClient;
use crate::pool::ProviderPool;
use crate::provider::ManagedProvider;
use crate::singleflight::SingleFlight;
use alloy_primitives::Address;
use async_trait::async_trait;
use logrix_core::domain::{BlockEnvelope, BlockRef, ChainId, EventLog};
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use logrix_core::ports::ChainPort;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, warn};

/// Production Cost-Aware RPC Gateway implementing `ChainPort`.
///
/// Features:
/// - Priority & Latency weighted multi-provider pool
/// - Automatic failover with per-provider circuit breakers
/// - Compute Unit (CU) budget tracking & backfill pausing
/// - In-flight concurrent request deduplication (Single-Flight)
/// - Bulk stream fast-path (SQD / HyperSync)
#[derive(Clone)]
pub struct RpcGateway {
    chain_id: ChainId,
    pool: ProviderPool,
    budget: Arc<CuBudgetTracker>,
    singleflight: Arc<SingleFlight>,
    bulk_stream: Arc<BulkStreamClient>,
}

impl RpcGateway {
    pub fn new(
        chain_id: ChainId,
        pool: ProviderPool,
        max_cu_budget: Option<u64>,
        bulk_endpoint: Option<String>,
    ) -> Self {
        Self {
            chain_id,
            pool,
            budget: Arc::new(CuBudgetTracker::new(max_cu_budget)),
            singleflight: Arc::new(SingleFlight::new()),
            bulk_stream: Arc::new(BulkStreamClient::new(chain_id, bulk_endpoint)),
        }
    }

    /// Associated Chain ID.
    pub fn chain_id(&self) -> ChainId { self.chain_id }
    /// Access budget tracker.
    pub fn budget(&self) -> &CuBudgetTracker { &self.budget }
    /// Access provider pool.
    pub fn pool(&self) -> &ProviderPool { &self.pool }

    /// Execute a fallible operation across providers in the pool.
    /// Automatically trips circuit breakers on errors, records latencies on success,
    /// and retries on healthy fallback providers.
    pub async fn execute_with_fallback<T, F, Fut>(&self, method: &str, mut op: F) -> LogrixResult<T>
    where
        F: FnMut(ManagedProvider) -> Fut,
        Fut: std::future::Future<Output = LogrixResult<T>>,
    {
        // Check CU budget before executing
        if self.budget.is_exhausted() {
            return Err(LogrixError::rate_limited(
                ErrorSource::ChainRpc,
                format!(
                    "RPC Compute Unit budget exhausted ({} CUs consumed). Pausing queries.",
                    self.budget.total_consumed()
                ),
                None,
            ));
        }

        let mut last_err = None;
        let all_providers = self.pool.all_providers().await;
        let mut tried_names = std::collections::HashSet::new();

        for _ in 0..all_providers.len().max(1) {
            let provider = match self.pool.select_provider_excluding(&tried_names).await {
                Ok(p) => p,
                Err(e) => {
                    last_err = Some(e);
                    break;
                }
            };
            tried_names.insert(provider.name().to_string());
            let start = Instant::now();

            match op(provider.clone()).await {
                Ok(val) => {
                    let elapsed = start.elapsed();
                    provider.record_success(elapsed).await;
                    self.budget.record_method(method);
                    return Ok(val);
                }
                Err(e) => {
                    let elapsed = start.elapsed();
                    warn!(
                        provider = provider.name(),
                        method,
                        elapsed_ms = elapsed.as_millis(),
                        error = %e,
                        "Provider call failed, tripping breaker and attempting fallback"
                    );
                    provider.record_failure().await;
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::ChainRpc,
                "All RPC fallback providers failed",
            )
        }))
    }
}

#[async_trait]
impl ChainPort for RpcGateway {
    async fn get_latest_block_number(&self) -> LogrixResult<u64> {
        let key = "latest_block_number";
        let res_str = self
            .singleflight
            .execute(key, || async {
                self.execute_with_fallback("eth_blockNumber", |provider| async move {
                    provider.client().get_latest_block_number().await
                })
                .await
                .map(|num| num.to_string())
            })
            .await?;

        res_str.parse::<u64>().map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                format!("Failed to parse singleflight block number: {e}"),
            )
        })
    }

    async fn get_block_by_number(&self, number: u64) -> LogrixResult<Option<BlockRef>> {
        self.execute_with_fallback("eth_getBlockByNumber", |provider| async move {
            provider.client().get_block_by_number(number).await
        })
        .await
    }

    async fn fetch_logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &[Address],
    ) -> LogrixResult<Vec<EventLog>> {
        // Fast-path: Check SQD/HyperSync bulk stream first
        if self.bulk_stream.is_supported() {
            if let Ok(Some(logs)) = self
                .bulk_stream
                .stream_logs(from_block, to_block, addresses)
                .await
            {
                debug!(
                    count = logs.len(),
                    "Retrieved logs from bulk stream fast-path"
                );
                return Ok(logs);
            }
        }

        // Standard RPC Pool path
        self.execute_with_fallback("eth_getLogs", |provider| {
            let addrs = addresses.to_vec();
            async move {
                provider
                    .client()
                    .fetch_logs(from_block, to_block, &addrs)
                    .await
            }
        })
        .await
    }

    async fn fetch_block_envelope(
        &self,
        number: u64,
        addresses: &[Address],
    ) -> LogrixResult<Option<BlockEnvelope>> {
        self.execute_with_fallback("eth_getBlockByNumber", |provider| {
            let addrs = addresses.to_vec();
            async move { provider.client().fetch_block_envelope(number, &addrs).await }
        })
        .await
    }
}
