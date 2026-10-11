use alloy_primitives::B256;
use logrix_blob_s3::{compress_zstd, S3BlobStore};
use logrix_chain_evm::decode_erc20_transfer;
use logrix_core::{
    domain::{BlockEnvelope, Checkpoint, EventLog},
    error::LogrixResult,
    ports::{BlobPort, StorePort},
};
use logrix_handlers::UserLogicEngine;
use logrix_store_postgres::{EntityInsert, PostgresStore, TokenTransfer};
use tracing::debug;

pub async fn index_envelope(
    store: &PostgresStore,
    blob_store: Option<&S3BlobStore>,
    chain_id_u64: u64,
    envelope: &BlockEnvelope,
    engine_opt: Option<&UserLogicEngine>,
) -> LogrixResult<()> {
    save_token_transfers(store, chain_id_u64, &envelope.logs).await?;

    if let Some(engine) = engine_opt {
        process_entities(store, engine, chain_id_u64, &envelope.logs).await?;
    }

    let checkpoint = Checkpoint::new(
        envelope.chain_id,
        envelope.block_number,
        envelope.block_hash,
        true,
    );
    store
        .write_events_and_checkpoint(&envelope.logs, &checkpoint)
        .await?;

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
    save_token_transfers(store, chain_id.as_u64(), logs).await?;

    if let Some(engine) = engine_opt {
        process_entities(store, engine, chain_id.as_u64(), logs).await?;
    }

    let checkpoint = Checkpoint::new(chain_id, to_block, B256::ZERO, true);
    store.write_events_and_checkpoint(logs, &checkpoint).await?;

    Ok(())
}

async fn save_token_transfers(
    store: &PostgresStore,
    chain_id: u64,
    logs: &[EventLog],
) -> LogrixResult<()> {
    let mut transfers = Vec::new();
    for log in logs {
        if let Some(d) = decode_erc20_transfer(chain_id, log) {
            transfers.push(TokenTransfer {
                chain_id: d.chain_id,
                block_number: d.block_number,
                block_hash: d.block_hash,
                tx_hash: d.tx_hash,
                log_index: d.log_index,
                contract_address: d.contract_address,
                from_address: d.from_address,
                to_address: d.to_address,
                amount: d.amount,
                timestamp: d.timestamp,
            });
        }
    }
    if !transfers.is_empty() {
        store.save_transfers_batch(&transfers).await?;
    }
    Ok(())
}

async fn process_entities(
    store: &PostgresStore,
    engine: &UserLogicEngine,
    chain_id: u64,
    logs: &[EventLog],
) -> LogrixResult<()> {
    for log in logs {
        let staging = engine.process_log(log).await?;
        let (mutations, removed_keys, emitted) = engine.commit_staging(staging).await;

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
            store
                .save_entities_batch(chain_id, log.block_number, &inserts)
                .await?;
        }

        if !removed_keys.is_empty() {
            for key in &removed_keys {
                if let Some((entity_type, entity_id)) = key.split_once(':') {
                    store
                        .delete_entity(chain_id, entity_type, entity_id)
                        .await?;
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
    Ok(())
}
