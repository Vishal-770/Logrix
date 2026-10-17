use crate::producer::{publish_many, publish_single};
use crate::queue::RabbitMQQueue;
use async_trait::async_trait;
use lapin::{
    options::{BasicAckOptions, BasicNackOptions, QueueDeclareOptions},
    types::FieldTable,
};
use logrix_core::{
    domain::{MessageHandle, QueueMessage, QueueType},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::QueuePort,
};
use tracing::info;

#[async_trait]
impl QueuePort for RabbitMQQueue {
    async fn publish(&self, queue_type: QueueType, message: &QueueMessage) -> LogrixResult<()> {
        let (_, routing_key) = self.topology().queue_and_key(queue_type);
        publish_single(
            self.channel(),
            &self.topology().exchange,
            routing_key,
            queue_type,
            message,
        )
        .await
    }

    async fn publish_batch(
        &self,
        queue_type: QueueType,
        messages: &[QueueMessage],
    ) -> LogrixResult<()> {
        let (_, routing_key) = self.topology().queue_and_key(queue_type);
        publish_many(
            self.channel(),
            &self.topology().exchange,
            routing_key,
            queue_type,
            messages,
        )
        .await
    }

    async fn consume(&self, queue_type: QueueType) -> LogrixResult<Option<MessageHandle>> {
        let (queue_name, _) = self.topology().queue_and_key(queue_type);

        let msg = self
            .channel()
            .basic_get(
                queue_name,
                lapin::options::BasicGetOptions { no_ack: false },
            )
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to get message from {queue_name}: {e}"),
                )
            })?;

        match msg {
            Some(delivery) => {
                let message: QueueMessage =
                    serde_json::from_slice(&delivery.data).map_err(|e| {
                        LogrixError::new(
                            ErrorClass::Permanent,
                            ErrorSource::Queue,
                            format!("Failed to deserialize message: {e}"),
                        )
                    })?;

                Ok(Some(MessageHandle::new(
                    delivery.delivery.delivery_tag.to_string(),
                    message,
                    queue_type,
                )))
            }
            None => Ok(None),
        }
    }

    async fn ack(&self, handle: &MessageHandle) -> LogrixResult<()> {
        let tag: u64 = handle.receipt_id.parse().map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Invalid delivery tag '{}': {e}", handle.receipt_id),
            )
        })?;

        self.channel()
            .basic_ack(tag, BasicAckOptions { multiple: false })
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to ack message tag {tag}: {e}"),
                )
            })
    }

    async fn nack(&self, handle: &MessageHandle, requeue: bool) -> LogrixResult<()> {
        let tag: u64 = handle.receipt_id.parse().map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Invalid delivery tag '{}': {e}", handle.receipt_id),
            )
        })?;

        self.channel()
            .basic_nack(
                tag,
                BasicNackOptions {
                    requeue,
                    multiple: false,
                },
            )
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to nack message tag {tag}: {e}"),
                )
            })
    }

    async fn dead_letter(&self, handle: &MessageHandle, reason: &str) -> LogrixResult<()> {
        let dlq_msg = handle.message.clone();
        self.publish(QueueType::DeadLetter, &dlq_msg).await?;
        self.ack(handle).await?;

        info!(
            handle = %handle.receipt_id,
            reason = %reason,
            "Message moved to Dead-Letter Queue"
        );
        Ok(())
    }

    async fn depth(&self, queue_type: QueueType) -> LogrixResult<u64> {
        let (queue_name, _) = self.topology().queue_and_key(queue_type);

        let queue_state = self
            .channel()
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    passive: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to query depth for {queue_name}: {e}"),
                )
            })?;

        Ok(queue_state.message_count() as u64)
    }
}
