use crate::model::{TokenTransfer, TransferFilter};
use async_trait::async_trait;
use logrix_core::{
    domain::{ChainId, Checkpoint, EventLog},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::StorePort,
};
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::time::Duration;
use tracing::{debug, info};

/// Production PostgreSQL implementation of `StorePort`.
///
/// Features high-throughput vectorized UNNEST batch inserts, atomic transaction
/// boundaries across events and checkpoints, and automatic reorg rollbacks.
#[derive(Debug, Clone)]
pub struct PostgresStore {
    pool: PgPool,
    pipeline_id: String,
}

impl PostgresStore {
    /// Create a new PostgresStore with production-tuned pool settings.
    pub async fn connect(database_url: &str, pipeline_id: impl Into<String>) -> LogrixResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(30)
            .min_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .idle_timeout(Duration::from_secs(600))
            .connect(database_url)
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Database,
                    format!("Failed to connect to PostgreSQL: {e}"),
                )
            })?;

        info!("Connected to PostgreSQL connection pool successfully");
        Ok(Self {
            pool,
            pipeline_id: pipeline_id.into(),
        })
    }

    /// Construct from an existing sqlx PgPool.
    pub fn from_pool(pool: PgPool, pipeline_id: impl Into<String>) -> Self {
        Self {
            pool,
            pipeline_id: pipeline_id.into(),
        }
    }

    /// Run embedded database migrations automatically.
    pub async fn migrate(&self) -> LogrixResult<()> {
        info!("Running database migrations...");
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Permanent,
                    ErrorSource::Database,
                    format!("Database migration failed: {e}"),
                )
            })?;
        info!("Database migrations applied successfully");
        Ok(())
    }

    /// Return reference to internal connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Save a batch of decoded ERC-20 token transfers idempotently.
    pub async fn save_transfers_batch(&self, transfers: &[TokenTransfer]) -> LogrixResult<()> {
        if transfers.is_empty() {
            return Ok(());
        }

        let mut chain_ids = Vec::with_capacity(transfers.len());
        let mut block_numbers = Vec::with_capacity(transfers.len());
        let mut block_hashes = Vec::with_capacity(transfers.len());
        let mut tx_hashes = Vec::with_capacity(transfers.len());
        let mut log_indices = Vec::with_capacity(transfers.len());
        let mut contract_addresses = Vec::with_capacity(transfers.len());
        let mut from_addresses = Vec::with_capacity(transfers.len());
        let mut to_addresses = Vec::with_capacity(transfers.len());
        let mut amounts = Vec::with_capacity(transfers.len());
        let mut timestamps = Vec::with_capacity(transfers.len());

        for t in transfers {
            chain_ids.push(t.chain_id as i64);
            block_numbers.push(t.block_number as i64);
            block_hashes.push(t.block_hash.clone());
            tx_hashes.push(t.tx_hash.clone());
            log_indices.push(t.log_index as i64);
            contract_addresses.push(t.contract_address.clone());
            from_addresses.push(t.from_address.clone());
            to_addresses.push(t.to_address.clone());
            amounts.push(t.amount.clone());
            timestamps.push(t.timestamp);
        }

        sqlx::query(
            r#"
            INSERT INTO token_transfers (
                chain_id, block_number, block_hash, tx_hash, log_index,
                contract_address, from_address, to_address, amount, timestamp
            )
            SELECT * FROM UNNEST(
                $1::bigint[], $2::bigint[], $3::varchar[], $4::varchar[], $5::bigint[],
                $6::varchar[], $7::varchar[], $8::varchar[], $9::numeric[], $10::timestamptz[]
            )
            ON CONFLICT (chain_id, tx_hash, log_index) DO NOTHING
            "#,
        )
        .bind(&chain_ids)
        .bind(&block_numbers)
        .bind(&block_hashes)
        .bind(&tx_hashes)
        .bind(&log_indices)
        .bind(&contract_addresses)
        .bind(&from_addresses)
        .bind(&to_addresses)
        .bind(&amounts)
        .bind(&timestamps)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to batch insert token transfers: {e}"),
            )
        })?;

        debug!(count = transfers.len(), "Batch inserted token transfers");
        Ok(())
    }

    /// Query token transfers matching optional filter parameters.
    pub async fn get_transfers(
        &self,
        chain_id: u64,
        filter: &TransferFilter,
    ) -> LogrixResult<Vec<TokenTransfer>> {
        let limit = filter.limit.unwrap_or(50).clamp(1, 1000);
        let offset = filter.offset.unwrap_or(0).max(0);

        let mut query = String::from(
            r#"
            SELECT 
                chain_id, block_number, block_hash, tx_hash, log_index,
                contract_address, from_address, to_address, amount::text, timestamp
            FROM token_transfers
            WHERE chain_id = $1 AND is_reverted = FALSE
            "#,
        );

        let mut param_idx = 2;
        if filter.contract_address.is_some() {
            query.push_str(&format!(
                " AND LOWER(contract_address) = LOWER(${param_idx})"
            ));
            param_idx += 1;
        }
        if filter.from_address.is_some() {
            query.push_str(&format!(" AND LOWER(from_address) = LOWER(${param_idx})"));
            param_idx += 1;
        }
        if filter.to_address.is_some() {
            query.push_str(&format!(" AND LOWER(to_address) = LOWER(${param_idx})"));
            param_idx += 1;
        }

        query.push_str(&format!(
            " ORDER BY block_number DESC, log_index DESC LIMIT ${param_idx} OFFSET ${}",
            param_idx + 1
        ));

        let mut sql = sqlx::query(&query).bind(chain_id as i64);

        if let Some(ref contract) = filter.contract_address {
            sql = sql.bind(contract);
        }
        if let Some(ref from) = filter.from_address {
            sql = sql.bind(from);
        }
        if let Some(ref to) = filter.to_address {
            sql = sql.bind(to);
        }
        sql = sql.bind(limit).bind(offset);

        let rows = sql.fetch_all(&self.pool).await.map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to query token transfers: {e}"),
            )
        })?;

        let transfers = rows
            .into_iter()
            .map(|row| TokenTransfer {
                chain_id: row.get::<i64, _>("chain_id") as u64,
                block_number: row.get::<i64, _>("block_number") as u64,
                block_hash: row.get("block_hash"),
                tx_hash: row.get("tx_hash"),
                log_index: row.get::<i64, _>("log_index") as u64,
                contract_address: row.get("contract_address"),
                from_address: row.get("from_address"),
                to_address: row.get("to_address"),
                amount: row.get("amount"),
                timestamp: row.get("timestamp"),
            })
            .collect();

        Ok(transfers)
    }
}

