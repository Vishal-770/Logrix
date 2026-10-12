use crate::client::WebhookHttpClient;
use crate::store::WebhookStore;
use chrono::Utc;
use logrix_core::domain::{WebhookDelivery, WebhookEndpoint, WebhookPayload};
use logrix_core::error::LogrixResult;
use std::collections::HashSet;

pub struct DispatchTargetOps;

impl DispatchTargetOps {
    pub async fn dispatch_endpoint(
        client: &WebhookHttpClient,
        store: &WebhookStore,
        ep: &WebhookEndpoint,
        payload: &WebhookPayload,
    ) -> bool {
        let attempts = ep.max_retries.saturating_add(1);
        let res = client
            .dispatch_with_retries(&ep.url, &ep.secret, payload, attempts)
            .await;

        let delivery = WebhookDelivery {
            id: res.delivery_id,
            endpoint_id: Some(ep.id),
            event_type: payload.event.clone(),
            payload: serde_json::to_value(payload).unwrap_or_default(),
            status_code: res.status_code,
            success: res.success,
            attempt: res.attempts_taken,
            is_retry: res.attempts_taken > 1,
            error_message: res.error_message,
            latency_ms: res.latency_ms,
            created_at: Utc::now(),
        };
        let _ = store.record_delivery(&delivery).await;
        res.success
    }

    pub async fn dispatch_endpoints(
        client: &WebhookHttpClient,
        store: &Option<WebhookStore>,
        event_type: &str,
        payload: &WebhookPayload,
        dispatched_urls: &mut HashSet<String>,
    ) -> LogrixResult<(bool, bool)> {
        let mut any_delivered = false;
        let mut any_failed = false;

        if let Some(store) = store {
            let endpoints = store.list_active_endpoints().await?;
            for ep in endpoints {
                if ep.events.is_empty() || ep.events.iter().any(|ev| ev == event_type) {
                    dispatched_urls.insert(ep.url.clone());
                    if Self::dispatch_endpoint(client, store, &ep, payload).await {
                        any_delivered = true;
                    } else {
                        any_failed = true;
                    }
                }
            }
        }

        Ok((any_delivered, any_failed))
    }
}
