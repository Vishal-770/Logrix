use crate::indexer::index_envelope;
use crate::processor::ProcessorContext;
use logrix_core::domain::{BlockRangeJob, MessageHandle, QueueMessage, QueueType};
use logrix_core::ports::ChainPort;
use logrix_reconciler::ContinuityStatus;
use std::time::Duration;
use tracing::warn;

pub async fn handle_live_block(
    ctx: &ProcessorContext<'_>,
    block_num: u64,
    handle: &MessageHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let addrs = ctx.target_addresses();
    let Ok(Some(env)) = ctx.gateway.fetch_block_envelope(block_num, &addrs).await else {
        let _ = ctx.queue.nack(handle, true).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        return Ok(());
    };

    match ctx.detector.check_envelope(&env).await {
        Ok(ContinuityStatus::Continuous) => {
            match index_envelope(
                ctx.store,
                ctx.blob_store,
                ctx.config.chain_id.as_u64(),
                &env,
                ctx.engine,
            )
            .await
            {
                Ok(()) => {
                    let _ = ctx.queue.ack(handle).await;
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        block = env.block_number,
                        logs_count = env.logs.len(),
                        "Failed to index block envelope"
                    );
                    let _ = ctx.queue.nack(handle, true).await;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
        Ok(ContinuityStatus::ReorgDetected {
            fork_block,
            fork_hash,
            reorg_depth,
        }) => {
            warn!(
                fork_block,
                fork_hash = %fork_hash,
                reorg_depth,
                "Reorg detected! Executing atomic rollback"
            );
            if ctx
                .handler
                .execute_rollback(fork_block, fork_hash, reorg_depth)
                .await
                .is_ok()
            {
                if fork_block == env.block_number - 1 {
                    ctx.detector.buffer().write().await.push(env.block_ref());
                    if index_envelope(
                        ctx.store,
                        ctx.blob_store,
                        ctx.config.chain_id.as_u64(),
                        &env,
                        ctx.engine,
                    )
                    .await
                    .is_ok()
                    {
                        let _ = ctx.queue.ack(handle).await;
                    } else {
                        let _ = ctx.queue.nack(handle, true).await;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                } else {
                    let gap = BlockRangeJob::try_new(
                        ctx.config.chain_id,
                        fork_block + 1,
                        env.block_number,
                        format!(
                            "{}:{}-{}",
                            ctx.config.chain_id,
                            fork_block + 1,
                            env.block_number
                        ),
                    )?;
                    if ctx
                        .queue
                        .publish(QueueType::Backfill, &QueueMessage::BackfillRange(gap))
                        .await
                        .is_ok()
                    {
                        let _ = ctx.queue.ack(handle).await;
                    } else {
                        let _ = ctx.queue.nack(handle, true).await;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            } else {
                let _ = ctx.queue.nack(handle, true).await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
        Ok(ContinuityStatus::GapDetected {
            from_block,
            to_block,
        }) => {
            let gap = BlockRangeJob::try_new(
                ctx.config.chain_id,
                from_block,
                to_block,
                format!("{}:{from_block}-{to_block}", ctx.config.chain_id),
            )?;
            let _ = ctx
                .queue
                .publish(QueueType::Backfill, &QueueMessage::BackfillRange(gap))
                .await;
            let _ = ctx.queue.nack(handle, true).await;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Err(_) => {
            let _ = ctx.queue.nack(handle, true).await;
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    Ok(())
}
