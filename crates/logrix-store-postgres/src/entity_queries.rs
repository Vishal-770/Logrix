use crate::model::DynamicEntityRecord;
use crate::store::PostgresStore;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use sqlx::Row;

impl PostgresStore {
    /// Retrieve a single dynamic entity by entity type and ID.
    pub async fn get_entity(
        &self,
        chain_id: u64,
        entity_type: &str,
        entity_id: &str,
    ) -> LogrixResult<Option<DynamicEntityRecord>> {
        let query = "SELECT chain_id, entity_type, entity_id, data, block_number, is_reverted, updated_at \
                     FROM logrix_entities \
                     WHERE chain_id = $1 AND entity_type = $2 AND entity_id = $3 AND is_reverted = FALSE";
        let row = sqlx::query(query)
            .bind(chain_id as i64)
            .bind(entity_type)
            .bind(entity_id)
            .fetch_optional(self.pool())
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Database,
                    format!("Failed to get dynamic entity: {e}"),
                )
            })?;

        Ok(row.map(|r| DynamicEntityRecord {
            chain_id: r.get::<i64, _>("chain_id") as u64,
            entity_type: r.get("entity_type"),
            entity_id: r.get("entity_id"),
            data: r.get("data"),
            block_number: r.get::<i64, _>("block_number") as u64,
            is_reverted: r.get("is_reverted"),
            updated_at: r.get("updated_at"),
        }))
    }

    /// Retrieve paginated dynamic entities by entity type.
    pub async fn get_entities_by_type(
        &self,
        chain_id: u64,
        entity_type: &str,
        limit: i64,
        offset: i64,
    ) -> LogrixResult<Vec<DynamicEntityRecord>> {
        let safe_limit = limit.clamp(1, 1000);
        let safe_offset = offset.max(0);

        let query =
            "SELECT chain_id, entity_type, entity_id, data, block_number, is_reverted, updated_at \
                     FROM logrix_entities \
                     WHERE chain_id = $1 AND entity_type = $2 AND is_reverted = FALSE \
                     ORDER BY block_number DESC, entity_id ASC \
                     LIMIT $3 OFFSET $4";

        let rows = sqlx::query(query)
            .bind(chain_id as i64)
            .bind(entity_type)
            .bind(safe_limit)
            .bind(safe_offset)
            .fetch_all(self.pool())
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Database,
                    format!("Failed to query entities by type {entity_type}: {e}"),
                )
            })?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(DynamicEntityRecord {
                chain_id: row.get::<i64, _>("chain_id") as u64,
                entity_type: row.get("entity_type"),
                entity_id: row.get("entity_id"),
                data: row.get("data"),
                block_number: row.get::<i64, _>("block_number") as u64,
                is_reverted: row.get("is_reverted"),
                updated_at: row.get("updated_at"),
            });
        }
        Ok(results)
    }

    /// Query dynamic entities matching custom SQL filter.
    pub async fn query_entities_raw(&self, query: &str) -> LogrixResult<Vec<DynamicEntityRecord>> {
        let rows = sqlx::query(query)
            .fetch_all(self.pool())
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Database,
                    format!("Failed to query dynamic entities: {e}"),
                )
            })?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            results.push(DynamicEntityRecord {
                chain_id: row.try_get::<i64, _>("chain_id").unwrap_or(0) as u64,
                entity_type: row.try_get("entity_type").unwrap_or_default(),
                entity_id: row.try_get("entity_id").unwrap_or_default(),
                data: row.try_get("data").unwrap_or(serde_json::Value::Null),
                block_number: row.try_get::<i64, _>("block_number").unwrap_or(0) as u64,
                is_reverted: row.try_get("is_reverted").unwrap_or(false),
                updated_at: row
                    .try_get("updated_at")
                    .unwrap_or_else(|_| chrono::Utc::now()),
            });
        }
        Ok(results)
    }
}
