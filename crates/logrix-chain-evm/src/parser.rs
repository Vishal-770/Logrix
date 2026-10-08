use alloy_primitives::{Address, Bytes, B256};
use logrix_core::{
    domain::{BlockEnvelope, ChainId, EventLog},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};
use serde_json::Value;

/// Parse a required 32-byte hash string, rejecting missing or invalid values.
pub fn parse_required_hash(val: Option<&str>, field: &str) -> LogrixResult<B256> {
    let raw = val.ok_or_else(|| {
        LogrixError::new(
            ErrorClass::Permanent,
            ErrorSource::ChainRpc,
            format!("Block response missing required field '{field}'"),
        )
    })?;

    raw.parse::<B256>().map_err(|e| {
        LogrixError::new(
            ErrorClass::Permanent,
            ErrorSource::ChainRpc,
            format!("Invalid hash for field '{field}' ('{raw}'): {e}"),
        )
    })
}

/// Parse a hex u64 string with optional 0x prefix.
pub fn parse_hex_u64(val: Option<&str>) -> u64 {
    val.map(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0))
        .unwrap_or(0)
}

/// Strictly parse a block JSON-RPC object into a BlockEnvelope.
pub fn parse_block_envelope(
    block_res: &Value,
    logs: Vec<EventLog>,
    chain_id: ChainId,
    number: u64,
) -> LogrixResult<BlockEnvelope> {
    let hash_str = block_res.get("hash").and_then(|v| v.as_str());
    let parent_hash_str = block_res.get("parentHash").and_then(|v| v.as_str());
    let timestamp_hex = block_res
        .get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("0x0");

    let hash = parse_required_hash(hash_str, "hash")?;
    let parent_hash = parse_required_hash(parent_hash_str, "parentHash")?;
    let timestamp = parse_hex_u64(Some(timestamp_hex));

    Ok(BlockEnvelope::new(
        chain_id,
        number,
        hash,
        parent_hash,
        timestamp,
        logs,
    ))
}

/// Parse an individual eth_getLogs item into an EventLog.
pub fn parse_rpc_log(item: &Value) -> EventLog {
    let address: Address = item
        .get("address")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .parse()
        .unwrap_or_default();
    let mut topics = Vec::new();
    if let Some(t_arr) = item.get("topics").and_then(|v| v.as_array()) {
        for t in t_arr {
            if let Some(ts) = t.as_str() {
                if let Ok(b) = ts.parse::<B256>() {
                    topics.push(b);
                }
            }
        }
    }
    let data_str = item.get("data").and_then(|v| v.as_str()).unwrap_or("0x");
    let data_bytes =
        alloy_primitives::hex::decode(data_str.trim_start_matches("0x")).unwrap_or_default();
    let tx_hash: B256 = item
        .get("transactionHash")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .parse()
        .unwrap_or_default();
    let block_hash: B256 = item
        .get("blockHash")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .parse()
        .unwrap_or_default();
    let block_number = parse_hex_u64(item.get("blockNumber").and_then(|v| v.as_str()));
    let log_index = parse_hex_u64(item.get("logIndex").and_then(|v| v.as_str()));
    let tx_index = parse_hex_u64(item.get("transactionIndex").and_then(|v| v.as_str()));

    EventLog {
        address,
        topics,
        data: Bytes::from(data_bytes),
        tx_hash,
        log_index,
        tx_index,
        block_number,
        block_hash,
    }
}
