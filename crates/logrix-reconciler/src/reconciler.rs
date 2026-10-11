use logrix_core::domain::{BlockRangeJob, ChainId, QueueMessage, QueueType};
use logrix_core::error::LogrixResult;
use logrix_core::ports::{ChainPort, QueuePort, StorePort};
use std::sync::atomic::{AtomicU64, Ordering};
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
    lag_threshold: u64,
    last_dispatched_to: Arc<AtomicU64>,
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
            last_dispatched_to: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Configure minimum blocks behind chain head required to trigger gap fill.
    #[must_use]
    pub fn with_lag_threshold(mut self, lag_threshold: u64) -> Self {
        self.lag_threshold = lag_threshold;
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

            self.last_dispatched_to.store(to_block, Ordering::Relaxed);

            return Ok(missing_count);
        }

        Ok(0)
    }

    /// Run the continuous reconciliation loop until cancellation.
    pub async fn run_loop(&self) {
        self.run_loop_with_shutdown(std::future::pending()).await;
    }

    /// Run the continuous reconciliation loop with graceful shutdown signal.
    pub async fn run_loop_with_shutdown<F>(&self, shutdown: F)
    where
        F: std::future::Future<Output = ()> + Send,
    {
        info!(
            chain = self.chain_id.as_u64(),
            interval_secs = self.interval.as_secs(),
            lag_threshold = self.lag_threshold,
            "Starting self-healing GapReconciler background loop"
        );

        let mut timer = tokio::time::interval(self.interval);
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    info!(chain = self.chain_id.as_u64(), "GapReconciler received shutdown signal, exiting cleanly");
                    break;
                }
                _ = timer.tick() => {
                    if let Err(e) = self.check_and_reconcile().await {
                        warn!(error = %e, "Error occurred during gap reconciliation check");
                    }
                }
            }
        }
    }
}
