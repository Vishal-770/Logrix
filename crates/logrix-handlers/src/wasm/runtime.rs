use super::host_funcs::register_host_functions;
use super::host_state::HostState;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use std::path::Path;
use wasmtime::{Config, Engine, Linker, Module, OptLevel};

/// Sandboxed WebAssembly runtime managing JIT compilation, fuel metering, and host links.
#[derive(Clone)]
pub struct WasmRuntime {
    engine: Engine,
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create default WasmRuntime")
    }
}

impl WasmRuntime {
    /// Initialize a new Wasmtime engine configured for fuel metering and memory bounds.
    pub fn new() -> LogrixResult<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        config.cranelift_opt_level(OptLevel::Speed);

        let engine = Engine::new(&config).map_err(|e| {
            LogrixError::new(
                ErrorClass::Fatal,
                ErrorSource::Plugin,
                format!("Failed to initialize Wasmtime Engine: {e}"),
            )
        })?;

        Ok(Self { engine })
    }

    /// Access the underlying Wasmtime engine.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Create a Linker populated with Logrix standard host functions.
    pub fn new_linker(&self) -> LogrixResult<Linker<HostState>> {
        let mut linker = Linker::new(&self.engine);
        register_host_functions(&mut linker).map_err(|e| {
            LogrixError::new(
                ErrorClass::Fatal,
                ErrorSource::Plugin,
                format!("Failed to link host functions: {e}"),
            )
        })?;
        Ok(linker)
    }

    /// Compile raw WASM bytecode into an executable Module.
    pub fn compile_module(&self, wasm_bytes: &[u8]) -> LogrixResult<Module> {
        Module::new(&self.engine, wasm_bytes).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Plugin,
                format!("Failed to compile WASM module: {e}"),
            )
        })
    }

    /// Read and compile a WASM file from the filesystem.
    pub fn compile_module_file<P: AsRef<Path>>(&self, path: P) -> LogrixResult<Module> {
        let bytes = std::fs::read(path.as_ref()).map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Plugin,
                format!("Failed to read WASM binary {:?}: {e}", path.as_ref()),
            )
        })?;
        self.compile_module(&bytes)
    }
}
