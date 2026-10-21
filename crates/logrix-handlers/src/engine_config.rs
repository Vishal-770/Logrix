use crate::engine::UserLogicEngine;
use crate::wasm::WasmHandlerInstance;
use alloy_primitives::Address;

impl UserLogicEngine {
    pub(crate) fn configure_instance(
        &self,
        contract: Address,
        mut instance: WasmHandlerInstance,
    ) -> WasmHandlerInstance {
        if let Some(fuel) = self.global_fuel_limit {
            instance = instance.with_fuel_limit(fuel);
        }
        if let Some(mb) = self.global_max_memory_mb {
            instance = instance.with_max_memory((mb as usize) * 1024 * 1024);
        }
        if let Some(ref idx) = self.manifest.indexing {
            if let Some(fuel) = idx.wasm_fuel_limit {
                instance = instance.with_fuel_limit(fuel);
            }
            if let Some(mb) = idx.wasm_max_memory_mb {
                instance = instance.with_max_memory(mb * 1024 * 1024);
            }
        }
        if let Some(c) = self
            .manifest
            .contracts
            .iter()
            .find(|c| c.address == contract)
        {
            if let Some(fuel) = c.fuel_limit {
                instance = instance.with_fuel_limit(fuel);
            }
            if let Some(mb) = c.max_memory_mb {
                instance = instance.with_max_memory(mb * 1024 * 1024);
            }
        }
        instance
    }
}
