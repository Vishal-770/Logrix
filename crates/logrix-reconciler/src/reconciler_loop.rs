use crate::reconciler::GapReconciler;
use tracing::{info, warn};

impl GapReconciler {
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
            chunk_size = self.chunk_size,
            "Starting self-healing GapReconciler background loop"
        );

        let mut timer = tokio::time::interval(self.interval);
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    info!(
                        chain = self.chain_id.as_u64(),
                        "GapReconciler received shutdown signal, exiting cleanly"
                    );
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
