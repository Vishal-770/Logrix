use crate::client::SqsClientConfig;
use logrix_core::error::LogrixResult;

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
