use super::webhook_dto::*;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use logrix_core::domain::{WebhookDelivery, WebhookPayload};
use logrix_webhook::{generate_secret, WebhookHttpClient};
use uuid::Uuid;

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
        Ok(Json(RotateSecretResponse { id, secret: new_secret }))
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

    let client = WebhookHttpClient::default();
    let test_payload = WebhookPayload::new(
        "test.ping",
        0,
        serde_json::json!({ "message": "Logrix test webhook delivery", "endpoint_id": id }),
    );

    let res = client.dispatch(&ep.url, &ep.secret, &test_payload).await;

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
