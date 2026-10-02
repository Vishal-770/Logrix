use crate::domain::{ChainId, EventLog};
use crate::error::LogrixResult;
use async_trait::async_trait;

/// Abstract output sink for dispatching events to external consumers (e.g. Webhooks, Kafka, Streams).
#[async_trait]
pub trait SinkPort: Send + Sync {
    /// Emit decoded event logs to the sink destination.
    async fn emit_events(&self, events: &[EventLog]) -> LogrixResult<()>;

    /// Emit reorg notification indicating that blocks from `from_block` have been reverted.
    async fn emit_revert(&self, chain_id: ChainId, from_block: u64) -> LogrixResult<()>;
}
