use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Webhook endpoint subscription configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookEndpoint {
    pub id: Uuid,
    pub url: String,
    pub secret: String,
    pub events: Vec<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

impl WebhookEndpoint {
    pub fn new(url: impl Into<String>, secret: impl Into<String>, events: Vec<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            url: url.into(),
            secret: secret.into(),
            events,
            is_active: true,
            created_at: Utc::now(),
        }
    }

    /// Return a masked representation of the signing secret for API responses.
    #[must_use]
    pub fn masked_secret(&self) -> String {
        if self.secret.len() > 8 {
            format!(
                "{}...{}",
                &self.secret[..6],
                &self.secret[self.secret.len() - 4..]
            )
        } else {
            "whsec_****".to_string()
        }
    }
}

/// Outgoing webhook delivery envelope sent to external HTTP endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookPayload {
    pub event: String,
    pub chain_id: u64,
    pub timestamp: u64,
    pub data: serde_json::Value,
}

impl WebhookPayload {
    pub fn new(event: impl Into<String>, chain_id: u64, data: serde_json::Value) -> Self {
        Self {
            event: event.into(),
            chain_id,
            timestamp: Utc::now().timestamp() as u64,
            data,
        }
    }
}

/// Record of an attempted webhook delivery for auditing and observability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookDelivery {
    pub id: Uuid,
    pub endpoint_id: Option<Uuid>,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub status_code: Option<u16>,
    pub success: bool,
    pub error_message: Option<String>,
    pub latency_ms: u64,
    pub created_at: DateTime<Utc>,
}
