use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// High-level domain entity representing an ERC-20 token transfer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenTransfer {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
    pub contract_address: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub timestamp: DateTime<Utc>,
}

/// Filter criteria for querying token transfers.
#[derive(Debug, Default, Clone)]
pub struct TransferFilter {
    pub contract_address: Option<String>,
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
