use logrix_core::{
    domain::WebhookDelivery,
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Delivery audit record queries for PostgreSQL.
pub struct DeliveryOps;

impl DeliveryOps {
    pub async fn record(pool: &PgPool, del: &WebhookDelivery) -> LogrixResult<()> {
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
        .execute(pool)
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

    pub async fn list(
        pool: &PgPool,
        endpoint_id: Option<Uuid>,
        success: Option<bool>,
        limit: i64,
    ) -> LogrixResult<Vec<WebhookDelivery>> {
        let rows = sqlx::query(
            r#"
            SELECT id, endpoint_id, event_type, payload, status_code, success, error_message, latency_ms, created_at
            FROM logrix_webhook_deliveries
            WHERE ($1::uuid IS NULL OR endpoint_id = $1)
              AND ($2::bool IS NULL OR success = $2)
            ORDER BY created_at DESC
            LIMIT $3
            "#,
        )
        .bind(endpoint_id)
        .bind(success)
        .bind(limit)
        .fetch_all(pool)
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
