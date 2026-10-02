use async_graphql::{
    Context, EmptyMutation, EmptySubscription, InputObject, Object, Schema, SimpleObject,
};
use logrix_core::{domain::ChainId, ports::StorePort};
use logrix_store_postgres::{PostgresStore, TransferFilter};
use std::sync::Arc;

#[derive(SimpleObject, Clone)]
pub struct Transfer {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
    pub contract_address: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub timestamp: String,
}

#[derive(InputObject, Default, Clone)]
pub struct TransferFilterInput {
    pub contract_address: Option<String>,
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(SimpleObject, Clone)]
pub struct CheckpointStatus {
    pub chain_id: u64,
    pub last_indexed_block: u64,
    pub last_block_hash: String,
    pub is_finalized: bool,
}

#[derive(SimpleObject, Clone)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
}

pub struct QueryRoot;

#[Object]
impl QueryRoot {
    /// Retrieve indexed token transfers matching optional filters.
    async fn transfers(
        &self,
        ctx: &Context<'_>,
        chain_id: u64,
        filter: Option<TransferFilterInput>,
    ) -> async_graphql::Result<Vec<Transfer>> {
        let store = ctx.data::<Arc<PostgresStore>>()?;
        let f = filter.unwrap_or_default();
        let db_filter = TransferFilter {
            contract_address: f.contract_address,
            from_address: f.from_address,
            to_address: f.to_address,
            limit: f.limit,
            offset: f.offset,
        };

        let raw_transfers = store
            .get_transfers(chain_id, &db_filter)
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        let transfers = raw_transfers
            .into_iter()
            .map(|t| Transfer {
                chain_id: t.chain_id,
                block_number: t.block_number,
                block_hash: t.block_hash,
                tx_hash: t.tx_hash,
                log_index: t.log_index,
                contract_address: t.contract_address,
                from_address: t.from_address,
                to_address: t.to_address,
                amount: t.amount,
                timestamp: t.timestamp.to_rfc3339(),
            })
            .collect();

        Ok(transfers)
    }

    /// Retrieve the current indexing checkpoint for a chain.
    async fn checkpoint(
        &self,
        ctx: &Context<'_>,
        chain_id: u64,
    ) -> async_graphql::Result<Option<CheckpointStatus>> {
        let store = ctx.data::<Arc<PostgresStore>>()?;
        let cp_res = store
            .get_checkpoint(ChainId::new(chain_id))
            .await
            .map_err(|e| async_graphql::Error::new(e.to_string()))?;

        Ok(cp_res.map(|cp| CheckpointStatus {
            chain_id: cp.chain_id.as_u64(),
            last_indexed_block: cp.last_indexed_block,
            last_block_hash: format!("{:#x}", cp.last_indexed_hash),
            is_finalized: cp.is_finalized,
        }))
    }

    /// Check health and liveness of the Logrix API service.
    async fn health(&self) -> HealthStatus {
        HealthStatus {
            status: "UP".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

pub type LogrixSchema = Schema<QueryRoot, EmptyMutation, EmptySubscription>;

pub fn build_schema(store: Arc<PostgresStore>) -> LogrixSchema {
    Schema::build(QueryRoot, EmptyMutation, EmptySubscription)
        .data(store)
        .finish()
}
