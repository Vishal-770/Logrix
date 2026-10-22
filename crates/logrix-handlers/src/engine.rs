use crate::declarative::DeclarativeMapper;
use crate::manifest::Manifest;
use crate::staging::{EmittedEntity, TransactionalStagingBuffer};
use crate::wasm::{WasmHandlerInstance, WasmRuntime};
use alloy_primitives::Address;
use logrix_core::domain::EventLog;
use logrix_core::error::LogrixResult;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Combined orchestration engine executing declarative rules and WASM handlers.
#[derive(Clone)]
pub struct UserLogicEngine {
    pub(crate) manifest: Manifest,
    pub(crate) declarative: DeclarativeMapper,
    pub(crate) wasm_runtime: WasmRuntime,
    pub(crate) wasm_handlers: HashMap<Address, WasmHandlerInstance>,
    /// Global key-value state store backing host db_get/set across blocks
    pub(crate) state_store: Arc<RwLock<HashMap<String, String>>>,
    pub(crate) global_fuel_limit: Option<u64>,
    pub(crate) global_max_memory_mb: Option<u32>,
}

impl UserLogicEngine {
    pub fn new(manifest: Manifest) -> LogrixResult<Self> {
        let wasm_runtime = WasmRuntime::new()?;
        Ok(Self {
            manifest,
            declarative: DeclarativeMapper::new(),
            wasm_runtime,
            wasm_handlers: HashMap::new(),
            state_store: Arc::new(RwLock::new(HashMap::new())),
            global_fuel_limit: None,
            global_max_memory_mb: None,
        })
    }

    /// Set explicit global WASM limits (e.g. from CLI flags).
    pub fn with_global_limits(mut self, fuel_limit: u64, max_memory_mb: u32) -> Self {
        self.global_fuel_limit = Some(fuel_limit);
        self.global_max_memory_mb = Some(max_memory_mb);
        self
    }

    /// Access parsed manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Load and register a compiled WASM handler for a specific contract.
    pub fn load_wasm_handler(&mut self, contract: Address, wasm_bytes: &[u8]) -> LogrixResult<()> {
        let module = self.wasm_runtime.compile_module(wasm_bytes)?;
        let instance = WasmHandlerInstance::new(self.wasm_runtime.clone(), module);
        let instance = self.configure_instance(contract, instance);
        info!(%contract, "Registered sandboxed WASM handler for contract");
        self.wasm_handlers.insert(contract, instance);
        Ok(())
    }

    /// Access registered WASM handler instance for inspection.
    pub fn wasm_handler(&self, contract: &Address) -> Option<&WasmHandlerInstance> {
        self.wasm_handlers.get(contract)
    }

    /// Load and compile a WASM handler from a file on disk.
    pub fn load_wasm_handler_file<P: AsRef<Path>>(
        &mut self,
        contract: Address,
        path: P,
    ) -> LogrixResult<()> {
        let module = self.wasm_runtime.compile_module_file(path)?;
        let instance = WasmHandlerInstance::new(self.wasm_runtime.clone(), module);
        let instance = self.configure_instance(contract, instance);
        info!(%contract, "Registered sandboxed WASM handler from file");
        self.wasm_handlers.insert(contract, instance);
        Ok(())
    }

    /// Returns true if this log is active for indexing based on contract start_block.
    #[must_use]
    pub fn is_log_active(&self, log: &EventLog) -> bool {
        self.manifest
            .contracts
            .iter()
            .find(|c| c.address == log.address)
            .is_none_or(|c| c.is_active_for_block(log.block_number))
    }

    /// Process an incoming event log through declarative mappings and custom WASM logic.
    pub async fn process_log(&self, log: &EventLog) -> LogrixResult<TransactionalStagingBuffer> {
        let mut staging = TransactionalStagingBuffer::new();

        // 1. Fast Path: Declarative YAML mapping evaluation
        for contract in &self.manifest.contracts {
            if contract.address == log.address && contract.is_active_for_block(log.block_number) {
                let entities = self.declarative.apply_rules(&contract.declarative, log);
                for entity in entities {
                    staging.emit_entity(entity.entity_type, entity.payload);
                }
            }
        }

        // 2. Programmable Path: Sandboxed WebAssembly handler execution
        if self.is_log_active(log) {
            if let Some(handler) = self.wasm_handlers.get(&log.address) {
                let base_state = self.state_store.read().await.clone();

                let log_json = serde_json::to_string(log).map_err(|e| {
                    logrix_core::error::LogrixError::new(
                        logrix_core::error::ErrorClass::Integrity,
                        logrix_core::error::ErrorSource::Plugin,
                        format!("Failed to serialize event log for WASM handler: {e}"),
                    )
                })?;

                let handler_clone = handler.clone();
                let guest_staging = tokio::task::spawn_blocking(move || {
                    handler_clone.execute_event(&log_json, base_state)
                })
                .await
                .map_err(|e| {
                    logrix_core::error::LogrixError::new(
                        logrix_core::error::ErrorClass::Fatal,
                        logrix_core::error::ErrorSource::Plugin,
                        format!("Blocking WASM task panicked or failed: {e}"),
                    )
                })??;

                // Merge guest mutations into staging buffer
                for (k, v) in guest_staging.state_entries() {
                    staging.set_state(k.clone(), v.clone());
                }
                for key in guest_staging.removed_keys() {
                    staging.remove_state(key.clone());
                }
                for entity in guest_staging.emitted_entities() {
                    staging.emit_entity(entity.entity_type.clone(), entity.payload.clone());
                }
            }
        }

        Ok(staging)
    }

    /// Atomically commit all staged state changes into the persistent state store.
    /// Returns (state_mutations, removed_keys, emitted_entities).
    pub async fn commit_staging(
        &self,
        mut staging: TransactionalStagingBuffer,
    ) -> (HashMap<String, String>, HashSet<String>, Vec<EmittedEntity>) {
        let (state_mutations, removed_keys, emitted) = staging.drain();
        {
            let mut store = self.state_store.write().await;
            for key in &removed_keys {
                store.remove(key);
            }
            for (k, v) in &state_mutations {
                store.insert(k.clone(), v.clone());
            }
        }
        (state_mutations, removed_keys, emitted)
    }

    /// Query current value from the engine's state store.
    pub async fn get_state(&self, key: &str) -> Option<String> {
        let store = self.state_store.read().await;
        store.get(key).cloned()
    }

    /// Seed state store directly (used for pre-warming from Postgres on startup).
    pub async fn seed_state(&self, key: String, value: String) {
        let mut store = self.state_store.write().await;
        store.insert(key, value);
    }
}
