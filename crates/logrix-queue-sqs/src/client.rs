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

    /// Create a fluent builder for configuring SQS queue URLs and client.
    pub fn builder() -> SqsConfigBuilder {
        SqsConfigBuilder::default()
    }
}

/// Fluent builder for constructing SqsClientConfig with customizable queue URLs and endpoints.
#[derive(Debug, Default, Clone)]
pub struct SqsConfigBuilder {
    live_url: Option<String>,
    backfill_url: Option<String>,
    webhook_url: Option<String>,
    dlq_url: Option<String>,
    endpoint_override: Option<String>,
    region: Option<String>,
}

impl SqsConfigBuilder {
    #[must_use]
    pub fn live_queue_url(mut self, url: impl Into<String>) -> Self {
        self.live_url = Some(url.into());
        self
    }

    #[must_use]
    pub fn backfill_queue_url(mut self, url: impl Into<String>) -> Self {
        self.backfill_url = Some(url.into());
        self
    }

    #[must_use]
    pub fn webhook_queue_url(mut self, url: impl Into<String>) -> Self {
        self.webhook_url = Some(url.into());
        self
    }

    #[must_use]
    pub fn dlq_url(mut self, url: impl Into<String>) -> Self {
        self.dlq_url = Some(url.into());
        self
    }

    #[must_use]
    pub fn endpoint_override(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint_override = Some(endpoint.into());
        self
    }

    #[must_use]
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    pub async fn build(self) -> LogrixResult<SqsClientConfig> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        if let Some(ref endpoint) = self.endpoint_override {
            config_loader = config_loader.endpoint_url(endpoint);
        }
        if let Some(ref reg) = self.region {
            config_loader = config_loader.region(aws_sdk_sqs::config::Region::new(reg.clone()));
        }

        let sdk_config = config_loader.load().await;
        let client = SqsClient::new(&sdk_config);

        let mut queue_urls = HashMap::new();
        if let Some(live) = self.live_url {
            queue_urls.insert(QueueType::Live, live);
        }
        if let Some(backfill) = self.backfill_url {
            queue_urls.insert(QueueType::Backfill, backfill);
        }
        if let Some(webhook) = self.webhook_url {
            queue_urls.insert(QueueType::Webhook, webhook);
        }
        if let Some(dlq) = self.dlq_url {
            queue_urls.insert(QueueType::DeadLetter, dlq);
        }

        info!("Configured AWS SQS client via builder");
        Ok(SqsClientConfig { client, queue_urls })
    }
}
