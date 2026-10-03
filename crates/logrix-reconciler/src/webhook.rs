use logrix_core::domain::{QueueMessage, QueueType};
use logrix_core::error::LogrixResult;
use logrix_core::ports::QueuePort;
use logrix_resilience::RetryPolicy;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Webhook notification worker delivering HTTP POST callbacks for chain events (e.g. reorgs).
#[derive(Clone)]
pub struct WebhookDispatcher {
    queue: Arc<dyn QueuePort>,
    http_client: Client,
    retry_policy: RetryPolicy,
    target_webhook_url: Option<String>,
}

impl WebhookDispatcher {
    pub fn new(queue: Arc<dyn QueuePort>, target_webhook_url: Option<String>) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("build reqwest client");

        Self {
            queue,
            http_client,
            retry_policy: RetryPolicy::new(3, Duration::from_millis(200), Duration::from_secs(3)),
            target_webhook_url,
        }
    }

    /// Process a single webhook event from the queue.
    pub async fn process_one(&self) -> LogrixResult<bool> {
        let handle = match self.queue.consume(QueueType::Webhook).await? {
            Some(h) => h,
            None => return Ok(false),
        };

        let target_url = match &self.target_webhook_url {
            Some(url) => url.clone(),
            None => {
                // No external webhook configured; ack immediately
                self.queue.ack(&handle).await?;
                return Ok(true);
            }
        };

        let payload = match &handle.message {
            QueueMessage::Custom { payload, .. } => payload.clone(),
            _ => serde_json::json!({ "msg": "unknown_webhook_type" }),
        };

        let mut attempt = 0;
        let mut delivered = false;

        while attempt < self.retry_policy.max_attempts {
            attempt += 1;
            let resp = self
                .http_client
                .post(&target_url)
                .json(&payload)
                .send()
                .await;

            match resp {
                Ok(r) if r.status().is_success() => {
                    delivered = true;
                    break;
                }
                Ok(r) => {
                    warn!(status = %r.status(), attempt, "Webhook destination returned error status");
                }
                Err(e) => {
                    warn!(error = %e, attempt, "Failed to deliver webhook HTTP POST");
                }
            }

            tokio::time::sleep(self.retry_policy.delay_for_attempt(attempt)).await;
        }

        if delivered {
            info!("Webhook event successfully delivered");
            self.queue.ack(&handle).await?;
        } else {
            error!("Exhausted webhook retries; routing to Dead-Letter Queue (DLQ)");
            self.queue.nack(&handle, false).await?;
        }

        Ok(true)
    }

    /// Run the webhook processor background loop.
    pub async fn run_loop(&self) {
        info!("Starting WebhookDispatcher consumer loop");
        loop {
            match self.process_one().await {
                Ok(processed) => {
                    if !processed {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Error processing webhook queue item");
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
    }
}
