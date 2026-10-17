use super::chain::ChainId;
use crate::error::{ErrorSource, LogrixError, LogrixResult};
use alloy_primitives::B256;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use super::message::{MessageHandle, QueueMessage};

/// Logical queue types in Logrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueType {
    /// High-priority live head blocks.
    Live,
    /// Historical block range chunks.
    Backfill,
    /// Outgoing webhook notifications.
    Webhook,
    /// Dead-letter queue for repeatedly failing jobs.
    DeadLetter,
}

/// A job to process a single newly discovered block from the chain head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveBlockJob {
    pub job_id: String,
    pub chain_id: ChainId,
    pub block_number: u64,
    pub block_hash: B256,
    pub parent_hash: B256,
    pub attempt: u32,
}

impl LiveBlockJob {
    pub fn new(chain_id: ChainId, block_number: u64, block_hash: B256, parent_hash: B256) -> Self {
        Self {
            job_id: Uuid::new_v4().to_string(),
            chain_id,
            block_number,
            block_hash,
            parent_hash,
            attempt: 1,
        }
    }
}

/// A job to index a chunk range of historical blocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRangeJob {
    pub job_id: String,
    pub chain_id: ChainId,
    pub from_block: u64,
    pub to_block: u64,
    pub partition_key: String,
    pub attempt: u32,
}

impl BlockRangeJob {
    pub fn try_new(
        chain_id: ChainId,
        from_block: u64,
        to_block: u64,
        partition_key: impl Into<String>,
    ) -> LogrixResult<Self> {
        if from_block > to_block {
            return Err(LogrixError::validation(
                ErrorSource::Internal,
                format!(
                    "Block range is inverted: from_block ({from_block}) is greater than to_block ({to_block})"
                ),
            ));
        }

        Ok(Self {
            job_id: Uuid::new_v4().to_string(),
            chain_id,
            from_block,
            to_block,
            partition_key: partition_key.into(),
            attempt: 1,
        })
    }

    /// Construct a range when inverted input is a programming error.
    pub fn new(
        chain_id: ChainId,
        from_block: u64,
        to_block: u64,
        partition_key: impl Into<String>,
    ) -> Self {
        Self::try_new(chain_id, from_block, to_block, partition_key)
            .expect("block range construction requires from_block <= to_block")
    }

    #[must_use]
    pub fn block_count(&self) -> u64 {
        if self.to_block >= self.from_block {
            self.to_block - self.from_block + 1
        } else {
            0
        }
    }

    /// Halve this block range on "range too large" errors.
    #[must_use]
    pub fn split_half(&self) -> (Self, Option<Self>) {
        if self.from_block >= self.to_block {
            return (self.clone(), None);
        }
        let mid = self.from_block + (self.to_block - self.from_block) / 2;
        let first = Self {
            job_id: Uuid::new_v4().to_string(),
            chain_id: self.chain_id,
            from_block: self.from_block,
            to_block: mid,
            partition_key: self.partition_key.clone(),
            attempt: 1,
        };
        let second = Self {
            job_id: Uuid::new_v4().to_string(),
            chain_id: self.chain_id,
            from_block: mid + 1,
            to_block: self.to_block,
            partition_key: self.partition_key.clone(),
            attempt: 1,
        };
        (first, Some(second))
    }
}
