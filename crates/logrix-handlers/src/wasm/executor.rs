use super::host_state::HostState;
use super::instance::WasmHandlerInstance;
use crate::staging::TransactionalStagingBuffer;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use std::collections::HashMap;

impl WasmHandlerInstance {
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
        store.set_fuel(self.fuel_limit).map_err(|e| {
            LogrixError::new(
                ErrorClass::Fatal,
                ErrorSource::Plugin,
                format!("Failed to set WASM fuel limit: {e}"),
            )
        })?;
        store.set_epoch_deadline(super::instance::DEFAULT_EPOCH_DEADLINE_TICKS);

        let instance = linker.instantiate(&mut store, &self.module).map_err(|e| {
            LogrixError::new(
                ErrorClass::Fatal,
                ErrorSource::Plugin,
                format!("Failed to instantiate WASM module: {e}"),
            )
        })?;

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
            1024
        };

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
                if let Some(memory) = instance.get_memory(&mut store, "memory") {
                    let total_bytes = memory.data_size(&store);
                    if total_bytes > self.max_memory_bytes {
                        return Err(LogrixError::new(
                            ErrorClass::Fatal,
                            ErrorSource::Plugin,
                            format!("WASM execution exceeded memory limit ({total_bytes} bytes)"),
                        ));
                    }
                }
                Ok(store.into_data().staging)
            }
            Err(trap) => Err(LogrixError::new(
                ErrorClass::Fatal,
                ErrorSource::Plugin,
                format!("WASM execution trapped: {trap:#}"),
            )),
        }
    }
}
