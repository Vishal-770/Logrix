use crate::client::WebhookHttpClient;
use crate::store::WebhookStore;
use chrono::Utc;
use futures::stream::{FuturesUnordered, StreamExt};
use logrix_core::domain::{WebhookDelivery, WebhookEndpoint, WebhookPayload};
use logrix_core::error::LogrixResult;
use std::collections::HashSet;
use tracing::error;

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

        let payload_json = match serde_json::to_value(payload) {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "Failed to serialize webhook payload for delivery record");
                serde_json::json!({ "error": "serialization_failure", "event": payload.event })
            }
        };

        let delivery = WebhookDelivery {
            id: res.delivery_id,
            endpoint_id: Some(ep.id),
            event_type: payload.event.clone(),
            payload: payload_json,
            status_code: res.status_code,
            success: res.success,
            attempt: res.attempts_taken,
            is_retry: res.attempts_taken > 1,
            error_message: res.error_message,
            latency_ms: res.latency_ms,
            created_at: Utc::now(),
        };

        if let Err(e) = store.record_delivery(&delivery).await {
            error!(
                delivery_id = %delivery.id,
                endpoint_id = ?delivery.endpoint_id,
                error = %e,
                "Failed to persist webhook delivery audit record"
            );
        }

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
            let matching: Vec<_> = endpoints
                .into_iter()
                .filter(|ep| ep.events.is_empty() || ep.events.iter().any(|ev| ev == event_type))
                .collect();

            for ep in &matching {
                dispatched_urls.insert(ep.url.clone());
            }

            let mut tasks = FuturesUnordered::new();
            for ep in matching {
                tasks.push(
                    async move { Self::dispatch_endpoint(client, store, &ep, payload).await },
                );
            }

            while let Some(success) = tasks.next().await {
                if success {
                    any_delivered = true;
                } else {
                    any_failed = true;
                }
            }
        }

        Ok((any_delivered, any_failed))
    }
}
