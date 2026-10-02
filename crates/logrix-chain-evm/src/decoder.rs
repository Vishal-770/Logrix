use alloy_primitives::{b256, Address, B256, U256};
use chrono::Utc;
use logrix_core::domain::EventLog;
use serde::{Deserialize, Serialize};

/// Canonical keccak256 hash of `Transfer(address,address,uint256)`
pub const ERC20_TRANSFER_TOPIC: B256 =
    b256!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

/// Decoded ERC-20 transfer event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedTransfer {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
    pub contract_address: String,
    pub from_address: String,
    pub to_address: String,
    pub amount: String,
    pub timestamp: chrono::DateTime<Utc>,
}

/// Decode an EVM EventLog into a DecodedTransfer if it matches ERC-20 Transfer signature.
pub fn decode_erc20_transfer(chain_id: u64, log: &EventLog) -> Option<DecodedTransfer> {
    if log.topics.len() < 3 || log.topics[0] != ERC20_TRANSFER_TOPIC {
        return None;
    }

    // topics[1] contains indexed 'from' address in lower 20 bytes
    let from_bytes: &[u8; 32] = log.topics[1].as_ref();
    let from_addr = Address::from_slice(&from_bytes[12..32]);

    // topics[2] contains indexed 'to' address in lower 20 bytes
    let to_bytes: &[u8; 32] = log.topics[2].as_ref();
    let to_addr = Address::from_slice(&to_bytes[12..32]);

    // data contains non-indexed 'value' uint256
    let amount_u256 = if log.data.len() >= 32 {
        U256::from_be_slice(&log.data[0..32])
    } else {
        U256::ZERO
    };

    Some(DecodedTransfer {
        chain_id,
        block_number: log.block_number,
        block_hash: format!("{:#x}", log.block_hash),
        tx_hash: format!("{:#x}", log.tx_hash),
        log_index: log.log_index,
        contract_address: format!("{:#x}", log.address),
        from_address: format!("{:#x}", from_addr),
        to_address: format!("{:#x}", to_addr),
        amount: amount_u256.to_string(),
        timestamp: Utc::now(),
    })
}
