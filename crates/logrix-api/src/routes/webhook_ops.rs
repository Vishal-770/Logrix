use super::webhook_dto::*;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use logrix_core::domain::{WebhookDelivery, WebhookPayload};
use logrix_webhook::generate_secret;
use std::collections::HashSet;
use uuid::Uuid;

pub fn validate_events(
    events: &[String],
    known: &HashSet<String>,
) -> Result<(), (StatusCode, String)> {
    if known.is_empty() {
        return Ok(());
    }
    for ev in events {
        if ev != "*" && !known.contains(ev) {
            let valid: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Invalid event '{ev}'. Available: {}", valid.join(", ")),
            ));
        }
    }
    Ok(())
}

pub fn validate_webhook_url(url: &str) -> Result<(), (StatusCode, String)> {
    let parsed = reqwest::Url::parse(url).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "Webhook URL must be a valid absolute URL".to_string(),
        )
    })?;

    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Webhook URL must use http or https and include a host".to_string(),
        ));
    }

    Ok(())
}

pub async fn rotate_secret_handler(
    State(state): State<super::webhooks::WebhookApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RotateSecretResponse>, (StatusCode, String)> {
    let new_secret = generate_secret();
    let rotated = state
        .store
        .rotate_secret(id, &new_secret)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if rotated {
        Ok(Json(RotateSecretResponse {
            id,
            secret: new_secret,
        }))
    } else {
        Err((StatusCode::NOT_FOUND, "Endpoint not found".into()))
    }
}

pub async fn test_webhook_handler(
    State(state): State<super::webhooks::WebhookApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<TestWebhookResponse>, (StatusCode, String)> {
    let ep = state
        .store
        .get_endpoint(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Endpoint not found".into()))?;

    let test_payload = WebhookPayload::new(
        "test.ping",
        0,
        serde_json::json!({ "message": "Logrix test webhook delivery", "endpoint_id": id }),
    );

    let res = state
        .http_client
        .dispatch(&ep.url, &ep.secret, &test_payload)
        .await;

    let delivery = WebhookDelivery {
        id: res.delivery_id,
        endpoint_id: Some(id),
        event_type: "test.ping".into(),
        payload: serde_json::to_value(&test_payload).unwrap_or_default(),
        status_code: res.status_code,
        success: res.success,
        attempt: 1,
        is_retry: false,
        error_message: res.error_message.clone(),
        latency_ms: res.latency_ms,
        created_at: chrono::Utc::now(),
    };
    let _ = state.store.record_delivery(&delivery).await;

    Ok(Json(TestWebhookResponse {
        id: res.delivery_id,
        status_code: res.status_code,
        success: res.success,
        latency_ms: res.latency_ms,
        error_message: res.error_message,
    }))
}

pub async fn list_deliveries_handler(
    State(state): State<super::webhooks::WebhookApiState>,
    Query(params): Query<DeliveriesQuery>,
) -> Result<Json<Vec<WebhookDelivery>>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(50).min(100);
    let deliveries = state
        .store
        .list_deliveries(params.endpoint_id, params.success, limit)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(deliveries))
}

pub async fn delete_webhook(
    State(state): State<super::webhooks::WebhookApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let deleted = state
        .store
        .delete_endpoint(id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Ok(StatusCode::NOT_FOUND)
    }
}

#[cfg(test)]
mod tests {
    use super::validate_webhook_url;

    #[test]
    fn accepts_http_and_https_webhook_urls() {
        assert!(validate_webhook_url("https://example.com/hooks/logrix").is_ok());
        assert!(validate_webhook_url("http://localhost:8080/events").is_ok());
    }

    #[test]
    fn rejects_relative_and_unsupported_webhook_urls() {
        assert!(validate_webhook_url("/hooks/logrix").is_err());
        assert!(validate_webhook_url("ftp://example.com/hooks").is_err());
        assert!(validate_webhook_url("not-a-url").is_err());
    }
}
