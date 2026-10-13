use crate::client::EvmChainClient;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

impl EvmChainClient {
    /// Perform a raw JSON-RPC call with token bucket rate limiting and retry backoff.
    pub(crate) async fn call_rpc(&self, method: &str, params: Value) -> LogrixResult<Value> {
        let mut attempt = 0;

        loop {
            self.rate_limiter.acquire(1.0).await;

            let id = self.rpc_id.fetch_add(1, Ordering::Relaxed);
            let body = json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            });

            let start = Instant::now();
            let res = self
                .http_client
                .post(&self.rpc_url)
                .json(&body)
                .send()
                .await;

            match res {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let json_res: Value = resp.json().await.map_err(|e| {
                            LogrixError::new(
                                ErrorClass::Transient,
                                ErrorSource::ChainRpc,
                                format!("Failed to parse RPC response: {e}"),
                            )
                        })?;

                        if let Some(err) = json_res.get("error") {
                            let err_msg = err.to_string();
                            if err_msg.contains("more than")
                                || err_msg.contains("exceeded")
                                || err_msg.contains("range")
                            {
                                self.chunker.record_failure();
                            }
                            if attempt < self.retry_policy.max_attempts {
                                attempt += 1;
                                let delay = self.retry_policy.delay_for_attempt(attempt);
                                tokio::time::sleep(delay).await;
                                continue;
                            }
                            return Err(LogrixError::new(
                                ErrorClass::Permanent,
                                ErrorSource::ChainRpc,
                                format!("RPC response contained error: {err_msg}"),
                            ));
                        }

                        self.chunker.record_success(start.elapsed());
                        return Ok(json_res["result"].clone());
                    }

                    if status.is_server_error() || status.as_u16() == 429 {
                        self.chunker.record_failure();
                        let retry_after = resp
                            .headers()
                            .get("retry-after")
                            .and_then(|h| h.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok())
                            .map(Duration::from_secs);

                        if attempt < self.retry_policy.max_attempts {
                            attempt += 1;
                            let delay = retry_after
                                .unwrap_or_else(|| self.retry_policy.delay_for_attempt(attempt));
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        if status.as_u16() == 429 {
                            return Err(LogrixError::rate_limited(
                                ErrorSource::ChainRpc,
                                "HTTP 429 Too Many Requests from RPC gateway",
                                retry_after.or(Some(Duration::from_secs(2))),
                            ));
                        }
                        return Err(LogrixError::new(
                            ErrorClass::Transient,
                            ErrorSource::ChainRpc,
                            format!("RPC HTTP error status: {status}"),
                        ));
                    }

                    // Non-retryable client errors (400, 401, 403, 404, etc.)
                    self.chunker.record_failure();
                    return Err(LogrixError::new(
                        ErrorClass::Permanent,
                        ErrorSource::ChainRpc,
                        format!("RPC client HTTP error status: {status}"),
                    ));
                }
                Err(e) => {
                    self.chunker.record_failure();
                    if attempt < self.retry_policy.max_attempts {
                        attempt += 1;
                        let delay = self.retry_policy.delay_for_attempt(attempt);
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    return Err(LogrixError::new(
                        ErrorClass::Transient,
                        ErrorSource::ChainRpc,
                        format!("HTTP request failed: {e}"),
                    ));
                }
            }
        }
    }
}
