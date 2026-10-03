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
    dry_run: bool,
    queue: Arc<dyn QueuePort>,
) -> Result<(), Box<dyn std::error::Error>> {
    if dry_run {
        let chunk_size = 500;
        let (chunks, cu, usd) =
            CuBudgetTracker::estimate_range_cost(from_block, to_block, chunk_size);
        let total_blocks = if to_block >= from_block {
            to_block - from_block + 1
        } else {
            0
        };
        let est_seconds = chunks * 2;

        println!("\n=======================================================");
        println!("           LOGRIX HISTORICAL BACKFILL DRY-RUN          ");
        println!("=======================================================");
        println!("  Target Chain ID       : {}", chain_id);
        println!("  Target Contract       : {}", target_contract);
        println!("  Block Range           : {} -> {}", from_block, to_block);
        println!("  Total Blocks          : {}", total_blocks);
        println!("  Average Chunk Size    : {} blocks", chunk_size);
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
            chain = chain_id.as_u64(),
            "Dispatching backfill range to queue..."
        );
        let job = BlockRangeJob::new(
            chain_id,
            from_block,
            to_block,
            format!("{chain_id}:{from_block}-{to_block}"),
        );
        queue
            .publish(QueueType::Backfill, &QueueMessage::BackfillRange(job))
            .await?;
        info!("Backfill range dispatched successfully.");
    }
    Ok(())
}
