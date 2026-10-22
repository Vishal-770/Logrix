use alloy_primitives::Address;
use async_trait::async_trait;
use logrix_core::{
    domain::{BlockEnvelope, BlockRef, ChainId, EventLog},
    error::{ErrorSource, LogrixError, LogrixResult},
    ports::ChainPort,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Mock ChainPort allowing scripted block headers, event logs, latency, and fault injection.
#[derive(Debug, Clone, Default)]
pub struct MockChainPort {
    pub chain_id: ChainId,
    pub head_block: Arc<Mutex<u64>>,
    blocks: Arc<Mutex<HashMap<u64, BlockEnvelope>>>,
    injected_errors: Arc<AtomicUsize>,
    simulated_latency: Arc<Mutex<Option<Duration>>>,
}

impl MockChainPort {
    pub fn new(chain_id: ChainId) -> Self {
        Self {
            chain_id,
            head_block: Arc::new(Mutex::new(0)),
            blocks: Arc::new(Mutex::new(HashMap::new())),
            injected_errors: Arc::new(AtomicUsize::new(0)),
            simulated_latency: Arc::new(Mutex::new(None)),
        }
    }

    /// Configures artificial latency for all RPC calls.
    pub async fn set_simulated_latency(&self, latency: Duration) {
        *self.simulated_latency.lock().await = Some(latency);
    }

    /// Injects `count` transient errors for subsequent RPC queries.
    pub fn inject_transient_errors(&self, count: usize) {
        self.injected_errors.store(count, Ordering::SeqCst);
    }

    async fn apply_latency_and_check_error(&self) -> LogrixResult<()> {
        let delay = *self.simulated_latency.lock().await;
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
        let mut curr = self.injected_errors.load(Ordering::SeqCst);
        while curr > 0 {
            match self.injected_errors.compare_exchange_weak(
                curr,
                curr - 1,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    return Err(LogrixError::transient(
                        ErrorSource::ChainRpc,
                        "Simulated transient RPC network drop",
                    ));
                }
                Err(actual) => curr = actual,
            }
        }
        Ok(())
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
    fn chain_id(&self) -> ChainId {
        self.chain_id
    }

    async fn get_latest_block_number(&self) -> LogrixResult<u64> {
        self.apply_latency_and_check_error().await?;
        Ok(*self.head_block.lock().await)
    }

    async fn get_block_by_number(&self, number: u64) -> LogrixResult<Option<BlockRef>> {
        self.apply_latency_and_check_error().await?;
        let blocks = self.blocks.lock().await;
        Ok(blocks.get(&number).map(|b| b.block_ref()))
    }

    async fn fetch_logs(
        &self,
        from_block: u64,
        to_block: u64,
        addresses: &[Address],
    ) -> LogrixResult<Vec<EventLog>> {
        self.apply_latency_and_check_error().await?;
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
        self.apply_latency_and_check_error().await?;
        let blocks = self.blocks.lock().await;
        Ok(blocks.get(&number).cloned())
    }
}
