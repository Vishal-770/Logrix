use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use logrix_core::domain::{WebhookDelivery, WebhookEndpoint};
use logrix_webhook::{generate_secret, WebhookStore};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone)]
pub struct WebhookApiState {
    pub store: WebhookStore,
}

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub url: String,
    pub events: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct CreateWebhookResponse {
    pub id: Uuid,
    pub url: String,
    pub secret: String,
    pub events: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct WebhookEndpointDto {
    pub id: Uuid,
    pub url: String,
    pub masked_secret: String,
    pub events: Vec<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

pub fn webhook_routes(state: WebhookApiState) -> Router {
    Router::new()
        .route("/api/v1/webhooks", post(create_webhook).get(list_webhooks))
        .route("/api/v1/webhooks/{id}", delete(delete_webhook))
        .route("/api/v1/webhooks/deliveries", get(list_deliveries))
        .with_state(state)
}

async fn create_webhook(
    State(state): State<WebhookApiState>,
    Json(payload): Json<CreateWebhookRequest>,
) -> Result<(StatusCode, Json<CreateWebhookResponse>), (StatusCode, String)> {
    let secret = generate_secret();
    let events = payload.events.unwrap_or_default();
    let endpoint = WebhookEndpoint::new(payload.url.clone(), secret.clone(), events.clone());

    state
        .store
        .create_endpoint(&endpoint)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateWebhookResponse {
            id: endpoint.id,
            url: endpoint.url,
            secret,
            events,
            created_at: endpoint.created_at,
        }),
    ))
}

async fn list_webhooks(
    State(state): State<WebhookApiState>,
) -> Result<Json<Vec<WebhookEndpointDto>>, (StatusCode, String)> {
    let endpoints = state
        .store
        .list_active_endpoints()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let dtos = endpoints
        .into_iter()
        .map(|ep| {
            let masked_secret = ep.masked_secret();
            WebhookEndpointDto {
                id: ep.id,
                url: ep.url,
                masked_secret,
                events: ep.events,
                is_active: ep.is_active,
                created_at: ep.created_at,
            }
        })
        .collect();

    Ok(Json(dtos))
}

async fn delete_webhook(
    State(state): State<WebhookApiState>,
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

async fn list_deliveries(
    State(state): State<WebhookApiState>,
) -> Result<Json<Vec<WebhookDelivery>>, (StatusCode, String)> {
    let deliveries = state
        .store
        .list_deliveries(50)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(deliveries))
}
