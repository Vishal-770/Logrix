use crate::ingester_ops::{is_backpressure_active, publish_with_retry};
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
    max_queue_depth: u64,
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
                tokio::select! {
                    _ = crate::shutdown::wait_for_shutdown_signal() => {
                        info!("Shutdown received on standby instance; exiting cleanly");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {
                        continue;
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Leader election check error, standing by...");
                tokio::select! {
                    _ = crate::shutdown::wait_for_shutdown_signal() => {
                        info!("Shutdown received during leader election backoff; exiting cleanly");
                        break;
                    }
                    _ = tokio::time::sleep(Duration::from_secs(3)) => {
                        continue;
                    }
                }
            }
        }

        if is_backpressure_active(queue.as_ref(), max_queue_depth).await {
            tokio::select! {
                _ = crate::shutdown::wait_for_shutdown_signal() => {
                    info!("Shutdown received during backpressure throttle; exiting cleanly");
                    break;
                }
                _ = tokio::time::sleep(Duration::from_secs(2)) => {
                    continue;
                }
            }
        }

        let latest = tokio::select! {
            _ = crate::shutdown::wait_for_shutdown_signal() => {
                info!("Shutdown signal received in Ingester loop; exiting cleanly");
                break;
            }
            res = gateway.get_latest_block_number() => match res {
                Ok(num) => num,
                Err(e) => {
                    warn!(error = %e, "Failed to query latest block number from RPC Gateway");
                    tokio::select! {
                        _ = crate::shutdown::wait_for_shutdown_signal() => {
                            info!("Shutdown received during RPC backoff; exiting cleanly");
                            break;
                        }
                        _ = tokio::time::sleep(Duration::from_secs(2)) => {
                            continue;
                        }
                    }
                }
            }
        };

        if current_block <= latest {
            let to_block = (current_block + chunk_size - 1).min(latest);

            let published = if current_block == to_block && to_block == latest {
                let job = LiveBlockJob::new(chain_id, to_block, B256::ZERO, B256::ZERO);
                let msg = QueueMessage::LiveBlock(job);
                publish_with_retry(queue.as_ref(), QueueType::Live, &msg).await
            } else {
                let job = BlockRangeJob::new(
                    chain_id,
                    current_block,
                    to_block,
                    format!("{chain_id}:{current_block}-{to_block}"),
                );
                let msg = QueueMessage::BackfillRange(job);
                publish_with_retry(queue.as_ref(), QueueType::Backfill, &msg).await
            };

            if !published {
                info!("Ingester shutting down without advancing block checkpoint");
                break;
            }

            current_block = to_block + 1;
        } else {
            tokio::select! {
                _ = crate::shutdown::wait_for_shutdown_signal() => {
                    info!("Shutdown received while at chain head; exiting cleanly");
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(1500)) => {}
            }
        }
    }

    info!("Ingester loop finished draining");
    Ok(())
}
