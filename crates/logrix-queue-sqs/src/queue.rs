use crate::client::SqsClientConfig;
use logrix_core::{
    domain::{MessageHandle, QueueType},
    error::LogrixResult,
};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;

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
