use crate::buffer::RollingBlockBuffer;
use alloy_primitives::B256;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use logrix_core::ports::ChainPort;

/// Walk back block headers across buffer (and fallback to RPC) to locate the fork point.
/// Bounded by `max_depth` to prevent unbounded network walking and RPC resource drain.
pub async fn find_common_ancestor(
    chain_client: &dyn ChainPort,
    buf: &RollingBlockBuffer,
    start_block: u64,
    mut current_parent: B256,
    max_depth: u64,
) -> LogrixResult<(u64, B256)> {
    let mut curr_num = start_block.saturating_sub(1);
    let mut walked_depth = 0u64;

    while curr_num > 0 {
        if walked_depth >= max_depth {
            return Err(LogrixError::new(
                ErrorClass::Integrity,
                ErrorSource::Reconciler,
                format!(
                    "Reorg ancestor search exceeded max depth limit ({max_depth} blocks) at height {curr_num}"
                ),
            ));
        }

        if let Some(buf_ref) = buf.get_by_number(curr_num) {
            if buf_ref.hash == current_parent {
                return Ok((curr_num, current_parent));
            }
        }

        // Fallback: Query remote node for parent hash of curr_num
        match chain_client.fetch_block_envelope(curr_num, &[]).await? {
            Some(remote_block) => {
                if let Some(buf_ref) = buf.get_by_number(curr_num) {
                    if buf_ref.hash == remote_block.block_hash {
                        return Ok((curr_num, remote_block.block_hash));
                    }
                }
                current_parent = remote_block.parent_hash;
                curr_num = curr_num.saturating_sub(1);
                walked_depth += 1;
            }
            None => {
                return Err(LogrixError::new(
                    ErrorClass::Integrity,
                    ErrorSource::Reconciler,
                    format!(
                        "Failed to retrieve ancestor block header {curr_num} during reorg walk"
                    ),
                ));
            }
        }
    }

    Ok((0, B256::ZERO))
}
