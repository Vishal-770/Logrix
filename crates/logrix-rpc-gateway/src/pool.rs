use crate::provider::ManagedProvider;
use logrix_core::domain::ChainId;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Multi-provider pool router with round-robin load distribution and staleness detection.
#[derive(Clone)]
pub struct ProviderPool {
    chain_id: ChainId,
    providers: Arc<RwLock<Vec<ManagedProvider>>>,
    rr_counter: Arc<AtomicU64>,
}

impl ProviderPool {
    pub fn new(chain_id: ChainId) -> Self {
        Self {
            chain_id,
            providers: Arc::new(RwLock::new(Vec::new())),
            rr_counter: Arc::new(AtomicU64::new(0)),
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
        list.sort_by_key(|p| p.priority());
    }

    /// Select the best available provider.
    pub async fn select_provider(&self) -> LogrixResult<ManagedProvider> {
        self.select_provider_excluding(&std::collections::HashSet::new())
            .await
    }

    /// Select the best available provider with round-robin load distribution and staleness penalty.
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

        // Detect highest known block tip across all providers
        let max_block = list.iter().map(|p| p.latest_block()).max().unwrap_or(0);

        // Find healthy untried providers and score them
        let mut healthy = Vec::new();
        for p in list.iter() {
            if !excluded_names.contains(p.name()) && p.is_healthy().await {
                // Penalize providers lagging more than 3 blocks behind the highest seen tip
                let is_stale = max_block > 0
                    && p.latest_block() > 0
                    && max_block.saturating_sub(p.latest_block()) > 3;
                let effective_prio = if is_stale {
                    p.priority() + 10
                } else {
                    p.priority()
                };
                healthy.push((effective_prio, p));
            }
        }

        if let Some(&(best_prio, _)) = healthy.iter().min_by_key(|(prio, _)| *prio) {
            // Collect all providers matching the best effective priority tier
            let tier_providers: Vec<&ManagedProvider> = healthy
                .iter()
                .filter(|(prio, _)| *prio == best_prio)
                .map(|(_, p)| *p)
                .collect();

            // Round-robin load balance across candidates within the same tier
            let idx =
                (self.rr_counter.fetch_add(1, Ordering::Relaxed) as usize) % tier_providers.len();
            let selected = tier_providers[idx];

            debug!(
                provider = selected.name(),
                priority = selected.priority(),
                avg_latency_ms = selected.avg_latency_ms(),
                "Selected RPC provider from pool with tier load-balancing"
            );
            return Ok(selected.clone());
        }

        // Fallback to best-effort untried provider with shortest cooldown
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
