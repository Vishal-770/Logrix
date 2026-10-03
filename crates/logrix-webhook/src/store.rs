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

    /// Insert a new webhook endpoint.
    pub async fn create_endpoint(&self, ep: &WebhookEndpoint) -> LogrixResult<()> {
        sqlx::query(
            r#"
            INSERT INTO logrix_webhook_endpoints (id, url, secret, events, is_active, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(ep.id)
        .bind(&ep.url)
        .bind(&ep.secret)
        .bind(&ep.events)
        .bind(ep.is_active)
        .bind(ep.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Database,
                format!("Failed to insert webhook endpoint: {e}"),
            )
        })?;

        Ok(())
    }

    /// Fetch all active webhook endpoints.
    pub async fn list_active_endpoints(&self) -> LogrixResult<Vec<WebhookEndpoint>> {
        let rows = sqlx::query(
            r#"
            SELECT id, url, secret, events, is_active, created_at
            FROM logrix_webhook_endpoints
            WHERE is_active = true
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to list webhook endpoints: {e}"),
            )
        })?;

        let endpoints = rows
            .into_iter()
            .map(|r| WebhookEndpoint {
                id: r.get("id"),
                url: r.get("url"),
                secret: r.get("secret"),
                events: r.get("events"),
                is_active: r.get("is_active"),
                created_at: r.get("created_at"),
            })
            .collect();

        Ok(endpoints)
    }

    /// Delete a webhook endpoint by ID.
    pub async fn delete_endpoint(&self, id: Uuid) -> LogrixResult<bool> {
        let res = sqlx::query("DELETE FROM logrix_webhook_endpoints WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Database,
                    format!("Failed to delete webhook endpoint: {e}"),
                )
            })?;

        Ok(res.rows_affected() > 0)
    }

    /// Record a webhook delivery attempt in the audit table.
    pub async fn record_delivery(&self, del: &WebhookDelivery) -> LogrixResult<()> {
        let status_code = del.status_code.map(|s| s as i32);

        sqlx::query(
            r#"
            INSERT INTO logrix_webhook_deliveries (
                id, endpoint_id, event_type, payload, status_code, success, error_message, latency_ms, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(del.id)
        .bind(del.endpoint_id)
        .bind(&del.event_type)
        .bind(&del.payload)
        .bind(status_code)
        .bind(del.success)
        .bind(&del.error_message)
        .bind(del.latency_ms as i64)
        .bind(del.created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to record webhook delivery: {e}"),
            )
        })?;

        Ok(())
    }

    /// List recent webhook deliveries for debugging.
    pub async fn list_deliveries(&self, limit: i64) -> LogrixResult<Vec<WebhookDelivery>> {
        let rows = sqlx::query(
            r#"
            SELECT id, endpoint_id, event_type, payload, status_code, success, error_message, latency_ms, created_at
            FROM logrix_webhook_deliveries
            ORDER BY created_at DESC
            LIMIT $1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to list webhook deliveries: {e}"),
            )
        })?;

        let deliveries = rows
            .into_iter()
            .map(|r| {
                let status_i32: Option<i32> = r.get("status_code");
                let lat_i64: i64 = r.get("latency_ms");
                WebhookDelivery {
                    id: r.get("id"),
                    endpoint_id: r.get("endpoint_id"),
                    event_type: r.get("event_type"),
                    payload: r.get("payload"),
                    status_code: status_i32.map(|s| s as u16),
                    success: r.get("success"),
                    error_message: r.get("error_message"),
                    latency_ms: lat_i64 as u64,
                    created_at: r.get("created_at"),
                }
            })
            .collect();

        Ok(deliveries)
    }
}
