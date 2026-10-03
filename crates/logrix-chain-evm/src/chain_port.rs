use crate::client::EvmChainClient;
use alloy_primitives::{Address, Bytes, B256};
use async_trait::async_trait;
use logrix_core::{
    domain::{BlockEnvelope, BlockRef, EventLog},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::ChainPort,
};
use serde_json::{json, Value};
use tracing::debug;

#[async_trait]
impl ChainPort for EvmChainClient {
    async fn get_latest_block_number(&self) -> LogrixResult<u64> {
        let res = self.call_rpc("eth_blockNumber", json!([])).await?;
        let hex_str = res.as_str().ok_or_else(|| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                "eth_blockNumber did not return string",
            )
        })?;

        let stripped = hex_str.trim_start_matches("0x");
        u64::from_str_radix(stripped, 16).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                format!("Failed to parse hex block number '{hex_str}': {e}"),
            )
        })
    }

    async fn get_block_by_number(&self, number: u64) -> LogrixResult<Option<BlockRef>> {
        let hex_block = format!("0x{:x}", number);
        let res = self
            .call_rpc("eth_getBlockByNumber", json!([hex_block, false]))
            .await?;

        if res.is_null() {
            return Ok(None);
        }

        let hash_str = res.get("hash").and_then(|v| v.as_str()).ok_or_else(|| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                "Block missing hash field",
            )
        })?;

        let hash: B256 = hash_str.parse().map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                format!("Invalid block hash '{hash_str}': {e}"),
            )
        })?;

        Ok(Some(BlockRef::new(number, hash)))
    }

    async fn fetch_logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &[Address],
    ) -> LogrixResult<Vec<EventLog>> {
        let mut filter = json!({
            "fromBlock": format!("0x{:x}", from_block),
            "toBlock": format!("0x{:x}", to_block),
        });

        if !addresses.is_empty() {
            let addr_strs: Vec<String> = addresses.iter().map(|a| format!("{:#x}", a)).collect();
            filter["address"] = json!(addr_strs);
        }

        let res = self.call_rpc("eth_getLogs", json!([filter])).await?;
        let logs_array = res.as_array().ok_or_else(|| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                "eth_getLogs did not return an array",
            )
        })?;

        let event_logs: Vec<EventLog> = logs_array.iter().map(parse_rpc_log).collect();
        debug!(
            from_block,
            to_block,
            count = event_logs.len(),
            "Fetched logs from EVM RPC"
        );
        Ok(event_logs)
    }

    async fn fetch_block_envelope(
        &self,
        number: u64,
        addresses: &[Address],
    ) -> LogrixResult<Option<BlockEnvelope>> {
        let hex_block = format!("0x{:x}", number);
        let block_res = self
            .call_rpc("eth_getBlockByNumber", json!([hex_block, false]))
            .await?;

        if block_res.is_null() {
            return Ok(None);
        }

        let hash_str = block_res
            .get("hash")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let parent_hash_str = block_res
            .get("parentHash")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let timestamp_hex = block_res
            .get("timestamp")
            .and_then(|v| v.as_str())
            .unwrap_or("0x0");

        let hash: B256 = hash_str.parse().unwrap_or_default();
        let parent_hash: B256 = parent_hash_str.parse().unwrap_or_default();
        let timestamp =
            u64::from_str_radix(timestamp_hex.trim_start_matches("0x"), 16).unwrap_or(0);

        let logs = self.fetch_logs(number, number, addresses).await?;
        Ok(Some(BlockEnvelope::new(
            self.chain_id(),
            number,
            hash,
            parent_hash,
            timestamp,
            logs,
        )))
    }
}

fn parse_rpc_log(item: &Value) -> EventLog {
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

fn parse_hex_u64(val: Option<&str>) -> u64 {
    val.map(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(0))
        .unwrap_or(0)
}
