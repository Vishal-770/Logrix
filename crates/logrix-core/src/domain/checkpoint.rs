use super::chain::ChainId;
use crate::error::{ErrorSource, LogrixError, LogrixResult};
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
    pub fn try_new(
        chain_id: ChainId,
        last_indexed_block: u64,
        last_indexed_hash: B256,
        is_finalized: bool,
    ) -> LogrixResult<Self> {
        if last_indexed_hash == B256::ZERO {
            return Err(LogrixError::validation(
                ErrorSource::Internal,
                "Checkpoint block hash must be non-zero",
            ));
        }

        Ok(Self {
            chain_id,
            last_indexed_block,
            last_indexed_hash,
            updated_at: Utc::now(),
            is_finalized,
        })
    }

    /// Construct a checkpoint when invalid input is a programming error.
    pub fn new(
        chain_id: ChainId,
        last_indexed_block: u64,
        last_indexed_hash: B256,
        is_finalized: bool,
    ) -> Self {
        Self::try_new(
            chain_id,
            last_indexed_block,
            last_indexed_hash,
            is_finalized,
        )
        .expect("checkpoint construction requires a non-zero block hash")
    }
}
