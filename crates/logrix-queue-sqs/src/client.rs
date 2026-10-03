use aws_sdk_sqs::Client as SqsClient;
use logrix_core::{
    domain::QueueType,
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};
use std::collections::HashMap;
use tracing::info;

/// SQS client manager and queue URL resolver.
#[derive(Clone)]
pub struct SqsClientConfig {
    pub client: SqsClient,
    pub queue_urls: HashMap<QueueType, String>,
}

impl SqsClientConfig {
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

        info!("Configured AWS SQS client and queue endpoints");
        Ok(Self { client, queue_urls })
    }

    /// Resolve the full SQS queue URL for the given queue type.
    pub fn resolve_url(&self, queue_type: QueueType) -> LogrixResult<&str> {
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
