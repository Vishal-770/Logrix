use crate::client::WebhookHttpClient;
use crate::store::WebhookStore;
use logrix_core::{
    domain::{QueueMessage, QueueType, WebhookPayload},
    error::LogrixResult,
    ports::QueuePort,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Standalone worker service for consuming and dispatching webhooks.
#[derive(Clone)]
pub struct WebhookDispatcherService {
    queue: Arc<dyn QueuePort>,
    store: Option<WebhookStore>,
    client: WebhookHttpClient,
    default_webhook_url: Option<String>,
    default_secret: String,
}

impl WebhookDispatcherService {
    pub fn new(
        queue: Arc<dyn QueuePort>,
        store: Option<WebhookStore>,
        default_webhook_url: Option<String>,
        default_secret: impl Into<String>,
    ) -> Self {
        Self {
            queue,
            store,
            client: WebhookHttpClient::default(),
            default_webhook_url,
            default_secret: default_secret.into(),
        }
    }

    /// Process a single incoming message from `QueueType::Webhook`.
    pub async fn process_one(&self) -> LogrixResult<bool> {
        let handle = match self.queue.consume(QueueType::Webhook).await? {
            Some(h) => h,
            None => return Ok(false),
        };

        let (event_type, chain_id, data) = match &handle.message {
            QueueMessage::Custom {
                job_type,
                chain_id,
                payload,
                ..
            } => (job_type.clone(), chain_id.as_u64(), payload.clone()),
            _ => (
                "unknown".to_string(),
                1,
                serde_json::json!({ "msg": "unknown_webhook_type" }),
            ),
        };

        let webhook_payload = WebhookPayload::new(&event_type, chain_id, data);
        let mut dispatched_urls = std::collections::HashSet::new();

        // 1. Dispatch to database-registered active endpoints
        let (mut any_delivered, mut any_failed) =
            crate::target::DispatchTargetOps::dispatch_endpoints(
                &self.client,
                &self.store,
                &event_type,
                &webhook_payload,
                &mut dispatched_urls,
            )
            .await?;

        // 2. Dispatch to CLI configured default URL if set and not already dispatched
        if let Some(url) = &self.default_webhook_url {
            if !dispatched_urls.contains(url) {
                let res = self
                    .client
                    .dispatch(url, &self.default_secret, &webhook_payload)
                    .await;
                if res.success {
                    any_delivered = true;
                } else {
                    any_failed = true;
                }
            }
        }

        // 3. Acknowledge or dead-letter based on delivery outcome
        if !any_failed && (any_delivered || self.default_webhook_url.is_none()) {
            self.queue.ack(&handle).await?;
            info!(event = %event_type, "Webhook processed and acknowledged");
        } else {
            error!(event = %event_type, "Webhook delivery failed or partially failed; routing to DLQ");
            self.queue
                .dead_letter(&handle, "webhook delivery failure")
                .await?;
        }

        Ok(true)
    }

    /// Background loop for the dedicated webhook pod with graceful shutdown support.
    pub async fn run_loop_with_shutdown<F>(&self, shutdown: F)
    where
        F: std::future::Future<Output = ()> + Send,
    {
        info!("Starting standalone WebhookDispatcherService consumer loop");
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                biased;
                _ = &mut shutdown => {
                    info!("WebhookDispatcherService received shutdown signal, draining in-flight work");
                    break;
                }
                res = self.process_one() => {
                    match res {
                        Ok(processed) => {
                            if !processed {
                                tokio::select! {
                                    biased;
                                    _ = &mut shutdown => {
                                        info!("WebhookDispatcherService received shutdown signal during sleep");
                                        break;
                                    }
                                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                                }
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "Error processing webhook queue item");
                            tokio::select! {
                                biased;
                                _ = &mut shutdown => {
                                    info!("WebhookDispatcherService received shutdown signal during error backoff");
                                    break;
                                }
                                _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                            }
                        }
                    }
                }
            }
        }
    }

    /// Background loop for the dedicated webhook pod.
    pub async fn run_loop(&self) {
        self.run_loop_with_shutdown(std::future::pending()).await;
    }
}
