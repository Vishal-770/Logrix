//! User Logic Engine for Logrix: Declarative YAML event mappings and
//! sandboxed TypeScript/AssemblyScript WebAssembly handlers via Wasmtime.

pub mod declarative;
pub mod engine;
pub mod manifest;
pub mod sdk_manifest;
pub mod staging;
pub mod wasm;

mod engine_config;
mod manifest_env;

pub use declarative::DeclarativeMapper;
pub use engine::UserLogicEngine;
pub use manifest::{
    ContractManifest, DeclarativeRule, Manifest, ManifestIndexing, ManifestWebhook,
};
pub use staging::{EmittedEntity, TransactionalStagingBuffer};
pub use wasm::{
    HostState, WasmHandlerInstance, WasmRuntime, DEFAULT_EPOCH_DEADLINE_TICKS, DEFAULT_FUEL_LIMIT,
    DEFAULT_MAX_MEMORY_MB,
};
