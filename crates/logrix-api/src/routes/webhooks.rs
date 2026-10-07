use super::webhook_dto::*;
use super::webhook_ops::{rotate_secret_handler, test_webhook_handler};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use logrix_core::domain::{WebhookDelivery, WebhookEndpoint};
use logrix_webhook::{generate_secret, WebhookStore};
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct WebhookApiState {
    pub store: WebhookStore,
    pub known_events: Arc<HashSet<String>>,
}

impl WebhookApiState {
    pub fn new(store: WebhookStore, known_events: HashSet<String>) -> Self {
        Self { store, known_events: Arc::new(known_events) }
    }
}

pub fn webhook_routes(state: WebhookApiState) -> Router {
    Router::new()
        .route("/api/v1/webhooks", post(create_webhook).get(list_webhooks))
        .route("/api/v1/webhooks/{id}", delete(delete_webhook).patch(update_webhook).get(get_webhook))
        .route("/api/v1/webhooks/{id}/rotate-secret", post(rotate_secret_handler))
        .route("/api/v1/webhooks/{id}/test", post(test_webhook_handler))
        .route("/api/v1/webhooks/deliveries", get(list_deliveries))
        .with_state(state)
}

fn validate_events(events: &[String], known: &HashSet<String>) -> Result<(), (StatusCode, String)> {
    if known.is_empty() { return Ok(()); }
    for ev in events {
        if ev != "*" && !known.contains(ev) {
            let valid: Vec<&str> = known.iter().map(|s| s.as_str()).collect();
            return Err((StatusCode::BAD_REQUEST, format!("Invalid event '{ev}'. Available: {}", valid.join(", "))));
        }
    }
    Ok(())
}

async fn create_webhook(
    State(state): State<WebhookApiState>,
    Json(payload): Json<CreateWebhookRequest>,
) -> Result<(StatusCode, Json<CreateWebhookResponse>), (StatusCode, String)> {
    let events = payload.events.unwrap_or_default();
    validate_events(&events, &state.known_events)?;
    let secret = generate_secret();
    let endpoint = WebhookEndpoint::new(payload.url.clone(), secret.clone(), events.clone());

    state.store.create_endpoint(&endpoint).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok((StatusCode::CREATED, Json(CreateWebhookResponse {
        id: endpoint.id, url: endpoint.url, secret, events, created_at: endpoint.created_at,
    })))
}

async fn update_webhook(
    State(state): State<WebhookApiState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateWebhookRequest>,
) -> Result<Json<WebhookEndpointDto>, (StatusCode, String)> {
    if let Some(ref evs) = payload.events { validate_events(evs, &state.known_events)?; }
    let updated = state.store.update_endpoint(
        id, payload.url.as_deref(), payload.events.as_deref(), payload.is_active,
    ).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match updated {
        Some(ep) => {
            let masked_secret = ep.masked_secret();
            Ok(Json(WebhookEndpointDto {
                id: ep.id, url: ep.url, masked_secret,
                events: ep.events, is_active: ep.is_active, created_at: ep.created_at,
            }))
        }
        None => Err((StatusCode::NOT_FOUND, "Endpoint not found".into())),
    }
}

async fn get_webhook(
    State(state): State<WebhookApiState>,
    Path(id): Path<Uuid>,
) -> Result<Json<WebhookEndpointDto>, (StatusCode, String)> {
    let ep = state.store.get_endpoint(id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Endpoint not found".into()))?;

    let masked_secret = ep.masked_secret();
    Ok(Json(WebhookEndpointDto {
        id: ep.id, url: ep.url, masked_secret,
        events: ep.events, is_active: ep.is_active, created_at: ep.created_at,
    }))
}

async fn list_webhooks(
    State(state): State<WebhookApiState>,
) -> Result<Json<Vec<WebhookEndpointDto>>, (StatusCode, String)> {
    let endpoints = state.store.list_active_endpoints().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let dtos = endpoints.into_iter().map(|ep| {
        let masked_secret = ep.masked_secret();
        WebhookEndpointDto {
            id: ep.id, url: ep.url, masked_secret,
            events: ep.events, is_active: ep.is_active, created_at: ep.created_at,
        }
    }).collect();
    Ok(Json(dtos))
}

async fn delete_webhook(
    State(state): State<WebhookApiState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, (StatusCode, String)> {
    let deleted = state.store.delete_endpoint(id).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if deleted { Ok(StatusCode::NO_CONTENT) } else { Ok(StatusCode::NOT_FOUND) }
}

async fn list_deliveries(
    State(state): State<WebhookApiState>,
    Query(params): Query<DeliveriesQuery>,
) -> Result<Json<Vec<WebhookDelivery>>, (StatusCode, String)> {
    let limit = params.limit.unwrap_or(50).min(100);
    let deliveries = state.store.list_deliveries(params.endpoint_id, params.success, limit).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(deliveries))
}
