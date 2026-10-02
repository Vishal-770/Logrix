use super::chain::ChainId;
use alloy_primitives::B256;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Checkpoint recording the highest contiguous block successfully decoded and committed to the store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub chain_id: ChainId,
    pub last_indexed_block: u64,
    pub last_indexed_hash: B256,
    pub updated_at: DateTime<Utc>,
    /// Whether this block has surpassed the safe finality threshold of the target chain.
    pub is_finalized: bool,
}

impl Checkpoint {
    pub fn new(
        chain_id: ChainId,
        last_indexed_block: u64,
        last_indexed_hash: B256,
        is_finalized: bool,
    ) -> Self {
        Self {
            chain_id,
            last_indexed_block,
            last_indexed_hash,
            updated_at: Utc::now(),
            is_finalized,
        }
    }
}
