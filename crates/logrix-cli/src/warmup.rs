use logrix_handlers::UserLogicEngine;
use logrix_store_postgres::PostgresStore;
use tracing::info;

/// Pre-warm in-memory state store from PostgreSQL dynamic entities.
pub async fn prewarm_state_store(store: &PostgresStore, engine: &UserLogicEngine, chain_id: u64) {
    let query = format!(
        "SELECT entity_type, entity_id, data FROM logrix_entities WHERE chain_id = {} AND is_reverted = FALSE",
        chain_id
    );
    if let Ok(records) = store.query_entities_raw(&query).await {
        info!(
            count = records.len(),
            "Pre-warming in-memory state store from PostgreSQL"
        );
        for rec in records {
            let key = format!("{}:{}", rec.entity_type, rec.entity_id);
            engine.seed_state(key, rec.data.to_string()).await;
        }
    }
}
