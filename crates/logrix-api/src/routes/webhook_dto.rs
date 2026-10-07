use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateWebhookRequest {
    pub url: String,
    pub events: Option<Vec<String>>,
    pub max_retries: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWebhookRequest {
    pub url: Option<String>,
    pub events: Option<Vec<String>>,
    pub is_active: Option<bool>,
    pub max_retries: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct CreateWebhookResponse {
    pub id: Uuid,
    pub url: String,
    pub secret: String,
    pub events: Vec<String>,
    pub max_retries: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RotateSecretResponse {
    pub id: Uuid,
    pub secret: String,
}

#[derive(Debug, Serialize)]
pub struct TestWebhookResponse {
    pub id: Uuid,
    pub status_code: Option<u16>,
    pub success: bool,
    pub latency_ms: u64,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WebhookEndpointDto {
    pub id: Uuid,
    pub url: String,
    pub masked_secret: String,
    pub events: Vec<String>,
    pub is_active: bool,
    pub max_retries: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DeliveriesQuery {
    pub endpoint_id: Option<Uuid>,
    pub success: Option<bool>,
    pub limit: Option<i64>,
}
