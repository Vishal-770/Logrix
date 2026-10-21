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

/// Parse a required hex u64 string, returning explicit error on failure.
pub fn parse_required_hex_u64(val: Option<&str>, field: &str) -> LogrixResult<u64> {
    let raw = val.ok_or_else(|| {
        LogrixError::integrity(
            ErrorSource::ChainRpc,
            format!("Missing required hex field '{field}'"),
        )
    })?;

    let clean = raw.trim_start_matches("0x");
    u64::from_str_radix(clean, 16).map_err(|e| {
        LogrixError::integrity(
            ErrorSource::ChainRpc,
            format!("Invalid hex for field '{field}' ('{raw}'): {e}"),
        )
    })
}

/// Parse a hex u64 string with optional 0x prefix, defaulting to 0 if absent or invalid.
pub fn parse_hex_u64(val: Option<&str>) -> u64 {
    val.and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
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
    let timestamp_hex = block_res.get("timestamp").and_then(|v| v.as_str());

    let hash = parse_required_hash(hash_str, "hash")?;
    let parent_hash = parse_required_hash(parent_hash_str, "parentHash")?;
    let timestamp = parse_required_hex_u64(timestamp_hex, "timestamp")?;

    Ok(BlockEnvelope::new(
        chain_id,
        number,
        hash,
        parent_hash,
        timestamp,
        logs,
    ))
}

/// Strictly parse an individual eth_getLogs item into an EventLog, rejecting missing metadata.
pub fn try_parse_rpc_log(item: &Value) -> LogrixResult<EventLog> {
    let addr_str = item
        .get("address")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            LogrixError::integrity(ErrorSource::ChainRpc, "RPC log missing 'address' field")
        })?;
    let address: Address = addr_str.parse().map_err(|e| {
        LogrixError::integrity(
            ErrorSource::ChainRpc,
            format!("Invalid log address '{addr_str}': {e}"),
        )
    })?;

    let mut topics = Vec::new();
    if let Some(t_arr) = item.get("topics").and_then(|v| v.as_array()) {
        for (idx, t) in t_arr.iter().enumerate() {
            let ts = t.as_str().ok_or_else(|| {
                LogrixError::integrity(
                    ErrorSource::ChainRpc,
                    format!("Topic at index {idx} not a string"),
                )
            })?;
            let b = ts.parse::<B256>().map_err(|e| {
                LogrixError::integrity(
                    ErrorSource::ChainRpc,
                    format!("Invalid topic hash '{ts}' at index {idx}: {e}"),
                )
            })?;
            topics.push(b);
        }
    }

    let data_str = item.get("data").and_then(|v| v.as_str()).unwrap_or("0x");
    let data_bytes =
        alloy_primitives::hex::decode(data_str.trim_start_matches("0x")).map_err(|e| {
            LogrixError::integrity(
                ErrorSource::ChainRpc,
                format!("Invalid log hex data '{data_str}': {e}"),
            )
        })?;

    let tx_hash = parse_required_hash(
        item.get("transactionHash").and_then(|v| v.as_str()),
        "transactionHash",
    )?;
    let block_hash =
        parse_required_hash(item.get("blockHash").and_then(|v| v.as_str()), "blockHash")?;

    let block_number = parse_required_hex_u64(
        item.get("blockNumber").and_then(|v| v.as_str()),
        "blockNumber",
    )?;
    let log_index =
        parse_required_hex_u64(item.get("logIndex").and_then(|v| v.as_str()), "logIndex")?;
    let tx_index = parse_required_hex_u64(
        item.get("transactionIndex").and_then(|v| v.as_str()),
        "transactionIndex",
    )?;

    Ok(EventLog {
        address,
        topics,
        data: Bytes::from(data_bytes),
        tx_hash,
        log_index,
        tx_index,
        block_number,
        block_hash,
    })
}

/// Parse an individual eth_getLogs item into an EventLog (best-effort fallback).
pub fn parse_rpc_log(item: &Value) -> EventLog {
    try_parse_rpc_log(item).unwrap_or_else(|_| EventLog {
        address: Address::default(),
        topics: Vec::new(),
        data: Bytes::default(),
        tx_hash: B256::ZERO,
        log_index: 0,
        tx_index: 0,
        block_number: 0,
        block_hash: B256::ZERO,
    })
}

/// Extract standard Retry-After header as Duration.
pub fn extract_retry_after(headers: &reqwest::header::HeaderMap) -> Option<std::time::Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .map(std::time::Duration::from_secs)
}
