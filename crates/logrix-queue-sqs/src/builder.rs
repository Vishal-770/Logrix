use crate::client::SqsClientConfig;
use aws_sdk_sqs::Client as SqsClient;
use logrix_core::{domain::QueueType, error::LogrixResult};
use std::collections::HashMap;
use tracing::info;

/// Fluent builder for constructing SqsClientConfig with customizable queue URLs and endpoints.
#[derive(Debug, Default, Clone)]
pub struct SqsConfigBuilder {
    live_url: Option<String>,
    backfill_url: Option<String>,
    webhook_url: Option<String>,
    dlq_url: Option<String>,
    endpoint_override: Option<String>,
    region: Option<String>,
    wait_time_seconds: Option<i32>,
    max_number_of_messages: Option<i32>,
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

    #[must_use]
    pub fn wait_time_seconds(mut self, secs: i32) -> Self {
        self.wait_time_seconds = Some(secs);
        self
    }

    #[must_use]
    pub fn max_number_of_messages(mut self, count: i32) -> Self {
        self.max_number_of_messages = Some(count);
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
        Ok(SqsClientConfig {
            client,
            queue_urls,
            wait_time_seconds: self.wait_time_seconds.unwrap_or(2),
            max_number_of_messages: self.max_number_of_messages.unwrap_or(1),
        })
    }
}
