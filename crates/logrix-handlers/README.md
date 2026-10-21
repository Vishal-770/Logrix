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

---

## Configuration & Tuning

### 1. Declarative Manifest Configuration (`logrix.yaml`)

Configure contract rules, execution limits, and global handler parameters inside `logrix.yaml`:

```yaml
schema_version: "1.0.0"
chain_id: 421614

indexing:
  wasmFuelLimit: 5000000        # Global default fuel instruction limit
  wasmMaxMemoryMb: 256          # Global default WASM memory upper bound

contracts:
  - name: "UsdcToken"
    address: "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d"
    enabled: true
    fuel_limit: 10000000        # Per-contract fuel limit override
    max_memory_mb: 512          # Per-contract memory limit override
    wasm_handler: "./build/mapping.wasm"
    events:
      - "Transfer(address,address,uint256)"

  - name: "LegacyPool"
    address: "0x1111111111111111111111111111111111111111"
    enabled: false              # Temporarily disabled
```

| `logrix.yaml` Key | Type | Default Value | Required? | Description |
|---|---|---|---|---|
| `indexing.wasmFuelLimit` | `integer` | `1,000,000` | Optional | Default fuel instruction limit per execution |
| `indexing.wasmMaxMemoryMb` | `integer` | `128` MB | Optional | Default WASM linear memory ceiling |
| `contracts[].name` | `string` | N/A | Required | Human-readable contract name |
| `contracts[].address` | `string` | N/A | Required | Target smart contract Ethereum address (`0x...`) |
| `contracts[].events` | `string[]` | `[]` | Optional | Event signatures subscribed to by this contract |
| `contracts[].declarative` | `rule[]` | `[]` | Optional | Zero-code event-to-entity mapping rules |
| `contracts[].wasm_handler` | `string` | None | Optional | Path to compiled WebAssembly handler binary (`.wasm`) |
| `contracts[].enabled` | `boolean` | `true` | Optional | Toggle to pause indexing for this contract |
| `contracts[].fuel_limit` | `integer` | Global default | Optional | Per-contract fuel instruction ceiling |
| `contracts[].max_memory_mb`| `integer` | Global default | Optional | Per-contract WASM linear memory ceiling in MB |

### 2. Environment Variables & CLI Options

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `WASM_FUEL_LIMIT` | `--wasm-fuel-limit` | `1,000,000` (CLI) / `25,000,000` (Host) | Optional | Default fuel instruction budget per execution |
| `WASM_MAX_MEMORY_MB` | `--wasm-max-memory-mb` | `128` MB (CLI) / `64` MB (Host) | Optional | Linear memory allocation upper bound per instance |
| `MANIFEST_PATH` | `--manifest-path` | None | Optional | Path to YAML manifest file defining contracts |
| `SCHEMA_PATH` | `--schema-path` | None | Optional | Path to GraphQL or YAML entity schema file |

### 3. Programmatic Configuration (`WasmHandlerInstance`)

```rust
use logrix_handlers::wasm::{
    WasmHandlerInstance, WasmRuntime,
    DEFAULT_FUEL_LIMIT, DEFAULT_MAX_MEMORY_MB, DEFAULT_EPOCH_DEADLINE_TICKS,
};
use wasmtime::Module;

// Initialize WASM runtime and compile handler module
let runtime = WasmRuntime::new()?;
let module = Module::from_file(runtime.engine(), "handlers.wasm")?;

// Configure instance with custom fuel instruction limit and memory ceiling (in bytes)
let instance = WasmHandlerInstance::with_limits(
    runtime,
    module,
    DEFAULT_FUEL_LIMIT,          // default: 25,000,000 instructions
    DEFAULT_MAX_MEMORY_MB * 1024 * 1024, // default: 64 MB
);
```

