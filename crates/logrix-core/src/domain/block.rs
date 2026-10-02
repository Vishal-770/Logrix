use super::chain::ChainId;
use alloy_primitives::{Address, Bytes, B256};
use serde::{Deserialize, Serialize};

/// Lightweight pointer to a specific block (number and hash).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockRef {
    pub number: u64,
    pub hash: B256,
}

impl BlockRef {
    pub fn new(number: u64, hash: B256) -> Self {
        Self { number, hash }
    }
}

/// A raw event log emitted by an EVM smart contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventLog {
    /// Contract address emitting the event.
    pub address: Address,
    /// Indexed topic hashes (topic0 is event signature).
    pub topics: Vec<B256>,
    /// Non-indexed ABI encoded event data.
    pub data: Bytes,
    /// Transaction hash containing this log.
    pub tx_hash: B256,
    /// Log index inside the block.
    pub log_index: u64,
    /// Transaction index inside the block.
    pub tx_index: u64,
    /// Block number containing this log.
    pub block_number: u64,
    /// Block hash containing this log.
    pub block_hash: B256,
}

impl EventLog {
    /// Unique compound key for idempotency: (chain_id, block_hash, log_index)
    #[must_use]
    pub fn idempotency_key(&self, chain_id: ChainId) -> String {
        format!("{}:{}:{}", chain_id.as_u64(), self.block_hash, self.log_index)
    }
}

/// Generic block container exchanged across the Source and Decode pipeline boundary.
///
/// Designed to satisfy Design Rule Zero (seam for non-EVM and extra block headers).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockEnvelope {
    pub chain_id: ChainId,
    pub block_number: u64,
    pub block_hash: B256,
    pub parent_hash: B256,
    pub timestamp: u64,
    pub logs: Vec<EventLog>,
}

impl BlockEnvelope {
    #[must_use]
    pub fn block_ref(&self) -> BlockRef {
        BlockRef::new(self.block_number, self.block_hash)
    }

    #[must_use]
    pub fn has_parent_mismatch(&self, expected_parent_hash: B256) -> bool {
        self.parent_hash != expected_parent_hash
    }
}
