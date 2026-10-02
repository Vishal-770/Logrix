use async_trait::async_trait;
use logrix_core::{
    domain::{ChainId, Checkpoint, EventLog},
    error::LogrixResult,
    ports::StorePort,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Thread-safe in-memory StorePort for testing atomic writes, checkpoints, and reorg rollbacks.
#[derive(Debug, Clone, Default)]
pub struct MockStorePort {
    /// Stored events keyed by idempotency key
    events: Arc<Mutex<HashMap<String, (ChainId, EventLog)>>>,
    /// Checkpoints keyed by chain ID
    checkpoints: Arc<Mutex<HashMap<u64, Checkpoint>>>,
    /// Rollback event log history for assertions
    pub rollbacks: Arc<Mutex<Vec<(ChainId, u64)>>>,
}

impl MockStorePort {
    pub fn new() -> Self {
        Self::default()
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
        let mut store_events = self.events.lock().await;
        let mut store_checkpoints = self.checkpoints.lock().await;

        // Atomic write: all events + checkpoint
        for event in events {
            let key = event.idempotency_key(checkpoint.chain_id);
            // Idempotent write: duplicate keys are safely updated/ignored
            store_events.insert(key, (checkpoint.chain_id, event.clone()));
        }

        store_checkpoints.insert(checkpoint.chain_id.as_u64(), checkpoint.clone());
        Ok(())
    }

    async fn get_checkpoint(&self, chain_id: ChainId) -> LogrixResult<Option<Checkpoint>> {
        let store_checkpoints = self.checkpoints.lock().await;
        Ok(store_checkpoints.get(&chain_id.as_u64()).cloned())
    }

    async fn rollback_to_block(&self, chain_id: ChainId, to_block: u64) -> LogrixResult<()> {
        let mut store_events = self.events.lock().await;
        let mut store_checkpoints = self.checkpoints.lock().await;

        // Delete all events where block_number > to_block for this chain
        store_events.retain(|_, (c, event)| !(*c == chain_id && event.block_number > to_block));

        // Update checkpoint last_indexed_block
        if let Some(cp) = store_checkpoints.get_mut(&chain_id.as_u64()) {
            cp.last_indexed_block = to_block;
            cp.is_finalized = false;
        }

        self.rollbacks.lock().await.push((chain_id, to_block));
        Ok(())
    }
}
