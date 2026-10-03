use crate::detector::ReorgDetector;
use alloy_primitives::B256;
use logrix_core::domain::{ChainId, QueueMessage, QueueType};
use logrix_core::error::LogrixResult;
use logrix_core::ports::{QueuePort, StorePort};
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

/// Orchestrates atomic state rollback, database soft-delete, and revert notification dispatching.
#[derive(Clone)]
pub struct ReorgHandler {
    chain_id: ChainId,
    store: Arc<dyn StorePort>,
    queue: Arc<dyn QueuePort>,
    detector: ReorgDetector,
}

impl ReorgHandler {
    pub fn new(
        chain_id: ChainId,
        store: Arc<dyn StorePort>,
        queue: Arc<dyn QueuePort>,
        detector: ReorgDetector,
    ) -> Self {
        Self {
            chain_id,
            store,
            queue,
            detector,
        }
    }

    /// Execute atomic rollback to the fork block.
    ///
    /// 1. Updates database: marks all events above `fork_block` as `is_reverted = TRUE`.
    /// 2. Rewinds database checkpoint to `fork_block`.
    /// 3. Rewinds in-memory `RollingBlockBuffer` to `fork_block`.
    /// 4. Dispatches `event.reverted` notification message to `QueueType::Webhook`.
    pub async fn execute_rollback(
        &self,
        fork_block: u64,
        fork_hash: B256,
        reorg_depth: u64,
    ) -> LogrixResult<()> {
        warn!(
            chain = self.chain_id.as_u64(),
            fork_block,
            fork_hash = %fork_hash,
            reorg_depth,
            "Initiating blockchain reorg rollback..."
        );

        // 1. Soft-delete database state and rewind checkpoint
        self.store
            .rollback_to_block(self.chain_id, fork_block)
            .await?;

        // 2. Rewind in-memory ring buffer
        {
            let mut buf = self.detector.buffer().write().await;
            buf.rewind_to(fork_block);
        }

        // 3. Emit revert notification to webhook queue for downstream listeners
        let revert_payload = json!({
            "event": "chain.reorg",
            "chain_id": self.chain_id.as_u64(),
            "fork_block": fork_block,
            "fork_hash": format!("{:#x}", fork_hash),
            "reorg_depth": reorg_depth,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });

        let webhook_msg = QueueMessage::Custom {
            job_id: uuid::Uuid::new_v4().to_string(),
            chain_id: self.chain_id,
            job_type: "event.reverted".to_string(),
            attempt: 1,
            payload: revert_payload,
        };

        if let Err(e) = self.queue.publish(QueueType::Webhook, &webhook_msg).await {
            warn!(error = %e, "Failed to publish revert notification to webhook queue");
        } else {
            info!("Published chain.reorg notification to webhook queue");
        }

        info!(
            fork_block,
            reorg_depth, "Reorganization rollback completed successfully"
        );
        Ok(())
    }
}
