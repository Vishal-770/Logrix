//! User Logic Engine for Logrix: Declarative YAML event mappings and
//! sandboxed TypeScript/AssemblyScript WebAssembly handlers via Wasmtime.

pub mod declarative;
pub mod engine;
pub mod manifest;
pub mod staging;
pub mod wasm;

pub use declarative::DeclarativeMapper;
pub use engine::UserLogicEngine;
pub use manifest::{ContractManifest, DeclarativeRule, Manifest, ManifestWebhook};
pub use staging::{EmittedEntity, TransactionalStagingBuffer};
pub use wasm::{HostState, WasmHandlerInstance, WasmRuntime};
