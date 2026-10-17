use alloy_primitives::Address;
use logrix_core::{
    domain::{BlockRangeJob, ChainId, QueueMessage, QueueType},
    ports::QueuePort,
};
use logrix_rpc_gateway::CuBudgetTracker;
use std::sync::Arc;
use tracing::info;

pub async fn run_backfill(
    chain_id: ChainId,
    target_contract: Address,
    from_block: u64,
    to_block: u64,
    chunk_size: u64,
    dry_run: bool,
    queue: Arc<dyn QueuePort>,
) -> Result<(), Box<dyn std::error::Error>> {
    if from_block > to_block {
        return Err(format!(
            "Invalid block range: from_block ({from_block}) cannot be greater than to_block ({to_block})"
        )
        .into());
    }

    let effective_chunk = chunk_size.max(1);
    if dry_run {
        let (chunks, cu, usd) =
            CuBudgetTracker::estimate_range_cost(from_block, to_block, effective_chunk);
        let total_blocks = to_block - from_block + 1;
        let est_seconds = chunks * 2;

        println!("\n=======================================================");
        println!("           LOGRIX HISTORICAL BACKFILL DRY-RUN          ");
        println!("=======================================================");
        println!("  Target Chain ID       : {}", chain_id);
        println!("  Target Contract       : {}", target_contract);
        println!("  Block Range           : {} -> {}", from_block, to_block);
        println!("  Total Blocks          : {}", total_blocks);
        println!("  Chunk Size            : {} blocks", effective_chunk);
        println!("  Estimated Chunks      : {}", chunks);
        println!("  Estimated CUs         : {} Compute Units", cu);
        println!("  Estimated Cloud Cost  : ${:.4} USD (@ $1/1M CU)", usd);
        println!(
            "  Estimated Duration    : ~{} seconds ({:.1} minutes)",
            est_seconds,
            est_seconds as f64 / 60.0
        );
        println!("=======================================================\n");
    } else {
        info!(
            from_block,
            to_block,
            chunk_size = effective_chunk,
            chain = chain_id.as_u64(),
            "Dispatching sliced backfill ranges to queue..."
        );
        let mut curr = from_block;
        let mut dispatched_count = 0;
        while curr <= to_block {
            let next = (curr + effective_chunk - 1).min(to_block);
            let job =
                BlockRangeJob::try_new(chain_id, curr, next, format!("{chain_id}:{curr}-{next}"))?;
            queue
                .publish(QueueType::Backfill, &QueueMessage::BackfillRange(job))
                .await?;
            dispatched_count += 1;
            curr = next + 1;
        }
        info!(
            chunks = dispatched_count,
            "All backfill range jobs dispatched successfully."
        );
    }
    Ok(())
}
