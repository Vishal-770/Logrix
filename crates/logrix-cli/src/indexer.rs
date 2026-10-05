use alloy_primitives::B256;
use logrix_blob_s3::{compress_zstd, S3BlobStore};
use logrix_core::{
    domain::{BlockEnvelope, Checkpoint, EventLog},
    error::LogrixResult,
    ports::{BlobPort, StorePort},
};
use logrix_handlers::UserLogicEngine;
use logrix_store_postgres::{EntityInsert, PostgresStore};
use tracing::{debug, error};

pub async fn index_envelope(
    store: &PostgresStore,
    blob_store: Option<&S3BlobStore>,
    chain_id_u64: u64,
    envelope: &BlockEnvelope,
    engine_opt: Option<&UserLogicEngine>,
) -> LogrixResult<()> {
    let checkpoint = Checkpoint::new(
        envelope.chain_id,
        envelope.block_number,
        envelope.block_hash,
        true,
    );
    store
        .write_events_and_checkpoint(&envelope.logs, &checkpoint)
        .await?;

    if let Some(engine) = engine_opt {
        process_entities(store, engine, chain_id_u64, &envelope.logs).await;
    }

    if let Some(blob) = blob_store {
        if let Ok(raw_json) = serde_json::to_vec(envelope) {
            if let Ok(compressed) = compress_zstd(&raw_json, 3) {
                let s3_key = format!(
                    "chain_{}/blocks/{}.json.zst",
                    chain_id_u64, envelope.block_number
                );
                let _ = blob.put(&s3_key, &compressed).await;
            }
        }
    }

    Ok(())
}

pub async fn index_logs_batch(
    store: &PostgresStore,
    chain_id: logrix_core::domain::ChainId,
    to_block: u64,
    logs: &[EventLog],
    engine_opt: Option<&UserLogicEngine>,
) -> LogrixResult<()> {
    let checkpoint = Checkpoint::new(chain_id, to_block, B256::ZERO, true);
    store.write_events_and_checkpoint(logs, &checkpoint).await?;

    if let Some(engine) = engine_opt {
        process_entities(store, engine, chain_id.as_u64(), logs).await;
    }

    Ok(())
}

async fn process_entities(
    store: &PostgresStore,
    engine: &UserLogicEngine,
    chain_id: u64,
    logs: &[EventLog],
) {
    for log in logs {
        if let Ok(staging) = engine.process_log(log).await {
            let (mutations, removed_keys, emitted) = engine.commit_staging(staging).await;

            // Persist emitted entity upserts
            if !emitted.is_empty() {
                let inserts: Vec<EntityInsert> = emitted
                    .into_iter()
                    .map(|e| {
                        let entity_id = e
                            .payload
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("default")
                            .to_string();
                        EntityInsert {
                            entity_type: e.entity_type,
                            entity_id,
                            data: e.payload,
                        }
                    })
                    .collect();
                if let Err(e) = store
                    .save_entities_batch(chain_id, log.block_number, &inserts)
                    .await
                {
                    error!(error = %e, "Failed to save dynamic schema entities");
                }
            }

            // Soft-delete entities staged for removal (Entity.remove())
            if !removed_keys.is_empty() {
                for key in &removed_keys {
                    // Key format: "EntityType:id"
                    if let Some((entity_type, entity_id)) = key.split_once(':') {
                        if let Err(e) = store.delete_entity(chain_id, entity_type, entity_id).await
                        {
                            error!(error = %e, key, "Failed to soft-delete entity");
                        }
                    }
                }
                debug!(removed = removed_keys.len(), "Processed entity removals");
            }

            if !mutations.is_empty() {
                debug!(
                    mutations = mutations.len(),
                    "Committed user logic state mutations"
                );
            }
        }
    }
}
