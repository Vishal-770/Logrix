use crate::client::EvmChainClient;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::Instant;

impl EvmChainClient {
    /// Perform a batch JSON-RPC call in a single HTTP POST request.
    ///
    /// Preserves the exact ordering of the input `calls` slice in the returned results.
    pub async fn call_rpc_batch(&self, calls: Vec<(&str, Value)>) -> LogrixResult<Vec<Value>> {
        if calls.is_empty() {
            return Ok(Vec::new());
        }

        self.rate_limiter.acquire(calls.len() as f64).await;
        let mut id_map = Vec::with_capacity(calls.len());
        let mut batch_payload = Vec::with_capacity(calls.len());

        for (method, params) in calls {
            let id = self.rpc_id.fetch_add(1, Ordering::Relaxed);
            id_map.push(id);
            batch_payload.push(json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }));
        }

        let start = Instant::now();
        let resp = self
            .http_client
            .post(&self.rpc_url)
            .json(&batch_payload)
            .send()
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::ChainRpc,
                    format!("Batch HTTP request failed: {e}"),
                )
            })?;

        let status = resp.status();
        if !status.is_success() {
            return Err(LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::ChainRpc,
                format!("Batch RPC HTTP status: {status}"),
            ));
        }

        let raw: Value = resp.json().await.map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::ChainRpc,
                format!("Failed to parse batch JSON response: {e}"),
            )
        })?;

        let arr = raw.as_array().ok_or_else(|| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                "Batch RPC response not an array",
            )
        })?;

        let mut results = Vec::with_capacity(id_map.len());
        for expected_id in id_map {
            let matched = arr
                .iter()
                .find(|item| item.get("id").and_then(|v| v.as_u64()) == Some(expected_id));

            match matched {
                Some(item) => {
                    if let Some(err) = item.get("error") {
                        return Err(LogrixError::new(
                            ErrorClass::Transient,
                            ErrorSource::ChainRpc,
                            format!("Batch sub-call error: {err}"),
                        ));
                    }
                    results.push(item.get("result").cloned().unwrap_or(Value::Null));
                }
                None => {
                    return Err(LogrixError::new(
                        ErrorClass::Permanent,
                        ErrorSource::ChainRpc,
                        format!("Batch response missing id {expected_id}"),
                    ));
                }
            }
        }

        self.chunker.record_success(start.elapsed());
        Ok(results)
    }
}
