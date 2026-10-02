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
        format!(
            "{}:{}:{}",
            chain_id.as_u64(),
            self.block_hash,
            self.log_index
        )
    }
}

/// The category of data carried within a block envelope.
///
/// Leaves seams for future data types (traces, state diffs, non-EVM blocks) without schema rewrites.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeKind {
    /// Standard EVM smart contract event logs.
    #[default]
    Log,
    /// Raw full block header and metadata.
    Block,
    /// Transaction list with status and receipts.
    Transaction,
    /// Parity / Geth execution call traces and internal calls.
    Trace,
    /// State trie diffs / account storage changes.
    StateDiff,
    /// Custom user or non-EVM envelope payload.
    Custom,
}

/// Generic block container exchanged across the Source and Decode pipeline boundary.
///
/// Strictly adheres to the Open/Closed Principle (OCP) and Design Rule Zero:
/// any future chain family or data type can be conveyed via `kind` and `extra` without breaking core traits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockEnvelope {
    pub chain_id: ChainId,
    pub block_number: u64,
    pub block_hash: B256,
    pub parent_hash: B256,
    pub timestamp: u64,
    #[serde(default)]
    pub kind: EnvelopeKind,
    pub logs: Vec<EventLog>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

impl BlockEnvelope {
    pub fn new(
        chain_id: ChainId,
        block_number: u64,
        block_hash: B256,
        parent_hash: B256,
        timestamp: u64,
        logs: Vec<EventLog>,
    ) -> Self {
        Self {
            chain_id,
            block_number,
            block_hash,
            parent_hash,
            timestamp,
            kind: EnvelopeKind::Log,
            logs,
            extra: std::collections::HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_kind(mut self, kind: EnvelopeKind) -> Self {
        self.kind = kind;
        self
    }

    #[must_use]
    pub fn with_extra(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.extra.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn block_ref(&self) -> BlockRef {
        BlockRef::new(self.block_number, self.block_hash)
    }

    #[must_use]
    pub fn has_parent_mismatch(&self, expected_parent_hash: B256) -> bool {
        self.parent_hash != expected_parent_hash
    }
}
