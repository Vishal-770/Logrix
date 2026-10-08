use crate::client::EvmChainClient;
use crate::parser::{parse_block_envelope, parse_required_hash, parse_rpc_log};
use alloy_primitives::{Address, B256};
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

        let hash_str = res.get("hash").and_then(|v| v.as_str());
        let hash: B256 = parse_required_hash(hash_str, "hash")?;
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
        let mut filter = json!({
            "fromBlock": &hex_block,
            "toBlock": &hex_block,
        });

        if !addresses.is_empty() {
            let addr_strs: Vec<String> = addresses.iter().map(|a| format!("{:#x}", a)).collect();
            filter["address"] = json!(addr_strs);
        }

        // Use JSON-RPC batching for 1 round-trip fetch of header + event logs
        let calls = vec![
            ("eth_getBlockByNumber", json!([&hex_block, false])),
            ("eth_getLogs", json!([filter])),
        ];

        let results = match self.call_rpc_batch(calls).await {
            Ok(res) => res,
            Err(_) => {
                // Graceful fallback to sequential RPC calls if provider rejects batching
                let block = self
                    .call_rpc("eth_getBlockByNumber", json!([&hex_block, false]))
                    .await?;
                let logs = self
                    .call_rpc(
                        "eth_getLogs",
                        json!([{
                            "fromBlock": &hex_block,
                            "toBlock": &hex_block,
                            "address": if addresses.is_empty() {
                                Value::Null
                            } else {
                                json!(addresses.iter().map(|a| format!("{:#x}", a)).collect::<Vec<_>>())
                            }
                        }]),
                    )
                    .await?;
                vec![block, logs]
            }
        };

        let block_res = &results[0];
        let logs_res = &results[1];

        if block_res.is_null() {
            return Ok(None);
        }

        let logs_array = logs_res.as_array().ok_or_else(|| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                "eth_getLogs did not return an array",
            )
        })?;

        let event_logs: Vec<EventLog> = logs_array.iter().map(parse_rpc_log).collect();
        let envelope = parse_block_envelope(block_res, event_logs, self.chain_id(), number)?;
        Ok(Some(envelope))
    }
}
