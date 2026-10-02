use crate::domain::{BlockEnvelope, BlockRef, EventLog};
use crate::error::LogrixResult;
use alloy_primitives::Address;
use async_trait::async_trait;

/// Abstract chain client port for querying EVM nodes.
///
/// Implemented by the RPC Gateway (`logrix-chain-evm`), bulk streamers, or local node clients.
#[async_trait]
pub trait ChainPort: Send + Sync {
    /// Retrieve the current latest block number from the chain head.
    async fn get_latest_block_number(&self) -> LogrixResult<u64>;

    /// Retrieve block header reference (number and hash) by block number.
    async fn get_block_by_number(&self, number: u64) -> LogrixResult<Option<BlockRef>>;

    /// Fetch filtered event logs for a given block range and contract addresses.
    async fn fetch_logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &[Address],
    ) -> LogrixResult<Vec<EventLog>>;

    /// Fetch full block envelope including parent hash and logs by block number.
    async fn fetch_block_envelope(
        &self,
        number: u64,
        addresses: &[Address],
    ) -> LogrixResult<Option<BlockEnvelope>>;
}
