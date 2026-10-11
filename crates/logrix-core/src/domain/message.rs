use super::chain::ChainId;
use super::job::{BlockRangeJob, LiveBlockJob, QueueType};
use serde::{Deserialize, Serialize};

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
    pub source_queue: Option<QueueType>,
}

impl MessageHandle {
    pub fn new(receipt_id: impl Into<String>, message: QueueMessage) -> Self {
        Self {
            receipt_id: receipt_id.into(),
            message,
            source_queue: None,
        }
    }

    pub fn with_queue(
        receipt_id: impl Into<String>,
        message: QueueMessage,
        queue_type: QueueType,
    ) -> Self {
        Self {
            receipt_id: receipt_id.into(),
            message,
            source_queue: Some(queue_type),
        }
    }
}
