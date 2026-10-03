use crate::declarative::DeclarativeMapper;
use crate::manifest::Manifest;
use crate::staging::{EmittedEntity, TransactionalStagingBuffer};
use crate::wasm::{WasmHandlerInstance, WasmRuntime};
use alloy_primitives::Address;
use logrix_core::domain::EventLog;
use logrix_core::error::LogrixResult;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Combined orchestration engine executing declarative rules and WASM handlers.
#[derive(Clone)]
pub struct UserLogicEngine {
    manifest: Manifest,
    declarative: DeclarativeMapper,
    wasm_runtime: WasmRuntime,
    wasm_handlers: HashMap<Address, WasmHandlerInstance>,
    /// Global key-value state store backing host db_get/set across blocks
    state_store: Arc<RwLock<HashMap<String, String>>>,
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
        })
    }

    /// Access parsed manifest.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Load and register a compiled WASM handler for a specific contract.
    pub fn load_wasm_handler(&mut self, contract: Address, wasm_bytes: &[u8]) -> LogrixResult<()> {
        let module = self.wasm_runtime.compile_module(wasm_bytes)?;
        let instance = WasmHandlerInstance::new(self.wasm_runtime.clone(), module);
        info!(%contract, "Registered sandboxed WASM handler for contract");
        self.wasm_handlers.insert(contract, instance);
        Ok(())
    }

    /// Load and compile a WASM handler from a file on disk.
    pub fn load_wasm_handler_file<P: AsRef<Path>>(
        &mut self,
        contract: Address,
        path: P,
    ) -> LogrixResult<()> {
        let module = self.wasm_runtime.compile_module_file(path)?;
        let instance = WasmHandlerInstance::new(self.wasm_runtime.clone(), module);
        info!(%contract, "Registered sandboxed WASM handler from file");
        self.wasm_handlers.insert(contract, instance);
        Ok(())
    }

    /// Process an incoming event log through declarative mappings and custom WASM logic.
    pub async fn process_log(&self, log: &EventLog) -> LogrixResult<TransactionalStagingBuffer> {
        let mut staging = TransactionalStagingBuffer::new();

        // 1. Fast Path: Declarative YAML mapping evaluation
        for contract in &self.manifest.contracts {
            if contract.address == log.address {
                let entities = self.declarative.apply_rules(&contract.declarative, log);
                for entity in entities {
                    staging.emit_entity(entity.entity_type, entity.payload);
                }
            }
        }

        // 2. Programmable Path: Sandboxed WebAssembly handler execution
        if let Some(handler) = self.wasm_handlers.get(&log.address) {
            let base_state = {
                let guard = self.state_store.read().await;
                guard.clone()
            };

            let log_json = serde_json::to_string(log).unwrap_or_default();
            let guest_staging = handler.execute_event(&log_json, base_state)?;

            // Merge guest mutations into staging buffer
            for (k, v) in guest_staging.state_entries() {
                staging.set_state(k.clone(), v.clone());
            }
            for entity in guest_staging.emitted_entities() {
                staging.emit_entity(entity.entity_type.clone(), entity.payload.clone());
            }
        }

        Ok(staging)
    }

    /// Atomically commit all staged state changes into the persistent state store.
    pub async fn commit_staging(
        &self,
        mut staging: TransactionalStagingBuffer,
    ) -> (HashMap<String, String>, Vec<EmittedEntity>) {
        let (state_mutations, emitted) = staging.drain();
        {
            let mut store = self.state_store.write().await;
            for (k, v) in &state_mutations {
                store.insert(k.clone(), v.clone());
            }
        }
        (state_mutations, emitted)
    }

    /// Query current value from the engine's state store.
    pub async fn get_state(&self, key: &str) -> Option<String> {
        let store = self.state_store.read().await;
        store.get(key).cloned()
    }
}
