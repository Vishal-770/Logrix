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

/// Outgoing HTTP client for secure webhook dispatch.
#[derive(Clone)]
pub struct WebhookHttpClient {
    client: Client,
    retry_policy: RetryPolicy,
}

impl Default for WebhookHttpClient {
    fn default() -> Self {
        Self::new(Duration::from_secs(5), 1)
    }
}

impl WebhookHttpClient {
    pub fn new(timeout: Duration, max_retries: u32) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .expect("build reqwest client");

        let retry_policy = RetryPolicy::new(
            max_retries.max(1),
            Duration::from_millis(200),
            Duration::from_secs(4),
        );

        Self {
            client,
            retry_policy,
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
        let serialized = serde_json::to_vec(payload).unwrap_or_default();
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

            match req.send().await {
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
                    last_error = Some(format!("HTTP status {status}"));
                    warn!(
                        url,
                        status, attempt, "Webhook destination returned non-success"
                    );
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                    warn!(url, error = %e, attempt, "Failed to connect to webhook URL");
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
