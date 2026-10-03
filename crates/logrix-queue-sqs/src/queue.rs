use crate::client::SqsClientConfig;
use async_trait::async_trait;
use aws_sdk_sqs::types::QueueAttributeName;
use logrix_core::{
    domain::{MessageHandle, QueueMessage, QueueType},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::QueuePort,
};
use tracing::{debug, info};

/// AWS SQS implementation of `QueuePort`.
#[derive(Clone)]
pub struct SqsQueue {
    config: SqsClientConfig,
}

impl SqsQueue {
    pub fn new(config: SqsClientConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &SqsClientConfig {
        &self.config
    }

    pub fn client(&self) -> &aws_sdk_sqs::Client {
        &self.config.client
    }

    /// Convenience initializer from environment variables or custom endpoint (e.g. LocalStack).
    pub async fn from_env(
        live_queue_url: impl Into<String>,
        backfill_queue_url: impl Into<String>,
        webhook_queue_url: impl Into<String>,
        dlq_url: impl Into<String>,
        endpoint_override: Option<&str>,
    ) -> LogrixResult<Self> {
        let config = SqsClientConfig::from_env(
            live_queue_url,
            backfill_queue_url,
            webhook_queue_url,
            dlq_url,
            endpoint_override,
        )
        .await?;
        Ok(Self::new(config))
    }
}

#[async_trait]
impl QueuePort for SqsQueue {
    async fn publish(&self, queue_type: QueueType, message: &QueueMessage) -> LogrixResult<()> {
        let url = self.config.resolve_url(queue_type)?;
        let body = serde_json::to_string(message).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Failed to serialize message: {e}"),
            )
        })?;

        self.config
            .client
            .send_message()
            .queue_url(url)
            .message_body(body)
            .send()
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to send message to SQS {url}: {e}"),
                )
            })?;

        debug!(queue = ?queue_type, "Published message to AWS SQS");
        Ok(())
    }

    async fn consume(&self, queue_type: QueueType) -> LogrixResult<Option<MessageHandle>> {
        let url = self.config.resolve_url(queue_type)?;
        let resp = self
            .config
            .client
            .receive_message()
            .queue_url(url)
            .max_number_of_messages(1)
            .wait_time_seconds(2)
            .send()
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to receive message from SQS {url}: {e}"),
                )
            })?;

        if let Some(msg) = resp.messages().first() {
            let receipt = msg.receipt_handle().unwrap_or_default();
            let body = msg.body().unwrap_or_default();
            let queue_msg: QueueMessage = serde_json::from_str(body).map_err(|e| {
                LogrixError::new(
                    ErrorClass::Permanent,
                    ErrorSource::Queue,
                    format!("Failed to parse SQS message JSON: {e}"),
                )
            })?;

            Ok(Some(MessageHandle::with_queue(
                receipt, queue_msg, queue_type,
            )))
        } else {
            Ok(None)
        }
    }

    async fn ack(&self, handle: &MessageHandle) -> LogrixResult<()> {
        let target = handle.source_queue.unwrap_or(QueueType::Live);
        let url = self.config.resolve_url(target)?;
        self.config
            .client
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
        let target = handle.source_queue.unwrap_or(QueueType::Live);
        let url = self.config.resolve_url(target)?;
        let visibility = if requeue { 0 } else { 300 };

        self.config
            .client
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
        let url = self.config.resolve_url(queue_type)?;
        let resp = self
            .config
            .client
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

        let count = resp
            .attributes()
            .and_then(|a| a.get(&QueueAttributeName::ApproximateNumberOfMessages))
            .map(|s| s.as_str())
            .unwrap_or("0");
        Ok(count.parse().unwrap_or(0))
    }
}
