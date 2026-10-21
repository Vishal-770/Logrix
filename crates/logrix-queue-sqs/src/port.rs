use crate::queue::SqsQueue;
use async_trait::async_trait;
use aws_sdk_sqs::types::QueueAttributeName;
use logrix_core::{
    domain::{MessageHandle, QueueMessage, QueueType},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::QueuePort,
};
use tracing::{debug, info};

#[async_trait]
impl QueuePort for SqsQueue {
    async fn publish(&self, queue_type: QueueType, message: &QueueMessage) -> LogrixResult<()> {
        let url = self.config().resolve_url(queue_type)?;
        let body = serde_json::to_string(message).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Failed to serialize message: {e}"),
            )
        })?;

        self.client()
            .send_message()
            .queue_url(url)
            .message_body(body)
            .send()
            .await
            .map_err(|e| {
                LogrixError::new(ErrorClass::Transient, ErrorSource::Queue, format!("{e}"))
            })?;

        debug!(queue = ?queue_type, "Published message to AWS SQS");
        Ok(())
    }

    async fn publish_batch(
        &self,
        queue_type: QueueType,
        messages: &[QueueMessage],
    ) -> LogrixResult<()> {
        self.publish_batch(queue_type, messages).await
    }

    async fn consume(&self, queue_type: QueueType) -> LogrixResult<Option<MessageHandle>> {
        if let Some(cached) = self.pop_buffered_message(queue_type).await {
            return Ok(Some(cached));
        }

        let url = self.config().resolve_url(queue_type)?;
        let max_msgs = self.config().max_number_of_messages.clamp(1, 10);
        let resp = self
            .client()
            .receive_message()
            .queue_url(url)
            .max_number_of_messages(max_msgs)
            .wait_time_seconds(self.config().wait_time_seconds)
            .send()
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to receive message from SQS {url}: {e}"),
                )
            })?;

        let mut handles = Vec::new();
        for msg in resp.messages() {
            let receipt = msg.receipt_handle().ok_or_else(|| {
                LogrixError::new(
                    ErrorClass::Integrity,
                    ErrorSource::Queue,
                    "SQS message received without a receipt handle",
                )
            })?;

            let body = msg.body().ok_or_else(|| {
                LogrixError::new(
                    ErrorClass::Integrity,
                    ErrorSource::Queue,
                    "SQS message received with empty body",
                )
            })?;

            let queue_msg: QueueMessage = serde_json::from_str(body).map_err(|e| {
                LogrixError::new(
                    ErrorClass::Permanent,
                    ErrorSource::Queue,
                    format!("Failed to parse SQS message JSON: {e}"),
                )
            })?;

            handles.push(MessageHandle::new(receipt, queue_msg, queue_type));
        }

        if handles.is_empty() {
            return Ok(None);
        }

        let first = handles.remove(0);
        if !handles.is_empty() {
            self.extend_visibility_for_handles(queue_type, &handles)
                .await?;
            let mut buf_guard = self.buffer.lock().await;
            let queue_buf = buf_guard.entry(queue_type).or_default();
            for h in handles {
                queue_buf.push_back(h);
            }
        }
        Ok(Some(first))
    }

    async fn ack(&self, handle: &MessageHandle) -> LogrixResult<()> {
        let url = self.config().resolve_url(handle.source_queue)?;
        self.client()
            .delete_message()
            .queue_url(url)
            .receipt_handle(&handle.receipt_id)
            .send()
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to delete message from SQS: {e}"),
                )
            })?;
        Ok(())
    }

    async fn nack(&self, handle: &MessageHandle, requeue: bool) -> LogrixResult<()> {
        let url = self.config().resolve_url(handle.source_queue)?;
        let visibility = if requeue { 0 } else { 300 };

        self.client()
            .change_message_visibility()
            .queue_url(url)
            .receipt_handle(&handle.receipt_id)
            .visibility_timeout(visibility)
            .send()
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to change SQS message visibility: {e}"),
                )
            })?;
        Ok(())
    }

    async fn dead_letter(&self, handle: &MessageHandle, reason: &str) -> LogrixResult<()> {
        self.publish(QueueType::DeadLetter, &handle.message).await?;
        self.ack(handle).await?;
        info!(handle = %handle.receipt_id, reason, "Message moved to DLQ");
        Ok(())
    }

    async fn depth(&self, queue_type: QueueType) -> LogrixResult<u64> {
        let url = self.config().resolve_url(queue_type)?;
        let resp = self
            .client()
            .get_queue_attributes()
            .queue_url(url)
            .attribute_names(QueueAttributeName::ApproximateNumberOfMessages)
            .send()
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to query SQS queue attributes: {e}"),
                )
            })?;

        let count_str = resp
            .attributes()
            .and_then(|a| a.get(&QueueAttributeName::ApproximateNumberOfMessages))
            .ok_or_else(|| {
                LogrixError::new(
                    ErrorClass::Integrity,
                    ErrorSource::Queue,
                    format!("Missing ApproximateNumberOfMessages attribute for {queue_type:?}"),
                )
            })?;

        count_str.parse::<u64>().map_err(|e| {
            LogrixError::new(
                ErrorClass::Integrity,
                ErrorSource::Queue,
                format!("Failed to parse ApproximateNumberOfMessages '{count_str}': {e}"),
            )
        })
    }
}
