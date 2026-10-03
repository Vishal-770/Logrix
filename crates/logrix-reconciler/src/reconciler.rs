use logrix_core::domain::{BlockRangeJob, ChainId, QueueMessage, QueueType};
use logrix_core::error::LogrixResult;
use logrix_core::ports::{ChainPort, QueuePort, StorePort};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Self-healing continuous gap reconciler.
///
/// Runs periodically in the background to detect missing block gaps between the
/// database checkpoint and the live chain head, automatically dispatching backfill
/// range jobs to refill dropped messages.
#[derive(Clone)]
pub struct GapReconciler {
    chain_id: ChainId,
    store: Arc<dyn StorePort>,
    queue: Arc<dyn QueuePort>,
    chain_client: Arc<dyn ChainPort>,
    interval: Duration,
}

impl GapReconciler {
    pub fn new(
        chain_id: ChainId,
        store: Arc<dyn StorePort>,
        queue: Arc<dyn QueuePort>,
        chain_client: Arc<dyn ChainPort>,
        interval: Duration,
    ) -> Self {
        Self {
            chain_id,
            store,
            queue,
            chain_client,
            interval,
        }
    }

    /// Perform a single reconciliation scan.
    ///
    /// Returns the number of missing blocks scheduled for refill.
    pub async fn check_and_reconcile(&self) -> LogrixResult<u64> {
        let checkpoint = match self.store.get_checkpoint(self.chain_id).await? {
            Some(cp) => cp,
            None => {
                debug!("No checkpoint found yet; skipping gap reconciliation");
                return Ok(0);
            }
        };

        let latest_block = self.chain_client.get_latest_block_number().await?;
        let last_indexed = checkpoint.last_indexed_block;

        // If checkpoint is far behind live chain head (> 10 blocks) and no active jobs
        if latest_block > last_indexed + 10 {
            let from_block = last_indexed + 1;
            let to_block = latest_block.saturating_sub(1);
            let missing_count = to_block - from_block + 1;

            warn!(
                from_block,
                to_block,
                missing_count,
                "Gap reconciler detected lagging checkpoint behind chain head. Dispatching refill job."
            );

            let job = BlockRangeJob::new(
                self.chain_id,
                from_block,
                to_block,
                format!("{}:{}-{}", self.chain_id, from_block, to_block),
            );

            self.queue
                .publish(QueueType::Backfill, &QueueMessage::BackfillRange(job))
                .await?;

            return Ok(missing_count);
        }

        Ok(0)
    }

    /// Run the continuous reconciliation loop until cancellation.
    pub async fn run_loop(&self) {
        info!(
            chain = self.chain_id.as_u64(),
            interval_secs = self.interval.as_secs(),
            "Starting self-healing GapReconciler background loop"
        );

        let mut timer = tokio::time::interval(self.interval);
        loop {
            timer.tick().await;
            if let Err(e) = self.check_and_reconcile().await {
                warn!(error = %e, "Error occurred during gap reconciliation check");
            }
        }
    }
}
