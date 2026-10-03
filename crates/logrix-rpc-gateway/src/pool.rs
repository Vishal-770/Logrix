use crate::provider::ManagedProvider;
use logrix_core::domain::ChainId;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Multi-provider pool router with latency-weighted priority fallback.
#[derive(Clone)]
pub struct ProviderPool {
    chain_id: ChainId,
    providers: Arc<RwLock<Vec<ManagedProvider>>>,
}

impl ProviderPool {
    pub fn new(chain_id: ChainId) -> Self {
        Self {
            chain_id,
            providers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Associated Chain ID.
    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Register a provider in the pool.
    pub async fn add_provider(&self, provider: ManagedProvider) {
        let mut list = self.providers.write().await;
        list.push(provider);
        // Sort by priority (lowest number = highest priority)
        list.sort_by_key(|p| p.priority());
    }

    /// Select the best available provider:
    /// 1. Filters for healthy providers whose circuit breaker allows calls.
    /// 2. Groups by highest priority tier.
    /// 3. Picks lowest-latency provider in that tier.
    /// 4. If all in tier are tripped, falls back to next priority tier.
    pub async fn select_provider(&self) -> LogrixResult<ManagedProvider> {
        let list = self.providers.read().await;
        if list.is_empty() {
            return Err(LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                "No RPC providers configured in ProviderPool",
            ));
        }

        // Find healthy providers
        let mut healthy = Vec::new();
        for p in list.iter() {
            if p.is_healthy().await {
                healthy.push(p);
            }
        }

        if let Some(best) = healthy
            .iter()
            .min_by_key(|p| (p.priority(), p.avg_latency_ms()))
        {
            debug!(
                provider = best.name(),
                priority = best.priority(),
                avg_latency_ms = best.avg_latency_ms(),
                "Selected RPC provider from pool"
            );
            return Ok((*best).clone());
        }

        // All circuit breakers are currently open. Fall back to provider with shortest cooldown.
        warn!("All RPC providers are currently unhealthy or cooling down. Selecting best-effort fallback.");
        let best_effort = list
            .iter()
            .min_by_key(|p| (p.priority(), p.cooldown_duration()))
            .cloned()
            .unwrap();

        Ok(best_effort)
    }

    /// Get all providers in pool for status reporting.
    pub async fn all_providers(&self) -> Vec<ManagedProvider> {
        self.providers.read().await.clone()
    }
}
