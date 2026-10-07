use crate::deliveries::DeliveryOps;
use logrix_core::{
    domain::{WebhookDelivery, WebhookEndpoint},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Webhook database repository for Postgres.
#[derive(Clone)]
pub struct WebhookStore {
    pool: PgPool,
}

impl WebhookStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Insert a new webhook endpoint.
    pub async fn create_endpoint(&self, ep: &WebhookEndpoint) -> LogrixResult<()> {
        sqlx::query(
            "INSERT INTO logrix_webhook_endpoints (id, url, secret, events, is_active, created_at) VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(ep.id).bind(&ep.url).bind(&ep.secret).bind(&ep.events).bind(ep.is_active).bind(ep.created_at)
        .execute(&self.pool).await
        .map_err(|e| LogrixError::new(ErrorClass::Permanent, ErrorSource::Database, format!("Failed to insert endpoint: {e}")))?;
        Ok(())
    }

    /// Update an existing webhook endpoint's URL, events filter, and active status.
    pub async fn update_endpoint(
        &self,
        id: Uuid,
        url: Option<&str>,
        events: Option<&[String]>,
        is_active: Option<bool>,
        max_retries: Option<u32>,
    ) -> LogrixResult<Option<WebhookEndpoint>> {
        let current = match self.get_endpoint(id).await? {
            Some(ep) => ep,
            None => return Ok(None),
        };
        let new_url = url.unwrap_or(&current.url);
        let new_events = events.map(|ev| ev.to_vec()).unwrap_or(current.events);
        let new_active = is_active.unwrap_or(current.is_active);
        let new_retries = max_retries.unwrap_or(current.max_retries);

        sqlx::query("UPDATE logrix_webhook_endpoints SET url = $1, events = $2, is_active = $3 WHERE id = $4")
            .bind(new_url).bind(&new_events).bind(new_active).bind(id)
            .execute(&self.pool).await
            .map_err(|e| LogrixError::new(ErrorClass::Transient, ErrorSource::Database, format!("Update failed: {e}")))?;

        let mut updated = self.get_endpoint(id).await?;
        if let Some(ref mut ep) = updated {
            ep.max_retries = new_retries;
        }
        Ok(updated)
    }

    /// Rotate secret for an existing endpoint.
    pub async fn rotate_secret(&self, id: Uuid, new_secret: &str) -> LogrixResult<bool> {
        let res = sqlx::query("UPDATE logrix_webhook_endpoints SET secret = $1 WHERE id = $2")
            .bind(new_secret).bind(id).execute(&self.pool).await
            .map_err(|e| LogrixError::new(ErrorClass::Transient, ErrorSource::Database, format!("Rotate failed: {e}")))?;
        Ok(res.rows_affected() > 0)
    }

    /// Retrieve an endpoint by its unique identifier.
    pub async fn get_endpoint(&self, id: Uuid) -> LogrixResult<Option<WebhookEndpoint>> {
        let row = sqlx::query(
            "SELECT id, url, secret, events, is_active, created_at FROM logrix_webhook_endpoints WHERE id = $1"
        )
        .bind(id).fetch_optional(&self.pool).await
        .map_err(|e| LogrixError::new(ErrorClass::Transient, ErrorSource::Database, format!("Fetch failed: {e}")))?;

        Ok(row.map(|r| WebhookEndpoint {
            id: r.get("id"), url: r.get("url"), secret: r.get("secret"),
            events: r.get("events"), is_active: r.get("is_active"), max_retries: 0, created_at: r.get("created_at"),
        }))
    }

    /// Fetch all active webhook endpoints.
    pub async fn list_active_endpoints(&self) -> LogrixResult<Vec<WebhookEndpoint>> {
        let rows = sqlx::query(
            "SELECT id, url, secret, events, is_active, created_at FROM logrix_webhook_endpoints WHERE is_active = true ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool).await
        .map_err(|e| LogrixError::new(ErrorClass::Transient, ErrorSource::Database, format!("List failed: {e}")))?;

        let endpoints = rows.into_iter().map(|r| WebhookEndpoint {
            id: r.get("id"), url: r.get("url"), secret: r.get("secret"),
            events: r.get("events"), is_active: r.get("is_active"), max_retries: 0, created_at: r.get("created_at"),
        }).collect();
        Ok(endpoints)
    }

    /// Delete a webhook endpoint by ID.
    pub async fn delete_endpoint(&self, id: Uuid) -> LogrixResult<bool> {
        let res = sqlx::query("DELETE FROM logrix_webhook_endpoints WHERE id = $1")
            .bind(id).execute(&self.pool).await
            .map_err(|e| LogrixError::new(ErrorClass::Transient, ErrorSource::Database, format!("Delete failed: {e}")))?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn record_delivery(&self, del: &WebhookDelivery) -> LogrixResult<()> {
        DeliveryOps::record(&self.pool, del).await
    }

    pub async fn list_deliveries(
        &self, endpoint_id: Option<Uuid>, success: Option<bool>, limit: i64,
    ) -> LogrixResult<Vec<WebhookDelivery>> {
        DeliveryOps::list(&self.pool, endpoint_id, success, limit).await
    }
}
