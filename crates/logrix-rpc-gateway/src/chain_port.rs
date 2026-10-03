use crate::RpcGateway;
use alloy_primitives::Address;
use async_trait::async_trait;
use logrix_core::domain::{BlockEnvelope, BlockRef, EventLog};
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use logrix_core::ports::ChainPort;
use tracing::debug;

#[async_trait]
impl ChainPort for RpcGateway {
    async fn get_latest_block_number(&self) -> LogrixResult<u64> {
        let key = "latest_block_number";
        let res_str = self
            .singleflight
            .execute(key, || async {
                self.execute_with_fallback("eth_blockNumber", |provider| async move {
                    provider.client().get_latest_block_number().await
                })
                .await
                .map(|num| num.to_string())
            })
            .await?;

        res_str.parse::<u64>().map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::ChainRpc,
                format!("Failed to parse singleflight block number: {e}"),
            )
        })
    }

    async fn get_block_by_number(&self, number: u64) -> LogrixResult<Option<BlockRef>> {
        self.execute_with_fallback("eth_getBlockByNumber", |provider| async move {
            provider.client().get_block_by_number(number).await
        })
        .await
    }

    async fn fetch_logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &[Address],
    ) -> LogrixResult<Vec<EventLog>> {
        if self.bulk_stream.is_supported() {
            if let Ok(Some(logs)) = self
                .bulk_stream
                .stream_logs(from_block, to_block, addresses)
                .await
            {
                debug!(
                    count = logs.len(),
                    "Retrieved logs from bulk stream fast-path"
                );
                return Ok(logs);
            }
        }

        self.execute_with_fallback("eth_getLogs", |provider| {
            let addrs = addresses.to_vec();
            async move {
                provider
                    .client()
                    .fetch_logs(from_block, to_block, &addrs)
                    .await
            }
        })
        .await
    }

    async fn fetch_block_envelope(
        &self,
        number: u64,
        addresses: &[Address],
    ) -> LogrixResult<Option<BlockEnvelope>> {
        self.execute_with_fallback("eth_getBlockByNumber", |provider| {
            let addrs = addresses.to_vec();
            async move { provider.client().fetch_block_envelope(number, &addrs).await }
        })
        .await
    }
}
