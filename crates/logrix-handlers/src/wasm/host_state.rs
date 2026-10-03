use crate::staging::TransactionalStagingBuffer;
use std::collections::HashMap;

/// Sandbox host execution state for a single event invocation.
#[derive(Debug, Clone)]
pub struct HostState {
    /// In-memory mutations staged during this invocation
    pub staging: TransactionalStagingBuffer,
    /// Base state snapshot available for read-only db_get queries
    pub base_state: HashMap<String, String>,
    /// Maximum allowed linear memory in bytes (default 64MB)
    pub max_memory_bytes: usize,
}

impl HostState {
    pub fn new(base_state: HashMap<String, String>, max_memory_bytes: usize) -> Self {
        Self {
            staging: TransactionalStagingBuffer::new(),
            base_state,
            max_memory_bytes,
        }
    }

    /// Query state: checks in-memory staged updates first, then base snapshot.
    pub fn get_value(&self, key: &str) -> Option<&String> {
        self.staging
            .get_state(key)
            .or_else(|| self.base_state.get(key))
    }
}
