# logrix-handlers

Wasmtime WebAssembly execution runtime and declarative event handlers for Logrix indexers.

## Overview
Executes user-defined indexing logic via a two-tier architecture:
1. **Declarative Fast-Path**: High-performance YAML mapping converting event logs, topics, and calldata slices directly into structured entities with zero WASM VM overhead.
2. **Programmable Sandboxed WASM**: Executes AssemblyScript/Rust WASM modules in an isolated, fuel-metered, and memory-bounded Wasmtime engine.

## Features
- **Declarative Topic & Event Filtering**: Matches rules strictly against event names or `topic0` signatures, supporting wildcard mappings and `.as_u256` big-endian numeric decimal conversion (zero floating-point truncation).
- **User-Centric Customization**: Granular per-contract toggles (`enabled: bool`, `fuel_limit: Option<u64>`, `max_memory_mb: Option<usize>`) allow enabling/disabling contracts dynamically and tuning resource limits per module.
- **Strict Fuel Metering & Memory Ceilings**: Enforces deterministic instruction limits and linear memory ceilings to protect host nodes against infinite loops and memory leaks.
- **Transactional Staging Buffer**: Buffers entity emissions and key-value state mutations in memory, committing them atomically or discarding them cleanly upon guest traps or panics.
- **Guest Linear Memory Isolation**: Modular `memory.rs` utilities ensure memory-safe UTF-8 string reading and slice copying without buffer overrun vulnerabilities.
