use crate::indexer::index_logs_batch;
use crate::processor::ProcessorContext;
use logrix_core::domain::{BlockRangeJob, MessageHandle, QueueMessage, QueueType};
use logrix_core::ports::ChainPort;
use std::time::Duration;
use tracing::warn;

pub async fn handle_backfill_range(
    ctx: &ProcessorContext<'_>,
    job: &BlockRangeJob,
    handle: &MessageHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    match ctx
        .gateway
        .fetch_logs(job.from_block, job.to_block, &ctx.target_addresses())
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
                tokio::time::sleep(Duration::from_millis(500)).await;
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
                let first_ok = ctx
                    .queue
                    .publish(QueueType::Backfill, &QueueMessage::BackfillRange(first))
                    .await
                    .is_ok();
                let second_ok = match second_opt {
                    Some(second) => ctx
                        .queue
                        .publish(QueueType::Backfill, &QueueMessage::BackfillRange(second))
                        .await
                        .is_ok(),
                    None => true,
                };
                if first_ok && second_ok {
                    let _ = ctx.queue.ack(handle).await;
                } else {
                    let _ = ctx.queue.nack(handle, true).await;
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            } else {
                let _ = ctx.queue.nack(handle, true).await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
    Ok(())
}