#[async_trait]
impl StorePort for PostgresStore {
    async fn write_events_and_checkpoint(
        &self,
        events: &[EventLog],
        checkpoint: &Checkpoint,
    ) -> LogrixResult<()> {
        let mut tx = self.pool.begin().await.map_err(|e| {
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
        .bind(&self.pipeline_id)
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
        .bind(&self.pipeline_id)
        .fetch_optional(&self.pool)
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
        let mut tx = self.pool.begin().await.map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to start rollback transaction: {e}"),
            )
        })?;

        let chain_id_val = chain_id.as_u64() as i64;
        let to_block_val = to_block as i64;

        sqlx::query(
            "UPDATE logrix_events SET is_reverted = TRUE, reverted_at = NOW() WHERE chain_id = $1 AND block_number > $2 AND is_reverted = FALSE"
        )
        .bind(chain_id_val)
        .bind(to_block_val)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to soft-delete rolled-back events: {e}"),
            )
        })?;

        sqlx::query(
            "UPDATE token_transfers SET is_reverted = TRUE, reverted_at = NOW() WHERE chain_id = $1 AND block_number > $2 AND is_reverted = FALSE"
        )
        .bind(chain_id_val)
        .bind(to_block_val)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to soft-delete rolled-back transfers: {e}"),
            )
        })?;

        sqlx::query(
            r#"
            UPDATE logrix_checkpoints
            SET last_indexed_block = $2, is_finalized = FALSE, updated_at = NOW()
            WHERE chain_id = $1 AND pipeline_id = $3
            "#,
        )
        .bind(chain_id_val)
        .bind(to_block_val)
        .bind(&self.pipeline_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to update checkpoint on rollback: {e}"),
            )
        })?;

        tx.commit().await.map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to commit rollback transaction: {e}"),
            )
        })?;

        info!(
            chain_id = chain_id.as_u64(),
            to_block, "Successfully rolled back blockchain state"
        );
        Ok(())
    }
}
