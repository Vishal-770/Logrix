use crate::domain::{ChainId, Checkpoint, EventLog};
use crate::error::LogrixResult;
use async_trait::async_trait;

/// Abstract database port for atomic event writes, checkpoints, and reorg rollbacks.
///
/// Implemented by Postgres, SQLite, and external database adapters.
#[async_trait]
pub trait StorePort: Send + Sync {
    /// Commit event logs and advance the chain checkpoint in a single atomic transaction.
    ///
    /// If writing events or advancing checkpoint fails, the entire transaction rolls back.
    async fn write_events_and_checkpoint(
        &self,
        events: &[EventLog],
        checkpoint: &Checkpoint,
    ) -> LogrixResult<()>;

    /// Retrieve the current checkpoint for a chain.
    async fn get_checkpoint(&self, chain_id: ChainId) -> LogrixResult<Option<Checkpoint>>;

    /// Roll back all indexed event rows and reset checkpoint after a blockchain reorg.
    ///
    /// Deletes all event records where `block_number > to_block` and updates checkpoint.
    async fn rollback_to_block(&self, chain_id: ChainId, to_block: u64) -> LogrixResult<()>;
}
