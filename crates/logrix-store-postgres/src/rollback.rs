use crate::store::PostgresStore;
use logrix_core::{
    domain::ChainId,
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};
use tracing::info;

impl PostgresStore {
    pub(crate) async fn rollback_internal(&self, chain_id: ChainId, to_block: u64) -> LogrixResult<()> {
        let mut tx = self.pool().begin().await.map_err(|e| {
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
            "UPDATE logrix_entities SET is_reverted = TRUE, reverted_at = NOW() WHERE chain_id = $1 AND block_number > $2 AND is_reverted = FALSE"
        )
        .bind(chain_id_val)
        .bind(to_block_val)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Database,
                format!("Failed to soft-delete rolled-back entities: {e}"),
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
        .bind(self.pipeline_id())
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
