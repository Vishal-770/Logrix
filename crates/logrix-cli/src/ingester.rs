use alloy_primitives::B256;
use logrix_core::{
    domain::{BlockRangeJob, ChainId, LiveBlockJob, QueueMessage, QueueType},
    ports::{ChainPort, LeaderElectionPort, QueuePort, StorePort},
};
use logrix_store_postgres::PostgresStore;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

pub async fn run_ingester(
    chain_id: ChainId,
    gateway: Arc<logrix_rpc_gateway::RpcGateway>,
    queue: Arc<dyn QueuePort>,
    database_url: &str,
    start_block: Option<u64>,
    leader_elector: Arc<dyn LeaderElectionPort>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        chain = chain_id.as_u64(),
        "Starting Logrix Ingester with Cost-Aware RPC Gateway"
    );

    let store = PostgresStore::connect(database_url, "default").await?;

    let mut current_block = match store.get_checkpoint(chain_id).await? {
        Some(cp) => {
            info!(
                checkpoint = cp.last_indexed_block,
                "Resuming from database checkpoint"
            );
            cp.last_indexed_block + 1
        }
        None => {
            if let Some(block) = start_block {
                info!(block, "Starting from configured start block");
                block
            } else {
                let latest = gateway.get_latest_block_number().await?;
                let start = latest.saturating_sub(50);
                info!(
                    latest,
                    start, "No checkpoint found. Starting 50 blocks behind chain head"
                );
                start
            }
        }
    };

    let chunk_size = 500u64;

    loop {
        match leader_elector.try_acquire_or_renew().await {
            Ok(true) => {
                debug!("Active leader status confirmed, continuing ingestion loop");
            }
            Ok(false) => {
                info!("Standby instance (non-leader). Waiting to acquire lease...");
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
            Err(e) => {
                warn!(error = %e, "Leader election check error, standing by...");
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
        }

        match gateway.get_latest_block_number().await {
            Ok(latest) => {
                if current_block <= latest {
                    let to_block = (current_block + chunk_size - 1).min(latest);

                    if current_block == to_block && to_block == latest {
                        let job = LiveBlockJob::new(chain_id, to_block, B256::ZERO, B256::ZERO);
                        let msg = QueueMessage::LiveBlock(job);
                        queue.publish(QueueType::Live, &msg).await?;
                    } else {
                        let job = BlockRangeJob::new(
                            chain_id,
                            current_block,
                            to_block,
                            format!("{chain_id}:{current_block}-{to_block}"),
                        );
                        let msg = QueueMessage::BackfillRange(job);
                        queue.publish(QueueType::Backfill, &msg).await?;
                    }

                    current_block = to_block + 1;
                } else {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to query latest block number from RPC Gateway");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}
