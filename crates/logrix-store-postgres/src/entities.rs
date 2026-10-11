use crate::model::EntityInsert;
use crate::store::PostgresStore;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};

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

        let payloads: Vec<String> = entities
            .iter()
            .map(|e| {
                serde_json::json!({
                    "chain_id": chain_id,
                    "entity_type": e.entity_type,
                    "entity_id": e.entity_id,
                    "block_number": block_number,
                    "data": e.data
                })
                .to_string()
            })
            .filter(|s| s.len() < 7900)
            .collect();

        if !payloads.is_empty() {
            if let Err(e) = sqlx::query(
                "SELECT pg_notify('logrix_entity_mutations', p) FROM UNNEST($1::TEXT[]) AS p",
            )
            .bind(&payloads)
            .execute(self.pool())
            .await
            {
                tracing::warn!(error = %e, "Failed to broadcast pg_notify entity mutations");
            }
        }

        Ok(())
    }

    /// Soft-delete a single entity by marking it as reverted.
    /// Used by Entity.remove() from the WASM handler -- key format is "EntityType:id".
    pub async fn delete_entity(
        &self,
        chain_id: u64,
        entity_type: &str,
        entity_id: &str,
    ) -> LogrixResult<()> {
        let now = chrono::Utc::now();
        let query = "UPDATE logrix_entities SET is_reverted = TRUE, reverted_at = $4 \
                     WHERE chain_id = $1 AND entity_type = $2 AND entity_id = $3 AND is_reverted = FALSE";
        sqlx::query(query)
            .bind(chain_id as i64)
            .bind(entity_type)
            .bind(entity_id)
            .bind(now)
            .execute(self.pool())
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Database,
                    format!("Failed to soft-delete entity {entity_type}:{entity_id}: {e}"),
                )
            })?;

        Ok(())
    }
}
