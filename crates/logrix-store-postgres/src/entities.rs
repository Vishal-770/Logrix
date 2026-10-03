use crate::model::{DynamicEntityRecord, EntityInsert};
use crate::store::PostgresStore;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use sqlx::Row;

impl PostgresStore {
    /// Save a batch of dynamic schema entities idempotently and broadcast via pg_notify.
    pub async fn save_entities_batch(
        &self,
        chain_id: u64,
        block_number: u64,
        entities: &[EntityInsert],
    ) -> LogrixResult<()> {
        if entities.is_empty() {
            return Ok(());
        }

        let now = chrono::Utc::now();
        let mut chain_ids = Vec::with_capacity(entities.len());
        let mut entity_types = Vec::with_capacity(entities.len());
        let mut entity_ids = Vec::with_capacity(entities.len());
        let mut datas = Vec::with_capacity(entities.len());
        let mut block_numbers = Vec::with_capacity(entities.len());
        let mut is_reverteds = Vec::with_capacity(entities.len());
        let mut updated_ats = Vec::with_capacity(entities.len());

        for e in entities {
            chain_ids.push(chain_id as i64);
            entity_types.push(e.entity_type.clone());
            entity_ids.push(e.entity_id.clone());
            datas.push(e.data.clone());
            block_numbers.push(block_number as i64);
            is_reverteds.push(false);
            updated_ats.push(now);
        }

        let query = r#"
        INSERT INTO logrix_entities (chain_id, entity_type, entity_id, data, block_number, is_reverted, updated_at)
        SELECT * FROM UNNEST(
            $1::BIGINT[],
            $2::VARCHAR[],
            $3::VARCHAR[],
            $4::JSONB[],
            $5::BIGINT[],
            $6::BOOLEAN[],
            $7::TIMESTAMPTZ[]
        )
        ON CONFLICT (chain_id, entity_type, entity_id)
        DO UPDATE SET
            data = EXCLUDED.data,
            block_number = EXCLUDED.block_number,
            is_reverted = FALSE,
            reverted_at = NULL,
            updated_at = EXCLUDED.updated_at;
        "#;

        sqlx::query(query)
            .bind(&chain_ids)
            .bind(&entity_types)
            .bind(&entity_ids)
            .bind(&datas)
            .bind(&block_numbers)
            .bind(&is_reverteds)
            .bind(&updated_ats)
            .execute(self.pool())
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Database,
                    format!("Failed to batch upsert dynamic entities: {e}"),
                )
            })?;

        let payload = serde_json::json!({
            "chain_id": chain_id,
            "block_number": block_number,
            "count": entities.len()
        });
        let _ = sqlx::query("SELECT pg_notify('logrix_entity_mutations', $1)")
            .bind(payload.to_string())
            .execute(self.pool())
            .await;

        Ok(())
    }

    /// Retrieve a single dynamic entity by entity type and ID.
    pub async fn get_entity(
        &self,
        chain_id: u64,
        entity_type: &str,
        entity_id: &str,
    ) -> LogrixResult<Option<DynamicEntityRecord>> {
        let row = sqlx::query(
            r#"
            SELECT chain_id, entity_type, entity_id, data, block_number, is_reverted, updated_at
            FROM logrix_entities
            WHERE chain_id = $1 AND entity_type = $2 AND entity_id = $3 AND is_reverted = FALSE
            "#,
        )
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
}
