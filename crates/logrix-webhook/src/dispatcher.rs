use crate::client::WebhookHttpClient;
use crate::store::WebhookStore;
use chrono::Utc;
use logrix_core::{
    domain::{QueueMessage, QueueType, WebhookDelivery, WebhookPayload},
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
        let mut any_delivered = false;
        let mut any_failed = false;

        // 1. Dispatch to database-registered active endpoints
        if let Some(store) = &self.store {
            if let Ok(endpoints) = store.list_active_endpoints().await {
                for ep in endpoints {
                    if ep.events.is_empty() || ep.events.iter().any(|ev| ev == &event_type) {
                        let res = self
                            .client
                            .dispatch(&ep.url, &ep.secret, &webhook_payload)
                            .await;

                        let delivery = WebhookDelivery {
                            id: res.delivery_id,
                            endpoint_id: Some(ep.id),
                            event_type: event_type.clone(),
                            payload: serde_json::to_value(&webhook_payload).unwrap_or_default(),
                            status_code: res.status_code,
                            success: res.success,
                            error_message: res.error_message,
                            latency_ms: res.latency_ms,
                            created_at: Utc::now(),
                        };
                        let _ = store.record_delivery(&delivery).await;

                        if res.success {
                            any_delivered = true;
                        } else {
                            any_failed = true;
                        }
                    }
                }
            }
        }

        // 2. Dispatch to CLI configured default URL if set
        if let Some(url) = &self.default_webhook_url {
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

        // 3. Acknowledge or dead-letter based on delivery outcome
        if any_delivered || (!any_failed && self.default_webhook_url.is_none()) {
            self.queue.ack(&handle).await?;
            info!(event = %event_type, "Webhook processed and acknowledged");
        } else {
            error!(event = %event_type, "Failed to deliver webhook to destinations; routing to DLQ");
            self.queue
                .dead_letter(&handle, "webhook delivery failure")
                .await?;
        }

        Ok(true)
    }

    /// Background loop for the dedicated webhook pod.
    pub async fn run_loop(&self) {
        info!("Starting standalone WebhookDispatcherService consumer loop");
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
