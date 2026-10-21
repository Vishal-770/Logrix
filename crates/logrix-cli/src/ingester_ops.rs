use logrix_core::{
    domain::{QueueMessage, QueueType},
    ports::QueuePort,
};
use std::time::Duration;
use tracing::{info, warn};

pub const DEFAULT_MAX_PUBLISH_RETRIES: u32 = 10;
pub const DEFAULT_INITIAL_BACKOFF_MS: u64 = 100;
pub const DEFAULT_MAX_BACKOFF_SECS: u64 = 5;

/// Check if queue depths exceed the backpressure threshold.
/// Fails safe: if queue depth cannot be determined due to a broker error,
/// activates backpressure to avoid flooding an unresponsive or degrading queue.
pub async fn is_backpressure_active(queue: &dyn QueuePort, max_queue_depth: u64) -> bool {
    let live_depth = match queue.depth(QueueType::Live).await {
        Ok(d) => d,
        Err(e) => {
            warn!(error = %e, "Failed to query live queue depth; activating backpressure fail-safe");
            return true;
        }
    };

    let backfill_depth = match queue.depth(QueueType::Backfill).await {
        Ok(d) => d,
        Err(e) => {
            warn!(error = %e, "Failed to query backfill queue depth; activating backpressure fail-safe");
            return true;
        }
    };

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
/// Bounded by a maximum retry attempts threshold to prevent indefinite blocking.
pub async fn publish_with_retry(
    queue: &dyn QueuePort,
    queue_type: QueueType,
    msg: &QueueMessage,
) -> bool {
    publish_with_retry_limits(
        queue,
        queue_type,
        msg,
        DEFAULT_MAX_PUBLISH_RETRIES,
        Duration::from_millis(DEFAULT_INITIAL_BACKOFF_MS),
        Duration::from_secs(DEFAULT_MAX_BACKOFF_SECS),
    )
    .await
}

/// Publishes a message with custom retry limits and backoff durations.
pub async fn publish_with_retry_limits(
    queue: &dyn QueuePort,
    queue_type: QueueType,
    msg: &QueueMessage,
    max_retries: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
) -> bool {
    let mut backoff = initial_backoff;
    let mut attempts = 0;

    loop {
        match queue.publish(queue_type, msg).await {
            Ok(()) => return true,
            Err(e) => {
                attempts += 1;
                warn!(
                    error = %e,
                    attempts,
                    max_retries,
                    backoff_ms = backoff.as_millis(),
                    "Failed to publish to queue; retrying with exponential backoff"
                );

                if attempts >= max_retries {
                    tracing::error!(
                        attempts,
                        "Exceeded max retries publishing to queue; giving up to prevent hang"
                    );
                    return false;
                }

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
