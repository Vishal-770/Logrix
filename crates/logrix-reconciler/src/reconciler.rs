use logrix_core::domain::{BlockRangeJob, ChainId, QueueMessage, QueueType};
use logrix_core::error::LogrixResult;
use logrix_core::ports::{ChainPort, QueuePort, StorePort};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// Self-healing continuous gap reconciler.
///
/// Runs periodically in the background to detect missing block gaps between the
/// database checkpoint and the live chain head, automatically dispatching backfill
/// range jobs to refill dropped messages.
#[derive(Clone)]
pub struct GapReconciler {
    pub(crate) chain_id: ChainId,
    pub(crate) store: Arc<dyn StorePort>,
    pub(crate) queue: Arc<dyn QueuePort>,
    pub(crate) chain_client: Arc<dyn ChainPort>,
    pub(crate) interval: Duration,
    pub(crate) lag_threshold: u64,
    pub(crate) chunk_size: Option<u64>,
    pub(crate) last_dispatched_to: Arc<AtomicU64>,
    pub(crate) last_checkpoint_seen: Arc<AtomicU64>,
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
            lag_threshold: 10,
            chunk_size: None,
            last_dispatched_to: Arc::new(AtomicU64::new(0)),
            last_checkpoint_seen: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Configure minimum blocks behind chain head required to trigger gap fill.
    #[must_use]
    pub fn with_lag_threshold(mut self, lag_threshold: u64) -> Self {
        self.lag_threshold = lag_threshold;
        self
    }

    /// Configure chunk size per backfill range job dispatched by reconciler.
    #[must_use]
    pub fn with_chunk_size(mut self, chunk_size: u64) -> Self {
        self.chunk_size = Some(chunk_size.max(1));
        self
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

        // Reset in-flight marker if a reorg rolled the checkpoint backward
        let prev_seen = self
            .last_checkpoint_seen
            .swap(last_indexed, Ordering::Relaxed);
        if prev_seen > 0 && last_indexed < prev_seen {
            warn!(
                chain = self.chain_id.as_u64(),
                prev_seen,
                last_indexed,
                "Checkpoint rolled backward after reorg; resetting in-flight dispatch marker"
            );
            self.last_dispatched_to
                .store(last_indexed, Ordering::Relaxed);
        }

        // If checkpoint is far behind live chain head (> lag_threshold blocks)
        if latest_block > last_indexed + self.lag_threshold {
            let to_block = latest_block.saturating_sub(1);
            let last_dispatched = self.last_dispatched_to.load(Ordering::Relaxed);

            let from_block = if last_dispatched > last_indexed {
                if to_block <= last_dispatched {
                    debug!(
                        chain = self.chain_id.as_u64(),
                        last_indexed,
                        last_dispatched,
                        to_block,
                        "Gap range already in flight; skipping duplicate dispatch"
                    );
                    return Ok(0);
                }
                last_dispatched + 1
            } else {
                last_indexed + 1
            };

            if from_block > to_block {
                return Ok(0);
            }

            let missing_count = to_block - from_block + 1;

            warn!(
                from_block,
                to_block,
                missing_count,
                lag_threshold = self.lag_threshold,
                chunk_size = ?self.chunk_size,
                "Gap reconciler detected lagging checkpoint behind chain head. Dispatching refill jobs."
            );

            if let Some(chunk_size) = self.chunk_size {
                let mut cur = from_block;
                while cur <= to_block {
                    let chunk_end = (cur + chunk_size - 1).min(to_block);
                    let job = BlockRangeJob::try_new(
                        self.chain_id,
                        cur,
                        chunk_end,
                        format!("{}:{}-{}", self.chain_id, cur, chunk_end),
                    )?;
                    self.queue
                        .publish(QueueType::Backfill, &QueueMessage::BackfillRange(job))
                        .await?;
                    cur = chunk_end + 1;
                }
            } else {
                let job = BlockRangeJob::try_new(
                    self.chain_id,
                    from_block,
                    to_block,
                    format!("{}:{}-{}", self.chain_id, from_block, to_block),
                )?;
                self.queue
                    .publish(QueueType::Backfill, &QueueMessage::BackfillRange(job))
                    .await?;
            }

            self.last_dispatched_to.store(to_block, Ordering::Relaxed);

            return Ok(missing_count);
        }

        Ok(0)
    }
}
