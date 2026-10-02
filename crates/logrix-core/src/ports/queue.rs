use crate::domain::{MessageHandle, QueueMessage, QueueType};
use crate::error::LogrixResult;
use async_trait::async_trait;

/// Abstract queue port for producing and consuming indexing jobs.
///
/// Implemented by SQS, RabbitMQ, Redis, Memory, and gRPC plugins.
#[async_trait]
pub trait QueuePort: Send + Sync {
    /// Publish a message to the specified logical queue.
    async fn publish(&self, queue_type: QueueType, message: &QueueMessage) -> LogrixResult<()>;

    /// Consume a single message from the specified queue (non-blocking or with short timeout).
    async fn consume(&self, queue_type: QueueType) -> LogrixResult<Option<MessageHandle>>;

    /// Acknowledge successful processing and remove message from the queue.
    async fn ack(&self, handle: &MessageHandle) -> LogrixResult<()>;

    /// Negatively acknowledge and optionally requeue the message.
    async fn nack(&self, handle: &MessageHandle, requeue: bool) -> LogrixResult<()>;

    /// Move the poison message to the Dead-Letter Queue (DLQ).
    async fn dead_letter(&self, handle: &MessageHandle, reason: &str) -> LogrixResult<()>;

    /// Approximate count of visible messages in the queue (for autoscaling & metrics).
    async fn depth(&self, queue_type: QueueType) -> LogrixResult<u64>;
}
