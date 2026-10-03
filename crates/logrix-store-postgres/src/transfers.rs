use crate::model::{TokenTransfer, TransferFilter};
use crate::store::PostgresStore;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use sqlx::Row;
use tracing::debug;

impl PostgresStore {
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
        .execute(self.pool())
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

        let rows = sql.fetch_all(self.pool()).await.map_err(|e| {
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
