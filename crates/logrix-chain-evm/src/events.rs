use alloy_dyn_abi::DynSolValue;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const ERC20_TRANSFER_TOPIC: alloy_primitives::B256 =
    alloy_primitives::b256!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecodedEvent {
    pub event_name: String,
    pub contract_address: String,
    pub block_number: u64,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
    pub params: HashMap<String, serde_json::Value>,
    pub timestamp: chrono::DateTime<Utc>,
}

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

impl DecodedTransfer {
    /// Parse amount string as `alloy_primitives::U256` preventing floating-point truncation.
    pub fn amount_u256(&self) -> Result<alloy_primitives::U256, String> {
        self.amount
            .parse::<alloy_primitives::U256>()
            .map_err(|e| format!("Invalid U256 amount: {e}"))
    }
}

pub(crate) fn dyn_sol_to_json(val: &DynSolValue) -> serde_json::Value {
    match val {
        DynSolValue::Address(addr) => serde_json::json!(format!("{:#x}", addr)),
        DynSolValue::Bool(b) => serde_json::json!(b),
        DynSolValue::Int(i, _) => serde_json::json!(i.to_string()),
        DynSolValue::Uint(u, _) => serde_json::json!(u.to_string()),
        DynSolValue::FixedBytes(bytes, _) => serde_json::json!(format!("{:#x}", bytes)),
        DynSolValue::Bytes(b) => {
            serde_json::json!(format!("0x{}", alloy_primitives::hex::encode(b)))
        }
        DynSolValue::String(s) => serde_json::json!(s),
        DynSolValue::Array(arr) | DynSolValue::FixedArray(arr) => {
            serde_json::json!(arr.iter().map(dyn_sol_to_json).collect::<Vec<_>>())
        }
        DynSolValue::Tuple(tup) => {
            serde_json::json!(tup.iter().map(dyn_sol_to_json).collect::<Vec<_>>())
        }
        _ => serde_json::Value::Null,
    }
}
