use super::host_state::HostState;
use super::runtime::WasmRuntime;
use crate::staging::TransactionalStagingBuffer;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use std::collections::HashMap;
use wasmtime::Module;

/// Configured, executable instance of a user-defined WASM event handler.
#[derive(Clone)]
pub struct WasmHandlerInstance {
    runtime: WasmRuntime,
    module: Module,
    fuel_limit: u64,
    max_memory_bytes: usize,
}

impl WasmHandlerInstance {
    pub fn new(runtime: WasmRuntime, module: Module) -> Self {
        Self {
            runtime,
            module,
            fuel_limit: 1_000_000,              // 1M fuel ticks per event
            max_memory_bytes: 64 * 1024 * 1024, // 64 MB linear memory ceiling
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

    /// Execute the guest `handle_event(ptr, len)` handler for an event log JSON payload.
    ///
    /// Runs inside a sandboxed transaction: if the guest traps (e.g. out of fuel,
    /// memory ceiling exceeded, panic), all staged writes are discarded and an error is returned.
    pub fn execute_event(
        &self,
        event_json: &str,
        base_state: HashMap<String, String>,
    ) -> LogrixResult<TransactionalStagingBuffer> {
        let linker = self.runtime.new_linker()?;
        let host_state = HostState::new(base_state, self.max_memory_bytes);
        let mut store = wasmtime::Store::new(self.runtime.engine(), host_state);

        // Metering: Assign deterministic fuel ticks
        store.set_fuel(self.fuel_limit).map_err(|e| {
            LogrixError::new(
                ErrorClass::Fatal,
                ErrorSource::Plugin,
                format!("Failed to set WASM fuel limit: {e}"),
            )
        })?;

        // Epoch deadline
        store.set_epoch_deadline(100);

        // Instantiate
        let instance = linker.instantiate(&mut store, &self.module).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Plugin,
                format!("Failed to instantiate WASM module: {e}"),
            )
        })?;

        // Verify linear memory limit ceiling
        if let Some(memory) = instance.get_memory(&mut store, "memory") {
            let current_bytes = memory.data_size(&store);
            if current_bytes > self.max_memory_bytes {
                return Err(LogrixError::new(
                    ErrorClass::Fatal,
                    ErrorSource::Plugin,
                    format!(
                        "WASM initial memory ({current_bytes} bytes) exceeds limit ({})",
                        self.max_memory_bytes
                    ),
                ));
            }
        }

        // Allocate memory buffer in guest for event payload
        let payload_bytes = event_json.as_bytes();
        let ptr = if let Ok(alloc_fn) = instance.get_typed_func::<i32, i32>(&mut store, "allocate")
        {
            alloc_fn
                .call(&mut store, payload_bytes.len() as i32)
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Fatal,
                        ErrorSource::Plugin,
                        format!("WASM allocate call trapped: {e}"),
                    )
                })?
        } else if let Ok(alloc_fn) = instance.get_typed_func::<i32, i32>(&mut store, "alloc") {
            alloc_fn
                .call(&mut store, payload_bytes.len() as i32)
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Fatal,
                        ErrorSource::Plugin,
                        format!("WASM alloc call trapped: {e}"),
                    )
                })?
        } else {
            // Default offset in linear memory when static allocation is used
            1024
        };

        // Write input payload into guest memory
        if let Some(memory) = instance.get_memory(&mut store, "memory") {
            let data = memory.data_mut(&mut store);
            let start = ptr as usize;
            let end = start + payload_bytes.len();
            if end <= data.len() {
                data[start..end].copy_from_slice(payload_bytes);
            } else {
                return Err(LogrixError::new(
                    ErrorClass::Fatal,
                    ErrorSource::Plugin,
                    "Payload exceeds guest linear memory size",
                ));
            }
        }

        // Execute guest handler: handle_event(ptr: i32, len: i32) -> i32
        let handle_fn = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "handle_event")
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Permanent,
                    ErrorSource::Plugin,
                    format!("Missing exported 'handle_event(i32, i32) -> i32' in WASM module: {e}"),
                )
            })?;

        let res = handle_fn.call(&mut store, (ptr, payload_bytes.len() as i32));

        match res {
            Ok(ret_code) => {
                if ret_code < 0 {
                    return Err(LogrixError::new(
                        ErrorClass::Transient,
                        ErrorSource::Plugin,
                        format!("Handler returned error code {ret_code}"),
                    ));
                }
                // Check memory ceiling after execution
                if let Some(memory) = instance.get_memory(&mut store, "memory") {
                    let total_bytes = memory.data_size(&store);
                    if total_bytes > self.max_memory_bytes {
                        return Err(LogrixError::new(
                            ErrorClass::Fatal,
                            ErrorSource::Plugin,
                            format!(
                                "WASM execution exceeded 64MB memory limit ({total_bytes} bytes)"
                            ),
                        ));
                    }
                }
                // Success: extract transactional staging buffer
                Ok(store.into_data().staging)
            }
            Err(trap) => {
                // Trap (Out of fuel, panic, unreachable) -> state discarded
                Err(LogrixError::new(
                    ErrorClass::Fatal,
                    ErrorSource::Plugin,
                    format!("WASM execution trapped: {trap:#}"),
                ))
            }
        }
    }
}
