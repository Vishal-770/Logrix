use alloy_primitives::Address;
use async_trait::async_trait;
use logrix_core::{
    domain::{BlockEnvelope, BlockRef, ChainId, EventLog},
    error::LogrixResult,
    ports::ChainPort,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mock ChainPort allowing scripted block headers and event logs.
#[derive(Debug, Clone, Default)]
pub struct MockChainPort {
    pub chain_id: ChainId,
    pub head_block: Arc<Mutex<u64>>,
    blocks: Arc<Mutex<HashMap<u64, BlockEnvelope>>>,
}

impl MockChainPort {
    pub fn new(chain_id: ChainId) -> Self {
        Self {
            chain_id,
            head_block: Arc::new(Mutex::new(0)),
            blocks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn add_block(&self, envelope: BlockEnvelope) {
        let mut blocks = self.blocks.lock().await;
        let mut head = self.head_block.lock().await;
        if envelope.block_number > *head {
            *head = envelope.block_number;
        }
        blocks.insert(envelope.block_number, envelope);
    }
}

#[async_trait]
impl ChainPort for MockChainPort {
    async fn get_latest_block_number(&self) -> LogrixResult<u64> {
        Ok(*self.head_block.lock().await)
    }

    async fn get_block_by_number(&self, number: u64) -> LogrixResult<Option<BlockRef>> {
        let blocks = self.blocks.lock().await;
        Ok(blocks.get(&number).map(|b| b.block_ref()))
    }

    async fn fetch_logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &[Address],
    ) -> LogrixResult<Vec<EventLog>> {
        let blocks = self.blocks.lock().await;
        let mut logs = Vec::new();
        for block_num in from_block..=to_block {
            if let Some(block) = blocks.get(&block_num) {
                for log in &block.logs {
                    if addresses.is_empty() || addresses.contains(&log.address) {
                        logs.push(log.clone());
                    }
                }
            }
        }
        Ok(logs)
    }

    async fn fetch_block_envelope(
        &self,
        number: u64,
        _addresses: &[Address],
    ) -> LogrixResult<Option<BlockEnvelope>> {
        let blocks = self.blocks.lock().await;
        Ok(blocks.get(&number).cloned())
    }
}
