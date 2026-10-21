use crate::signature::sign_payload;
use logrix_core::domain::WebhookPayload;
use logrix_resilience::RetryPolicy;
use reqwest::Client;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracing::warn;
use uuid::Uuid;

/// Result summary of an attempted webhook delivery.
#[derive(Debug, Clone)]
pub struct DeliveryResult {
    pub delivery_id: Uuid,
    pub status_code: Option<u16>,
    pub success: bool,
    pub attempts_taken: u32,
    pub error_message: Option<String>,
    pub latency_ms: u64,
}

use std::sync::Arc;
use tokio::sync::Semaphore;

/// Outgoing HTTP client for secure webhook dispatch.
#[derive(Clone)]
pub struct WebhookHttpClient {
    client: Client,
    retry_policy: RetryPolicy,
    semaphore: Arc<Semaphore>,
}

impl Default for WebhookHttpClient {
    fn default() -> Self {
        Self::new(Duration::from_secs(5), 1)
    }
}

impl WebhookHttpClient {
    pub fn new(timeout: Duration, max_retries: u32) -> Self {
        Self::with_config(&crate::config::WebhookConfig {
            timeout,
            max_retries,
            ..Default::default()
        })
    }

    pub fn with_config(config: &crate::config::WebhookConfig) -> Self {
        let client = Client::builder()
            .timeout(config.timeout)
            .pool_idle_timeout(config.pool_idle_timeout)
            .pool_max_idle_per_host(config.pool_max_idle_per_host)
            .build()
            .expect("build reqwest client");

        let retry_policy = RetryPolicy::new(
            config.max_retries.max(1),
            config.retry_initial_delay,
            config.retry_max_delay,
        );

        let concurrency = config.max_concurrent_dispatches.max(1);
        Self {
            client,
            retry_policy,
            semaphore: Arc::new(Semaphore::new(concurrency)),
        }
    }

    /// Dispatch signed webhook payload with custom max_retries.
    pub async fn dispatch_with_retries(
        &self,
        url: &str,
        secret: &str,
        payload: &WebhookPayload,
        max_attempts: u32,
    ) -> DeliveryResult {
        let delivery_id = Uuid::new_v4();
        let serialized = match serde_json::to_vec(payload) {
            Ok(bytes) => bytes,
            Err(e) => {
                return DeliveryResult {
                    delivery_id,
                    status_code: None,
                    success: false,
                    attempts_taken: 0,
                    error_message: Some(format!("Failed to serialize webhook payload: {e}")),
                    latency_ms: 0,
                };
            }
        };
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let signature = sign_payload(secret, timestamp, &serialized);

        let mut attempt = 0;
        let mut last_status: Option<u16> = None;
        let mut last_error: Option<String> = None;
        let start = Instant::now();

        let attempts_limit = max_attempts.max(1);
        while attempt < attempts_limit {
            attempt += 1;

            let req = self
                .client
                .post(url)
                .header("Content-Type", "application/json")
                .header("X-Logrix-Delivery-ID", delivery_id.to_string())
                .header("X-Logrix-Event", &payload.event)
                .header("X-Logrix-Chain-Id", payload.chain_id.to_string())
                .header("X-Logrix-Signature", &signature)
                .body(serialized.clone());

            let send_result = match self.semaphore.acquire().await {
                Ok(_permit) => req.send().await,
                Err(e) => {
                    last_error = Some(format!("Concurrency semaphore closed: {e}"));
                    break;
                }
            };

            match send_result {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    last_status = Some(status);
                    if resp.status().is_success() {
                        return DeliveryResult {
                            delivery_id,
                            status_code: Some(status),
                            success: true,
                            attempts_taken: attempt,
                            error_message: None,
                            latency_ms: start.elapsed().as_millis() as u64,
                        };
                    }
                    let safe_url = crate::signature::sanitize_webhook_url(url);
                    last_error = Some(format!("HTTP status {status}"));
                    warn!(
                        url = %safe_url,
                        status, attempt, "Webhook destination returned non-success"
                    );
                }
                Err(e) => {
                    let safe_url = crate::signature::sanitize_webhook_url(url);
                    last_error = Some(e.to_string());
                    warn!(url = %safe_url, error = %e, attempt, "Failed to connect to webhook URL");
                }
            }

            if attempt < attempts_limit {
                tokio::time::sleep(self.retry_policy.delay_for_attempt(attempt)).await;
            }
        }

        DeliveryResult {
            delivery_id,
            status_code: last_status,
            success: false,
            attempts_taken: attempt,
            error_message: last_error,
            latency_ms: start.elapsed().as_millis() as u64,
        }
    }

    /// Dispatch signed webhook payload with default at-most-once single attempt.
    pub async fn dispatch(
        &self,
        url: &str,
        secret: &str,
        payload: &WebhookPayload,
    ) -> DeliveryResult {
        self.dispatch_with_retries(url, secret, payload, 1).await
    }
}
