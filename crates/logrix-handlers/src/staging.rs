use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// An entity emitted by declarative mappings or user WASM handlers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmittedEntity {
    pub entity_type: String,
    pub payload: serde_json::Value,
}

/// Transactional staging buffer holding state mutations and emitted entities.
///
/// Ensures all mutations from an event/block are buffered in memory and either
/// committed atomically or discarded cleanly on execution trap or error.
#[derive(Debug, Clone, Default)]
pub struct TransactionalStagingBuffer {
    /// In-memory key-value state mutations (e.g. running balances, counters)
    state: HashMap<String, String>,
    /// Keys explicitly removed via store.remove() -- takes priority over state writes
    removed: HashSet<String>,
    /// Structured domain entities emitted for persistence
    emitted: Vec<EmittedEntity>,
    /// Log messages collected during execution
    logs: Vec<(String, String)>,
}

impl TransactionalStagingBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read state value from staging buffer. Returns None for removed keys.
    pub fn get_state(&self, key: &str) -> Option<&String> {
        if self.removed.contains(key) {
            return None;
        }
        self.state.get(key)
    }

    /// Stage a state key-value update; clears any pending removal for that key.
    pub fn set_state(&mut self, key: String, value: String) {
        self.removed.remove(&key);
        self.state.insert(key, value);
    }

    /// Stage a state key removal. Supersedes any pending set for the same key.
    pub fn remove_state(&mut self, key: String) {
        self.state.remove(&key);
        self.removed.insert(key);
    }

    /// Stage an emitted entity.
    pub fn emit_entity(&mut self, entity_type: String, payload: serde_json::Value) {
        self.emitted.push(EmittedEntity {
            entity_type,
            payload,
        });
    }

    /// Record a diagnostic log message.
    pub fn log(&mut self, level: String, message: String) {
        self.logs.push((level, message));
    }

    /// Take all pending mutations and clear the buffer (atomic commit).
    /// Returns (state_mutations, removed_keys, emitted_entities).
    pub fn drain(&mut self) -> (HashMap<String, String>, HashSet<String>, Vec<EmittedEntity>) {
        let state = std::mem::take(&mut self.state);
        let removed = std::mem::take(&mut self.removed);
        let emitted = std::mem::take(&mut self.emitted);
        self.logs.clear();
        (state, removed, emitted)
    }

    /// Discard all pending mutations (rollback).
    pub fn rollback(&mut self) {
        self.state.clear();
        self.removed.clear();
        self.emitted.clear();
        self.logs.clear();
    }

    /// Returns the combined number of staged state entries and emitted entities.
    pub fn len(&self) -> usize {
        self.state.len() + self.emitted.len()
    }

    pub fn is_empty(&self) -> bool {
        self.state.is_empty() && self.emitted.is_empty()
    }

    /// Access emitted entities.
    pub fn emitted_entities(&self) -> &[EmittedEntity] {
        &self.emitted
    }

    /// Access staged state entries.
    pub fn state_entries(&self) -> &HashMap<String, String> {
        &self.state
    }

    /// Access keys staged for removal.
    pub fn removed_keys(&self) -> &HashSet<String> {
        &self.removed
    }
}
