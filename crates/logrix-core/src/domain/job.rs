use super::chain::ChainId;
use alloy_primitives::B256;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    pub fn new(
        chain_id: ChainId,
        from_block: u64,
        to_block: u64,
        partition_key: impl Into<String>,
    ) -> Self {
        Self {
            job_id: Uuid::new_v4().to_string(),
            chain_id,
            from_block,
            to_block,
            partition_key: partition_key.into(),
            attempt: 1,
        }
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

/// Strongly-typed queue message exchanged between producers and decoders.
///
/// Versioned JSON serialization ensures backward and forward compatibility.
///
/// Marked `#[non_exhaustive]` to adhere strictly to the Open/Closed Principle (OCP):
/// new job variants or plugins can be introduced without breaking existing match logic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "schema_version", content = "payload")]
#[non_exhaustive]
pub enum QueueMessage {
    #[serde(rename = "v1:live_block")]
    LiveBlock(LiveBlockJob),

    #[serde(rename = "v1:backfill_range")]
    BackfillRange(BlockRangeJob),

    #[serde(rename = "v1:custom")]
    Custom {
        job_id: String,
        chain_id: ChainId,
        job_type: String,
        attempt: u32,
        payload: serde_json::Value,
    },
}

impl QueueMessage {
    #[must_use]
    pub fn job_id(&self) -> &str {
        match self {
            Self::LiveBlock(j) => &j.job_id,
            Self::BackfillRange(j) => &j.job_id,
            Self::Custom { job_id, .. } => job_id,
        }
    }

    #[must_use]
    pub fn chain_id(&self) -> ChainId {
        match self {
            Self::LiveBlock(j) => j.chain_id,
            Self::BackfillRange(j) => j.chain_id,
            Self::Custom { chain_id, .. } => *chain_id,
        }
    }

    #[must_use]
    pub fn attempt(&self) -> u32 {
        match self {
            Self::LiveBlock(j) => j.attempt,
            Self::BackfillRange(j) => j.attempt,
            Self::Custom { attempt, .. } => *attempt,
        }
    }

    pub fn increment_attempt(&mut self) {
        match self {
            Self::LiveBlock(j) => j.attempt += 1,
            Self::BackfillRange(j) => j.attempt += 1,
            Self::Custom { attempt, .. } => *attempt += 1,
        }
    }
}

/// Abstract receipt handle returned upon consuming a message from any queue provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageHandle {
    pub receipt_id: String,
    pub message: QueueMessage,
}

impl MessageHandle {
    pub fn new(receipt_id: impl Into<String>, message: QueueMessage) -> Self {
        Self {
            receipt_id: receipt_id.into(),
            message,
        }
    }
}
