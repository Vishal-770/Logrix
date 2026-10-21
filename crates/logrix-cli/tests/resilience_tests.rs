use async_trait::async_trait;
use logrix_cli::shutdown::{shutdown_channel, wait_for_shutdown_signal, wait_shutdown};
use logrix_core::{
    domain::{ChainId, MessageHandle, QueueMessage, QueueType},
    error::{ErrorSource, LogrixError, LogrixResult},
    ports::QueuePort,
};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Default)]
struct FlakyQueue {
    failure_countdown: AtomicU32,
    published_count: AtomicU32,
    simulated_depth: AtomicU32,
}

#[async_trait]
impl QueuePort for FlakyQueue {
    async fn publish(&self, _queue_type: QueueType, _message: &QueueMessage) -> LogrixResult<()> {
        let remaining = self.failure_countdown.load(Ordering::SeqCst);
        if remaining > 0 {
            self.failure_countdown.fetch_sub(1, Ordering::SeqCst);
            return Err(LogrixError::transient(
                ErrorSource::Queue,
                "Simulated broker blip",
            ));
        }
        self.published_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn consume(&self, _queue_type: QueueType) -> LogrixResult<Option<MessageHandle>> {
        let remaining = self.failure_countdown.load(Ordering::SeqCst);
        if remaining > 0 {
            self.failure_countdown.fetch_sub(1, Ordering::SeqCst);
            return Err(LogrixError::transient(
                ErrorSource::Queue,
                "Simulated connection drop",
            ));
        }
        Ok(None)
    }

    async fn ack(&self, _handle: &MessageHandle) -> LogrixResult<()> {
        Ok(())
    }

    async fn nack(&self, _handle: &MessageHandle, _requeue: bool) -> LogrixResult<()> {
        Ok(())
    }

    async fn dead_letter(&self, _handle: &MessageHandle, _reason: &str) -> LogrixResult<()> {
        Ok(())
    }

    async fn depth(&self, _queue_type: QueueType) -> LogrixResult<u64> {
        Ok(self.simulated_depth.load(Ordering::SeqCst) as u64)
    }
}

#[tokio::test]
async fn test_shutdown_channel_multiple_subscribers() {
    let (tx, rx1) = shutdown_channel();
    let rx2 = rx1.clone();
    let rx3 = rx1.clone();

    let h1 = tokio::spawn(async move { wait_shutdown(rx1).await });
    let h2 = tokio::spawn(async move { wait_shutdown(rx2).await });
    let h3 = tokio::spawn(async move { wait_shutdown(rx3).await });

    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!h1.is_finished());
    assert!(!h2.is_finished());
    assert!(!h3.is_finished());

    tx.send(true).expect("Send shutdown");

    h1.await.expect("Task 1 finished cleanly");
    h2.await.expect("Task 2 finished cleanly");
    h3.await.expect("Task 3 finished cleanly");
}

#[tokio::test]
async fn test_shutdown_signal_no_false_trigger() {
    let timeout_res =
        tokio::time::timeout(Duration::from_millis(100), wait_for_shutdown_signal()).await;
    assert!(
        timeout_res.is_err(),
        "Shutdown signal must not fire without an external signal"
    );
}

#[tokio::test]
async fn test_flaky_queue_recovery_after_retries() {
    let flaky = Arc::new(FlakyQueue {
        failure_countdown: AtomicU32::new(2),
        published_count: AtomicU32::new(0),
        ..Default::default()
    });

    let job = logrix_core::domain::LiveBlockJob::new(
        ChainId::new(1),
        100,
        alloy_primitives::B256::ZERO,
        alloy_primitives::B256::ZERO,
    );
    let msg = QueueMessage::LiveBlock(job);

    let mut backoff = Duration::from_millis(10);
    let mut success = false;
    for _ in 0..5 {
        match flaky.publish(QueueType::Live, &msg).await {
            Ok(()) => {
                success = true;
                break;
            }
            Err(_) => {
                tokio::time::sleep(backoff).await;
                backoff *= 2;
            }
        }
    }

    assert!(
        success,
        "Publish succeeded after simulated transient failures"
    );
    assert_eq!(flaky.published_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_ingester_backpressure_threshold() {
    use logrix_cli::ingester_ops::is_backpressure_active;

    let queue = FlakyQueue {
        simulated_depth: AtomicU32::new(400),
        ..Default::default()
    };
    // Live depth (400) + Backfill depth (400) = 800
    // Below threshold of 1000
    assert!(!is_backpressure_active(&queue, 1000).await);

    // At or above threshold of 800
    assert!(is_backpressure_active(&queue, 800).await);
}

#[test]
fn test_postgres_config_bounded_defaults() {
    let cfg = logrix_store_postgres::PostgresConfig::new("postgres://localhost/test", "default");
    assert_eq!(cfg.max_connections, 10);
    assert_eq!(cfg.min_connections, 2);
    assert_eq!(cfg.statement_timeout, std::time::Duration::from_secs(30));
}
