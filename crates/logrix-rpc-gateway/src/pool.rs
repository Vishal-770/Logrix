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
        self.select_provider_excluding(&std::collections::HashSet::new())
            .await
    }

    /// Select the best available provider, excluding those already attempted in this request.
    pub async fn select_provider_excluding(
        &self,
        excluded_names: &std::collections::HashSet<String>,
    ) -> LogrixResult<ManagedProvider> {
        let list = self.providers.read().await;
        if list.is_empty() {
            return Err(LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                "No RPC providers configured in ProviderPool",
            ));
        }

        // Find healthy providers that have not yet been tried
        let mut healthy = Vec::new();
        for p in list.iter() {
            if !excluded_names.contains(p.name()) && p.is_healthy().await {
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

        // If no healthy untried providers, fallback to best-effort untried provider with shortest cooldown
        let untried: Vec<&ManagedProvider> = list
            .iter()
            .filter(|p| !excluded_names.contains(p.name()))
            .collect();

        if let Some(best_effort) = untried
            .iter()
            .min_by_key(|p| (p.priority(), p.cooldown_duration()))
        {
            warn!(
                provider = best_effort.name(),
                "All preferred providers cooling down or tried. Selecting best-effort fallback."
            );
            return Ok((*best_effort).clone());
        }

        Err(LogrixError::new(
            ErrorClass::Transient,
            ErrorSource::ChainRpc,
            "All available RPC providers exhausted for this request",
        ))
    }

    /// Get all providers in pool for status reporting.
    pub async fn all_providers(&self) -> Vec<ManagedProvider> {
        self.providers.read().await.clone()
    }
}
