use crate::indexer::{index_envelope, index_logs_batch};
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
    let addrs = target_addresses(ctx);
    let Ok(Some(env)) = ctx.gateway.fetch_block_envelope(block_num, &addrs).await else {
        let _ = ctx.queue.nack(handle, true).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        return Ok(());
    };

    match ctx.detector.check_envelope(&env).await {
        Ok(ContinuityStatus::Continuous) => {
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
                    let _ = index_envelope(
                        ctx.store,
                        ctx.blob_store,
                        ctx.config.chain_id.as_u64(),
                        &env,
                        ctx.engine,
                    )
                    .await;
                } else {
                    let gap = BlockRangeJob::new(
                        ctx.config.chain_id,
                        fork_block + 1,
                        env.block_number,
                        format!(
                            "{}:{}-{}",
                            ctx.config.chain_id,
                            fork_block + 1,
                            env.block_number
                        ),
                    );
                    let _ = ctx
                        .queue
                        .publish(QueueType::Backfill, &QueueMessage::BackfillRange(gap))
                        .await;
                }
                let _ = ctx.queue.ack(handle).await;
            } else {
                let _ = ctx.queue.nack(handle, true).await;
            }
        }
        Ok(ContinuityStatus::GapDetected {
            from_block,
            to_block,
        }) => {
            let gap = BlockRangeJob::new(
                ctx.config.chain_id,
                from_block,
                to_block,
                format!("{}:{from_block}-{to_block}", ctx.config.chain_id),
            );
            let _ = ctx
                .queue
                .publish(QueueType::Backfill, &QueueMessage::BackfillRange(gap))
                .await;
            let _ = ctx.queue.nack(handle, true).await;
        }
        Err(_) => {
            let _ = ctx.queue.nack(handle, true).await;
        }
    }
    Ok(())
}

pub async fn handle_backfill_range(
    ctx: &ProcessorContext<'_>,
    job: &BlockRangeJob,
    handle: &MessageHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    match ctx
        .gateway
        .fetch_logs(job.from_block, job.to_block, &target_addresses(ctx))
        .await
    {
        Ok(logs) => {
            if index_logs_batch(
                ctx.store,
                ctx.config.chain_id,
                job.to_block,
                &logs,
                ctx.engine,
            )
            .await
            .is_ok()
            {
                let _ = ctx.queue.ack(handle).await;
            } else {
                let _ = ctx.queue.nack(handle, true).await;
            }
        }
        Err(e) => {
            let err = e.to_string();
            if err.contains("more than")
                || err.contains("limit")
                || err.contains("range")
                || err.contains("exceeded")
                || err.contains("10000 results")
                || err.contains("413")
            {
                let (first, second_opt) = job.split_half();
                warn!(
                    from = job.from_block,
                    to = job.to_block,
                    "RPC range limit exceeded; halving range: {}..={}",
                    first.from_block,
                    first.to_block
                );
                let _ = ctx
                    .queue
                    .publish(QueueType::Backfill, &QueueMessage::BackfillRange(first))
                    .await;
                if let Some(second) = second_opt {
                    let _ = ctx
                        .queue
                        .publish(QueueType::Backfill, &QueueMessage::BackfillRange(second))
                        .await;
                }
                let _ = ctx.queue.ack(handle).await;
            } else {
                let _ = ctx.queue.nack(handle, true).await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
    Ok(())
}

fn target_addresses(ctx: &ProcessorContext<'_>) -> Vec<alloy_primitives::Address> {
    if let Some(engine) = ctx.engine {
        let addrs: Vec<_> = engine
            .manifest()
            .contracts
            .iter()
            .map(|c| c.address)
            .collect();
        if !addrs.is_empty() {
            return addrs;
        }
    }
    vec![ctx.config.target_contract]
}
