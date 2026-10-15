use crate::client::SqsClientConfig;
use logrix_core::{
    domain::{MessageHandle, QueueType},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

pub const SQS_MAX_VISIBILITY_TIMEOUT: i32 = 43_200;

/// AWS SQS implementation of `QueuePort`.
#[derive(Clone)]
pub struct SqsQueue {
    config: SqsClientConfig,
    pub(crate) buffer: Arc<Mutex<HashMap<QueueType, VecDeque<MessageHandle>>>>,
}

impl SqsQueue {
    pub fn new(config: SqsClientConfig) -> Self {
        Self {
            config,
            buffer: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn config(&self) -> &SqsClientConfig {
        &self.config
    }

    pub fn client(&self) -> &aws_sdk_sqs::Client {
        &self.config.client
    }

    /// Extend visibility for messages waiting in the local receive buffer.
    pub(crate) async fn extend_visibility_for_handles(
        &self,
        queue_type: QueueType,
        handles: &[MessageHandle],
    ) -> LogrixResult<()> {
        let url = self.config.resolve_url(queue_type)?;
        for handle in handles {
            self.client()
                .change_message_visibility()
                .queue_url(url)
                .receipt_handle(&handle.receipt_id)
                .visibility_timeout(SQS_MAX_VISIBILITY_TIMEOUT)
                .send()
                .await
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Transient,
                        ErrorSource::Queue,
                        format!("Failed to extend SQS message visibility: {e}"),
                    )
                })?;
        }
        Ok(())
    }

    /// Convenience initializer from environment variables or custom endpoint (e.g. LocalStack).
    pub async fn from_env(
        live_queue_url: impl Into<String>,
        backfill_queue_url: impl Into<String>,
        webhook_queue_url: impl Into<String>,
        dlq_url: impl Into<String>,
        endpoint_override: Option<&str>,
    ) -> LogrixResult<Self> {
        Self::from_env_with_params(
            live_queue_url,
            backfill_queue_url,
            webhook_queue_url,
            dlq_url,
            endpoint_override,
            2,
            1,
        )
        .await
    }

    /// Convenience initializer with custom long polling wait time and batch count.
    pub async fn from_env_with_params(
        live_queue_url: impl Into<String>,
        backfill_queue_url: impl Into<String>,
        webhook_queue_url: impl Into<String>,
        dlq_url: impl Into<String>,
        endpoint_override: Option<&str>,
        wait_time_seconds: i32,
        max_number_of_messages: i32,
    ) -> LogrixResult<Self> {
        let mut config = SqsClientConfig::from_env(
            live_queue_url,
            backfill_queue_url,
            webhook_queue_url,
            dlq_url,
            endpoint_override,
        )
        .await?;
        config.wait_time_seconds = wait_time_seconds;
        config.max_number_of_messages = max_number_of_messages;
        Ok(Self::new(config))
    }
}
