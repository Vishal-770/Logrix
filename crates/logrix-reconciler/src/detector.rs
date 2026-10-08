use crate::buffer::RollingBlockBuffer;
use alloy_primitives::B256;
use logrix_core::domain::{BlockEnvelope, ChainId};
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use logrix_core::ports::ChainPort;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Detection outcome after verifying an incoming block envelope against chain continuity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuityStatus {
    /// Block connects continuously to previous block. No reorg.
    Continuous,
    /// Parent hash mismatch detected: chain reorganized! Contains the common ancestor fork point.
    ReorgDetected {
        fork_block: u64,
        fork_hash: B256,
        reorg_depth: u64,
    },
    /// A missing block gap was detected (e.g. current block is tip + 5).
    GapDetected { from_block: u64, to_block: u64 },
}

/// Blockchain reorganization detector coordinating in-memory buffer and RPC ancestor walking.
#[derive(Clone)]
pub struct ReorgDetector {
    chain_id: ChainId,
    buffer: Arc<RwLock<RollingBlockBuffer>>,
    chain_client: Arc<dyn ChainPort>,
}

impl ReorgDetector {
    pub fn new(
        chain_id: ChainId,
        chain_client: Arc<dyn ChainPort>,
        buffer_capacity: usize,
    ) -> Self {
        Self {
            chain_id,
            buffer: Arc::new(RwLock::new(RollingBlockBuffer::new(buffer_capacity))),
            chain_client,
        }
    }

    /// Return the monitored chain id.
    pub fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    /// Access internal ring buffer.
    pub fn buffer(&self) -> &Arc<RwLock<RollingBlockBuffer>> {
        &self.buffer
    }

    /// Verify an incoming block envelope for parent continuity.
    ///
    /// If parent hash matches, appends to buffer and returns `ContinuityStatus::Continuous`.
    /// If parent mismatch or gap, calculates fork point and returns `ReorgDetected` or `GapDetected`.
    pub async fn check_envelope(&self, envelope: &BlockEnvelope) -> LogrixResult<ContinuityStatus> {
        let mut buf = self.buffer.write().await;

        if let Some(tip) = buf.latest() {
            if envelope.block_number == tip.number + 1 {
                if envelope.parent_hash == tip.hash {
                    // Perfect continuous block
                    buf.push(envelope.block_ref());
                    Ok(ContinuityStatus::Continuous)
                } else {
                    // REORG! Same height + 1, but parent hash doesn't match tip
                    warn!(
                        block = envelope.block_number,
                        expected_parent = %tip.hash,
                        actual_parent = %envelope.parent_hash,
                        "Blockchain reorg detected: parent hash mismatch"
                    );

                    let tip_number = tip.number;
                    // Take snapshot and release write lock before making remote network RPC calls
                    let buf_snapshot = buf.clone();
                    drop(buf);

                    let (fork_block, fork_hash) = self
                        .find_common_ancestor(
                            &buf_snapshot,
                            envelope.block_number,
                            envelope.parent_hash,
                        )
                        .await?;
                    let reorg_depth = tip_number.saturating_sub(fork_block);

                    info!(
                        fork_block,
                        fork_hash = %fork_hash,
                        reorg_depth,
                        "Identified common ancestor fork point"
                    );

                    Ok(ContinuityStatus::ReorgDetected {
                        fork_block,
                        fork_hash,
                        reorg_depth,
                    })
                }
            } else if envelope.block_number > tip.number + 1 {
                // Gap detected: skipped one or more blocks
                let from = tip.number + 1;
                let to = envelope.block_number - 1;
                warn!(from, to, "Gap detected in block stream");
                Ok(ContinuityStatus::GapDetected {
                    from_block: from,
                    to_block: to,
                })
            } else {
                // Older or duplicate block number
                if let Some(existing) = buf.get_by_number(envelope.block_number) {
                    if existing.hash != envelope.block_hash {
                        // Reorg at previously seen block height
                        let fork_block = envelope.block_number.saturating_sub(1);
                        return Ok(ContinuityStatus::ReorgDetected {
                            fork_block,
                            fork_hash: envelope.parent_hash,
                            reorg_depth: tip.number.saturating_sub(fork_block),
                        });
                    }
                }
                Ok(ContinuityStatus::Continuous)
            }
        } else {
            // First block seen
            buf.push(envelope.block_ref());
            Ok(ContinuityStatus::Continuous)
        }
    }

    /// Walk back block headers across buffer (and fallback to RPC) to locate the fork point.
    async fn find_common_ancestor(
        &self,
        buf: &RollingBlockBuffer,
        start_block: u64,
        mut current_parent: B256,
    ) -> LogrixResult<(u64, B256)> {
        let mut curr_num = start_block.saturating_sub(1);

        while curr_num > 0 {
            if let Some(buf_ref) = buf.get_by_number(curr_num) {
                if buf_ref.hash == current_parent {
                    return Ok((curr_num, current_parent));
                }
            }

            // Fallback: Query remote node for parent hash of curr_num
            match self
                .chain_client
                .fetch_block_envelope(curr_num, &[])
                .await?
            {
                Some(remote_block) => {
                    if let Some(buf_ref) = buf.get_by_number(curr_num) {
                        if buf_ref.hash == remote_block.block_hash {
                            return Ok((curr_num, remote_block.block_hash));
                        }
                    }
                    current_parent = remote_block.parent_hash;
                    // Walk one step back
                    curr_num = curr_num.saturating_sub(1);
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

        // Fork point at genesis or block 0
        Ok((0, B256::ZERO))
    }
}
