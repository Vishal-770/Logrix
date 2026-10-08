use crate::budget::CuBudgetTracker;
use crate::bulk::BulkStreamClient;
use crate::pool::ProviderPool;
use crate::provider::ManagedProvider;
use crate::singleflight::SingleFlight;
use logrix_core::domain::ChainId;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::warn;

/// Production Cost-Aware RPC Gateway implementing `ChainPort`.
#[derive(Clone)]
pub struct RpcGateway {
    pub(crate) chain_id: ChainId,
    pub(crate) pool: ProviderPool,
    pub(crate) budget: Arc<CuBudgetTracker>,
    pub(crate) singleflight: Arc<SingleFlight>,
    pub(crate) bulk_stream: Arc<BulkStreamClient>,
    pub(crate) block_cache: Arc<RwLock<Option<(u64, Instant)>>>,
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
            block_cache: Arc::new(RwLock::new(None)),
        }
    }

    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }
    pub fn budget(&self) -> &CuBudgetTracker {
        &self.budget
    }
    pub fn pool(&self) -> &ProviderPool {
        &self.pool
    }

    /// Read unexpired latest block height from 500ms micro-cache.
    pub async fn get_cached_block(&self) -> Option<u64> {
        let guard = self.block_cache.read().await;
        if let Some((block, ts)) = *guard {
            if ts.elapsed() < Duration::from_millis(500) {
                return Some(block);
            }
        }
        None
    }

    /// Store latest block height in 500ms micro-cache.
    pub async fn set_cached_block(&self, block: u64) {
        let mut guard = self.block_cache.write().await;
        *guard = Some((block, Instant::now()));
    }

    /// Execute a fallible operation across providers in the pool with intelligent error handling.
    pub async fn execute_with_fallback<T, F, Fut>(&self, method: &str, mut op: F) -> LogrixResult<T>
    where
        F: FnMut(ManagedProvider) -> Fut,
        Fut: std::future::Future<Output = LogrixResult<T>>,
    {
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
                    let err_msg = e.to_string();
                    let is_query_limit = err_msg.contains("more than")
                        || err_msg.contains("exceeded")
                        || err_msg.contains("range")
                        || err_msg.contains("limit")
                        || err_msg.contains("too many");

                    if is_query_limit {
                        // Do not trip circuit breaker on healthy providers when range is oversized
                        warn!(
                            provider = provider.name(),
                            method,
                            error = %e,
                            "Query range limit reached, returning error without tripping breaker"
                        );
                        return Err(e);
                    }

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
