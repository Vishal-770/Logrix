use logrix_core::{
    domain::{QueueMessage, QueueType},
    ports::QueuePort,
};
use std::time::Duration;
use tracing::{info, warn};

/// Check if queue depths exceed the backpressure threshold.
/// Returns true if backpressure is active and ingestion should throttle.
pub async fn is_backpressure_active(queue: &dyn QueuePort, max_queue_depth: u64) -> bool {
    let live_depth = queue.depth(QueueType::Live).await.unwrap_or(0);
    let backfill_depth = queue.depth(QueueType::Backfill).await.unwrap_or(0);
    let total = live_depth + backfill_depth;

    if total >= max_queue_depth {
        warn!(
            total_depth = total,
            max_queue_depth, "Queue depth exceeded backpressure threshold; throttling ingestion"
        );
        true
    } else {
        false
    }
}

/// Publishes a message to the specified queue with exponential backoff on broker failures.
pub async fn publish_with_retry(
    queue: &dyn QueuePort,
    queue_type: QueueType,
    msg: &QueueMessage,
) -> bool {
    let mut backoff = Duration::from_millis(100);
    let max_backoff = Duration::from_secs(5);

    loop {
        match queue.publish(queue_type, msg).await {
            Ok(()) => return true,
            Err(e) => {
                warn!(
                    error = %e,
                    backoff_ms = backoff.as_millis(),
                    "Failed to publish to queue; retrying with exponential backoff"
                );
                tokio::select! {
                    _ = crate::shutdown::wait_for_shutdown_signal() => {
                        info!("Shutdown signal received during queue publish retry; aborting");
                        return false;
                    }
                    _ = tokio::time::sleep(backoff) => {
                        backoff = (backoff * 2).min(max_backoff);
                    }
                }
            }
        }
    }
}
