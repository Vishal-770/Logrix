use alloy_dyn_abi::{DynSolValue, EventExt};
use alloy_json_abi::{Event, JsonAbi};
use alloy_primitives::{b256, B256};
use chrono::Utc;
use logrix_core::domain::EventLog;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const ERC20_TRANSFER_TOPIC: B256 =
    b256!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef");

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

#[derive(Debug, Clone, Default)]
pub struct AbiEventDecoder {
    events: HashMap<B256, Event>,
}

impl AbiEventDecoder {
    pub fn new() -> Self {
        Self {
            events: HashMap::new(),
        }
    }

    pub fn standard_erc20() -> Self {
        let mut decoder = Self::new();
        let _ = decoder.add_event_signature(
            "Transfer(address indexed from, address indexed to, uint256 value)",
        );
        let _ = decoder.add_event_signature(
            "Approval(address indexed owner, address indexed spender, uint256 value)",
        );
        decoder
    }

    pub fn add_abi_json(&mut self, abi_json: &str) -> Result<usize, String> {
        let abi: JsonAbi = serde_json::from_str(abi_json)
            .map_err(|e| format!("Invalid contract ABI JSON: {e}"))?;
        let mut count = 0;
        for (_name, event_list) in abi.events {
            for event in event_list {
                let selector = event.selector();
                self.events.insert(selector, event);
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn add_event_signature(&mut self, sig: &str) -> Result<B256, String> {
        let raw = sig.trim();
        let formatted = if raw.starts_with("event ") {
            raw.to_string()
        } else {
            format!("event {raw}")
        };
        let event: Event = formatted
            .parse()
            .map_err(|e| format!("Invalid event signature: {e}"))?;
        let selector = event.selector();
        self.events.insert(selector, event);
        Ok(selector)
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn decode_log(&self, log: &EventLog) -> Option<DecodedEvent> {
        let topic0 = log.topics.first()?;
        let event = self.events.get(topic0)?;

        let decoded = event
            .decode_log_parts(log.topics.clone(), &log.data, false)
            .ok()?;
        let mut params = HashMap::new();

        let indexed_inputs: Vec<_> = event.inputs.iter().filter(|i| i.indexed).collect();
        for (i, input) in indexed_inputs.iter().enumerate() {
            if let Some(val) = decoded.indexed.get(i) {
                let key = if input.name.is_empty() {
                    format!("param_{i}")
                } else {
                    input.name.clone()
                };
                params.insert(key, dyn_sol_to_json(val));
            }
        }

        let body_inputs: Vec<_> = event.inputs.iter().filter(|i| !i.indexed).collect();
        for (i, input) in body_inputs.iter().enumerate() {
            if let Some(val) = decoded.body.get(i) {
                let key = if input.name.is_empty() {
                    format!("param_{}", indexed_inputs.len() + i)
                } else {
                    input.name.clone()
                };
                params.insert(key, dyn_sol_to_json(val));
            }
        }

        Some(DecodedEvent {
            event_name: event.name.clone(),
            contract_address: format!("{:#x}", log.address),
            block_number: log.block_number,
            block_hash: format!("{:#x}", log.block_hash),
            tx_hash: format!("{:#x}", log.tx_hash),
            log_index: log.log_index,
            params,
            timestamp: Utc::now(),
        })
    }
}

static STANDARD_ERC20_DECODER: std::sync::LazyLock<AbiEventDecoder> =
    std::sync::LazyLock::new(AbiEventDecoder::standard_erc20);

pub fn decode_erc20_transfer(chain_id: u64, log: &EventLog) -> Option<DecodedTransfer> {
    let decoded = STANDARD_ERC20_DECODER.decode_log(log)?;
    if decoded.event_name != "Transfer" {
        return None;
    }
    let from = decoded.params.get("from")?.as_str()?.to_string();
    let to = decoded.params.get("to")?.as_str()?.to_string();
    let amount = decoded.params.get("value")?.as_str()?.to_string();

    Some(DecodedTransfer {
        chain_id,
        block_number: decoded.block_number,
        block_hash: decoded.block_hash,
        tx_hash: decoded.tx_hash,
        log_index: decoded.log_index,
        contract_address: decoded.contract_address,
        from_address: from,
        to_address: to,
        amount,
        timestamp: decoded.timestamp,
    })
}

fn dyn_sol_to_json(val: &DynSolValue) -> serde_json::Value {
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
