use crate::store::PostgresStore;
use async_trait::async_trait;
use logrix_core::{
    domain::{ChainId, Checkpoint, EventLog},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::StorePort,
};
use sqlx::Row;
use tracing::debug;

#[async_trait]
impl StorePort for PostgresStore {
    async fn write_events_and_checkpoint(
        &self,
        events: &[EventLog],
        checkpoint: &Checkpoint,
    ) -> LogrixResult<()> {
        let mut tx = self.pool().begin().await.map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to start transaction: {e}"),
            )
        })?;

        // 1. Vectorized bulk insert of raw event logs
        if !events.is_empty() {
            let mut chain_ids = Vec::with_capacity(events.len());
            let mut block_numbers = Vec::with_capacity(events.len());
            let mut block_hashes = Vec::with_capacity(events.len());
            let mut tx_hashes = Vec::with_capacity(events.len());
            let mut tx_indices = Vec::with_capacity(events.len());
            let mut log_indices = Vec::with_capacity(events.len());
            let mut contract_addresses = Vec::with_capacity(events.len());
            let mut topic0s = Vec::with_capacity(events.len());
            let mut topics_arr: Vec<serde_json::Value> = Vec::with_capacity(events.len());
            let mut data_arr = Vec::with_capacity(events.len());

            for e in events {
                chain_ids.push(checkpoint.chain_id.as_u64() as i64);
                block_numbers.push(e.block_number as i64);
                block_hashes.push(format!("{:#x}", e.block_hash));
                tx_hashes.push(format!("{:#x}", e.tx_hash));
                tx_indices.push(e.tx_index as i64);
                log_indices.push(e.log_index as i64);
                contract_addresses.push(format!("{:#x}", e.address));
                topic0s.push(e.topics.first().map(|t| format!("{:#x}", t)));
                let topics_json: Vec<String> =
                    e.topics.iter().map(|t| format!("{:#x}", t)).collect();
                topics_arr.push(serde_json::json!(topics_json));
                data_arr.push(e.data.to_vec());
            }

            sqlx::query(
                r#"
                INSERT INTO logrix_events (
                    chain_id, block_number, block_hash, tx_hash, tx_index, log_index,
                    contract_address, topic0, topics, data
                )
                SELECT * FROM UNNEST(
                    $1::bigint[], $2::bigint[], $3::varchar[], $4::varchar[], $5::bigint[],
                    $6::bigint[], $7::varchar[], $8::varchar[], $9::jsonb[], $10::bytea[]
                )
                ON CONFLICT (chain_id, tx_hash, log_index) DO NOTHING
                "#,
            )
            .bind(&chain_ids)
            .bind(&block_numbers)
            .bind(&block_hashes)
            .bind(&tx_hashes)
            .bind(&tx_indices)
            .bind(&log_indices)
            .bind(&contract_addresses)
            .bind(&topic0s)
            .bind(&topics_arr)
            .bind(&data_arr)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Database,
                    format!("Failed to batch insert raw events: {e}"),
                )
            })?;
        }

        // 2. Advance checkpoint atomically in the same transaction
        sqlx::query(
            r#"
            INSERT INTO logrix_checkpoints (
                chain_id, pipeline_id, last_indexed_block, last_block_hash, is_finalized, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, NOW())
            ON CONFLICT (chain_id, pipeline_id) DO UPDATE SET
                last_indexed_block = EXCLUDED.last_indexed_block,
                last_block_hash = EXCLUDED.last_block_hash,
                is_finalized = EXCLUDED.is_finalized,
                updated_at = NOW()
            "#,
        )
        .bind(checkpoint.chain_id.as_u64() as i64)
        .bind(self.pipeline_id())
        .bind(checkpoint.last_indexed_block as i64)
        .bind(format!("{:#x}", checkpoint.last_indexed_hash))
        .bind(checkpoint.is_finalized)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to update checkpoint: {e}"),
            )
        })?;

        tx.commit().await.map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to commit events and checkpoint transaction: {e}"),
            )
        })?;

        debug!(
            chain_id = checkpoint.chain_id.as_u64(),
            block = checkpoint.last_indexed_block,
            events_count = events.len(),
            "Atomically committed events and updated checkpoint"
        );
        Ok(())
    }

    async fn get_checkpoint(&self, chain_id: ChainId) -> LogrixResult<Option<Checkpoint>> {
        let row = sqlx::query(
            r#"
            SELECT last_indexed_block, last_block_hash, is_finalized
            FROM logrix_checkpoints
            WHERE chain_id = $1 AND pipeline_id = $2
            "#,
        )
        .bind(chain_id.as_u64() as i64)
        .bind(self.pipeline_id())
        .fetch_optional(self.pool())
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to fetch checkpoint: {e}"),
            )
        })?;

        if let Some(row) = row {
            let last_block: i64 = row.get("last_indexed_block");
            let hash_str: String = row.get("last_block_hash");
            let is_finalized: bool = row.get("is_finalized");

            let hash = hash_str.parse().unwrap_or_default();
            Ok(Some(Checkpoint::new(
                chain_id,
                last_block as u64,
                hash,
                is_finalized,
            )))
        } else {
            Ok(None)
        }
    }

    async fn rollback_to_block(&self, chain_id: ChainId, to_block: u64) -> LogrixResult<()> {
        self.rollback_internal(chain_id, to_block).await
    }
}
