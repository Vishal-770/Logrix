# logrix-handlers

Wasmtime WebAssembly execution runtime and declarative event handlers for Logrix indexers.

## Overview
Executes user-defined AssemblyScript/WASM mapping logic in a sandboxed, fuel-bounded virtual machine.

## Features
- **Fuel Metering**: Enforces strict execution limits to protect the host against infinite loops and malicious scripts.
- **Transactional State Rollback**: State mutations staged in `WasmStateStore` are atomically committed or rolled back if the handler panics.
- **State Store Pre-Warming**: Pre-loads committed entity state on boot so `Entity.load(id)` works immediately without cold starts.
- **AssemblyScript Host Exports**: Full support for `abort` panic logging, cryptographic hashing, and entity persistence.
