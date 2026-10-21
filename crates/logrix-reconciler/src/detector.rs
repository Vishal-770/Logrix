use crate::ancestor::find_common_ancestor;
use crate::buffer::RollingBlockBuffer;
use alloy_primitives::B256;
use logrix_core::domain::{BlockEnvelope, ChainId};
use logrix_core::error::LogrixResult;
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

/// Default floor for maximum reorg ancestor walk depth (128 blocks).
pub const DEFAULT_MAX_REORG_DEPTH: u64 = 128;

/// Blockchain reorganization detector coordinating in-memory buffer and RPC ancestor walking.
#[derive(Clone)]
pub struct ReorgDetector {
    chain_id: ChainId,
    buffer: Arc<RwLock<RollingBlockBuffer>>,
    chain_client: Arc<dyn ChainPort>,
    max_reorg_depth: u64,
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
            max_reorg_depth: (buffer_capacity * 2).max(DEFAULT_MAX_REORG_DEPTH as usize) as u64,
        }
    }

    /// Configure maximum reorg ancestor walk depth limit.
    #[must_use]
    pub fn with_max_reorg_depth(mut self, max_depth: u64) -> Self {
        self.max_reorg_depth = max_depth;
        self
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
                    let buf_snapshot = buf.clone();
                    drop(buf);

                    let (fork_block, fork_hash) = find_common_ancestor(
                        self.chain_client.as_ref(),
                        &buf_snapshot,
                        envelope.block_number,
                        envelope.parent_hash,
                        self.max_reorg_depth,
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
}
