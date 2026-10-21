use async_trait::async_trait;
use logrix_cli::ingester_ops::{is_backpressure_active, publish_with_retry_limits};
use logrix_core::{
    domain::{LiveBlockJob, MessageHandle, QueueMessage, QueueType},
    error::{ErrorSource, LogrixError},
    ports::QueuePort,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct MockFailingQueue {
    depth_error: bool,
    publish_failures: AtomicUsize,
}

#[async_trait]
impl QueuePort for MockFailingQueue {
    async fn publish(
        &self,
        _queue_type: QueueType,
        _message: &QueueMessage,
    ) -> Result<(), LogrixError> {
        let prev = self.publish_failures.fetch_add(1, Ordering::SeqCst);
        if prev < 3 {
            Err(LogrixError::transient(
                ErrorSource::Queue,
                "broker connection reset",
            ))
        } else {
            Ok(())
        }
    }

    async fn consume(&self, _queue_type: QueueType) -> Result<Option<MessageHandle>, LogrixError> {
        Ok(None)
    }

    async fn ack(&self, _handle: &MessageHandle) -> Result<(), LogrixError> {
        Ok(())
    }

    async fn nack(&self, _handle: &MessageHandle, _requeue: bool) -> Result<(), LogrixError> {
        Ok(())
    }

    async fn dead_letter(&self, _handle: &MessageHandle, _reason: &str) -> Result<(), LogrixError> {
        Ok(())
    }

    async fn depth(&self, _queue_type: QueueType) -> Result<u64, LogrixError> {
        if self.depth_error {
            Err(LogrixError::transient(
                ErrorSource::Queue,
                "broker unreachable",
            ))
        } else {
            Ok(50)
        }
    }
}

#[tokio::test]
async fn test_backpressure_fails_safe_on_broker_depth_error() {
    let mock = MockFailingQueue {
        depth_error: true,
        publish_failures: AtomicUsize::new(0),
    };

    let active = is_backpressure_active(&mock, 1000).await;
    assert!(
        active,
        "Backpressure must activate (fail safe) when queue depth queries fail"
    );
}

#[tokio::test]
async fn test_publish_with_retry_succeeds_after_transient_failures() {
    let mock = MockFailingQueue {
        depth_error: false,
        publish_failures: AtomicUsize::new(0),
    };
    let job = LiveBlockJob::new(
        logrix_core::domain::ChainId::new(1),
        100,
        alloy_primitives::B256::ZERO,
        alloy_primitives::B256::ZERO,
    );
    let msg = QueueMessage::LiveBlock(job);

    let success = publish_with_retry_limits(
        &mock,
        QueueType::Live,
        &msg,
        5,
        Duration::from_millis(5),
        Duration::from_millis(20),
    )
    .await;

    assert!(success, "Publish must succeed after retries");
    assert_eq!(mock.publish_failures.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn test_publish_with_retry_aborts_on_exceeded_attempts() {
    let mock = MockFailingQueue {
        depth_error: false,
        publish_failures: AtomicUsize::new(0),
    };
    let job = LiveBlockJob::new(
        logrix_core::domain::ChainId::new(1),
        100,
        alloy_primitives::B256::ZERO,
        alloy_primitives::B256::ZERO,
    );
    let msg = QueueMessage::LiveBlock(job);

    // Limit to 2 retries, but mock fails 3 times
    let success = publish_with_retry_limits(
        &mock,
        QueueType::Live,
        &msg,
        2,
        Duration::from_millis(5),
        Duration::from_millis(20),
    )
    .await;

    assert!(
        !success,
        "Publish must abort and return false after exceeding max retries"
    );
}
