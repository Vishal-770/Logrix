use super::runtime::WasmRuntime;
use wasmtime::Module;

/// Default WASM execution instruction fuel budget (25,000,000 instructions).
pub const DEFAULT_FUEL_LIMIT: u64 = 25_000_000;

/// Default WASM guest linear memory ceiling (64 megabytes).
pub const DEFAULT_MAX_MEMORY_MB: usize = 64;

/// Default WASM epoch interruption deadline (100 ticks).
pub const DEFAULT_EPOCH_DEADLINE_TICKS: u64 = 100;

/// Configured, executable instance of a user-defined WASM event handler.
#[derive(Clone)]
pub struct WasmHandlerInstance {
    pub(crate) runtime: WasmRuntime,
    pub(crate) module: Module,
    pub(crate) fuel_limit: u64,
    pub(crate) max_memory_bytes: usize,
}

impl WasmHandlerInstance {
    pub fn new(runtime: WasmRuntime, module: Module) -> Self {
        let fuel_limit = std::env::var("WASM_FUEL_LIMIT")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_FUEL_LIMIT);
        let max_memory_mb = std::env::var("WASM_MAX_MEMORY_MB")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MAX_MEMORY_MB);
        Self::with_limits(runtime, module, fuel_limit, max_memory_mb * 1024 * 1024)
    }

    pub fn with_limits(
        runtime: WasmRuntime,
        module: Module,
        fuel_limit: u64,
        max_memory_bytes: usize,
    ) -> Self {
        Self {
            runtime,
            module,
            fuel_limit,
            max_memory_bytes,
        }
    }

    pub fn with_fuel_limit(mut self, fuel_limit: u64) -> Self {
        self.fuel_limit = fuel_limit;
        self
    }

    pub fn with_max_memory(mut self, max_memory_bytes: usize) -> Self {
        self.max_memory_bytes = max_memory_bytes;
        self
    }

    pub fn fuel_limit(&self) -> u64 {
        self.fuel_limit
    }

    pub fn max_memory_bytes(&self) -> usize {
        self.max_memory_bytes
    }
}
