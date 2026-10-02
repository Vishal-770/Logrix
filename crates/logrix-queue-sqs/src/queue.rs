use async_trait::async_trait;
use aws_sdk_sqs::{types::QueueAttributeName, Client as SqsClient};
use logrix_core::{
    domain::{MessageHandle, QueueMessage, QueueType},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::QueuePort,
};
use std::collections::HashMap;
use tracing::{debug, info};

/// AWS SQS implementation of `QueuePort`.
///
/// Supports AWS IAM authentication (IRSA on EKS), native SQS DLQ redrive,
/// and LocalStack endpoint override for local testing.
#[derive(Clone)]
pub struct SqsQueue {
    client: SqsClient,
    queue_urls: HashMap<QueueType, String>,
}

impl SqsQueue {
    pub fn new(client: SqsClient, queue_urls: HashMap<QueueType, String>) -> Self {
        Self { client, queue_urls }
    }

    /// Initialize from environment variables or custom endpoint (e.g. LocalStack).
    pub async fn from_env(
        live_queue_url: impl Into<String>,
        backfill_queue_url: impl Into<String>,
        webhook_queue_url: impl Into<String>,
        dlq_url: impl Into<String>,
        endpoint_override: Option<&str>,
    ) -> LogrixResult<Self> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        if let Some(endpoint) = endpoint_override {
            config_loader = config_loader.endpoint_url(endpoint);
        }

        let sdk_config = config_loader.load().await;
        let client = SqsClient::new(&sdk_config);

        let mut queue_urls = HashMap::new();
        queue_urls.insert(QueueType::Live, live_queue_url.into());
        queue_urls.insert(QueueType::Backfill, backfill_queue_url.into());
        queue_urls.insert(QueueType::Webhook, webhook_queue_url.into());
        queue_urls.insert(QueueType::DeadLetter, dlq_url.into());

        info!("Configured AWS SQS queue adapter successfully");
        Ok(Self { client, queue_urls })
    }

    fn get_url(&self, queue_type: QueueType) -> LogrixResult<&str> {
        self.queue_urls
            .get(&queue_type)
            .map(|s| s.as_str())
            .ok_or_else(|| {
                LogrixError::new(
                    ErrorClass::Permanent,
                    ErrorSource::Queue,
                    format!("No SQS queue URL configured for {queue_type:?}"),
                )
            })
    }
}

#[async_trait]
impl QueuePort for SqsQueue {
    async fn publish(&self, queue_type: QueueType, message: &QueueMessage) -> LogrixResult<()> {
        let url = self.get_url(queue_type)?;
        let body = serde_json::to_string(message).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Failed to serialize message: {e}"),
            )
        })?;

        self.client
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
        let url = self.get_url(queue_type)?;

        let resp = self
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

        let messages = resp.messages();
        if let Some(msg) = messages.first() {
            let receipt_handle = msg.receipt_handle().unwrap_or_default();
            let body_str = msg.body().unwrap_or_default();

            let queue_msg: QueueMessage = serde_json::from_str(body_str).map_err(|e| {
                LogrixError::new(
                    ErrorClass::Permanent,
                    ErrorSource::Queue,
                    format!("Failed to parse SQS message JSON: {e}"),
                )
            })?;

            Ok(Some(MessageHandle::new(receipt_handle, queue_msg)))
        } else {
            Ok(None)
        }
    }

    async fn ack(&self, handle: &MessageHandle) -> LogrixResult<()> {
        // We delete by attempting across queues or using Live as default
        let url = self.get_url(QueueType::Live)?;
        self.client
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
        let url = self.get_url(QueueType::Live)?;
        let visibility = if requeue { 0 } else { 300 };

        self.client
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

        info!(
            handle = %handle.receipt_id,
            reason,
            "Message dead-lettered in SQS"
        );
        Ok(())
    }

    async fn depth(&self, queue_type: QueueType) -> LogrixResult<u64> {
        let url = self.get_url(queue_type)?;

        let resp = self
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

        let count_str = resp
            .attributes()
            .and_then(|attrs| attrs.get(&QueueAttributeName::ApproximateNumberOfMessages))
            .map(|s| s.as_str())
            .unwrap_or("0");

        Ok(count_str.parse().unwrap_or(0))
    }
}
