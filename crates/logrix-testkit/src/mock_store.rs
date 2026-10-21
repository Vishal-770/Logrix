use alloy_primitives::B256;
use async_trait::async_trait;
use logrix_core::{
    domain::{ChainId, Checkpoint, EventLog},
    error::{ErrorSource, LogrixError, LogrixResult},
    ports::StorePort,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Thread-safe in-memory StorePort for testing atomic writes, checkpoints, reorg rollbacks, and faults.
#[derive(Debug, Clone, Default)]
pub struct MockStorePort {
    events: Arc<Mutex<HashMap<String, (ChainId, EventLog)>>>,
    checkpoints: Arc<Mutex<HashMap<u64, Checkpoint>>>,
    pub rollbacks: Arc<Mutex<Vec<(ChainId, u64, B256)>>>,
    injected_errors: Arc<AtomicUsize>,
    simulated_latency: Arc<Mutex<Option<Duration>>>,
}

impl MockStorePort {
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures artificial latency for database queries and writes.
    pub async fn set_simulated_latency(&self, latency: Duration) {
        *self.simulated_latency.lock().await = Some(latency);
    }

    /// Injects `count` transient database errors.
    pub fn inject_transient_errors(&self, count: usize) {
        self.injected_errors.store(count, Ordering::SeqCst);
    }

    async fn apply_latency_and_check_error(&self) -> LogrixResult<()> {
        if let Some(delay) = *self.simulated_latency.lock().await {
            tokio::time::sleep(delay).await;
        }
        let mut curr = self.injected_errors.load(Ordering::SeqCst);
        while curr > 0 {
            match self.injected_errors.compare_exchange_weak(
                curr,
                curr - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    return Err(LogrixError::transient(
                        ErrorSource::Database,
                        "Simulated database lock contention or timeout",
                    ));
                }
                Err(actual) => curr = actual,
            }
        }
        Ok(())
    }

    pub async fn total_events(&self) -> usize {
        self.events.lock().await.len()
    }

    pub async fn get_events_for_chain(&self, chain_id: ChainId) -> Vec<EventLog> {
        let events = self.events.lock().await;
        events
            .values()
            .filter(|(c, _)| *c == chain_id)
            .map(|(_, e)| e.clone())
            .collect()
    }
}

#[async_trait]
impl StorePort for MockStorePort {
    async fn write_events_and_checkpoint(
        &self,
        events: &[EventLog],
        checkpoint: &Checkpoint,
    ) -> LogrixResult<()> {
        self.apply_latency_and_check_error().await?;
        let mut store_events = self.events.lock().await;
        let mut store_checkpoints = self.checkpoints.lock().await;

        for event in events {
            let key = event.idempotency_key(checkpoint.chain_id);
            store_events.insert(key, (checkpoint.chain_id, event.clone()));
        }

        store_checkpoints.insert(checkpoint.chain_id.as_u64(), checkpoint.clone());
        Ok(())
    }

    async fn get_checkpoint(&self, chain_id: ChainId) -> LogrixResult<Option<Checkpoint>> {
        self.apply_latency_and_check_error().await?;
        let store_checkpoints = self.checkpoints.lock().await;
        Ok(store_checkpoints.get(&chain_id.as_u64()).cloned())
    }

    async fn rollback_to_block(
        &self,
        chain_id: ChainId,
        to_block: u64,
        fork_hash: B256,
    ) -> LogrixResult<()> {
        self.apply_latency_and_check_error().await?;
        let mut store_events = self.events.lock().await;
        let mut store_checkpoints = self.checkpoints.lock().await;

        store_events.retain(|_, (c, event)| !(*c == chain_id && event.block_number > to_block));

        if let Some(cp) = store_checkpoints.get_mut(&chain_id.as_u64()) {
            cp.last_indexed_block = to_block;
            cp.last_indexed_hash = fork_hash;
            cp.is_finalized = false;
        }

        self.rollbacks
            .lock()
            .await
            .push((chain_id, to_block, fork_hash));
        Ok(())
    }
}
