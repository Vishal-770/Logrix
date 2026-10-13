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

Configuration is supported across two levels: global runtime defaults via environment variables / CLI flags, and granular per-contract overrides inside `logrix.yaml`.

### Global Runtime Configuration

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `WASM_FUEL_LIMIT` | `--wasm-fuel-limit` | `1,000,000` (CLI) / `25,000,000` (WASM host) | Optional | Default fuel instruction budget per contract event execution |
| `WASM_MAX_MEMORY_MB` | `--wasm-max-memory-mb` | `128` MB (CLI) / `64` MB (WASM host) | Optional | Linear memory allocation upper bound per WASM instance |
| `MANIFEST_PATH` | `--manifest-path` | None | Optional | Path to YAML manifest file defining contracts and handlers |
| `SCHEMA_PATH` | `--schema-path` | None | Optional | Path to GraphQL or YAML entity schema file |

### Manifest Contract Configuration (`logrix.yaml`)

Every contract declared in `contracts:` can override resource limits and execution state:

| Field | Type | Default Value | Required? | Description |
|---|---|---|---|---|
| `name` | `string` | N/A | Required | Human-readable contract name |
| `address` | `string` | N/A | Required | Target smart contract Ethereum address (`0x...`) |
| `events` | `string[]` | `[]` | Optional | Event signatures subscribed to by this contract |
| `declarative` | `rule[]` | `[]` | Optional | Zero-code event-to-entity mapping rules |
| `wasm_handler` | `string` | None | Optional | Path to compiled WebAssembly handler binary (`.wasm`) |
| `enabled` | `boolean` | `true` | Optional | Toggle to pause indexing for this contract without removing config |
| `fuel_limit` | `integer` | Global default | Optional | Per-contract fuel instruction ceiling |
| `max_memory_mb` | `integer` | Global default | Optional | Per-contract WASM linear memory ceiling in megabytes |

### Manifest Example

```yaml
schema_version: "1.0.0"
chain_id: 421614

contracts:
  - name: "UsdcToken"
    address: "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d"
    enabled: true
    fuel_limit: 5000000
    max_memory_mb: 256
    wasm_handler: "./build/mapping.wasm"
    events:
      - "Transfer(address,address,uint256)"

  - name: "LegacyPool"
    address: "0x1111111111111111111111111111111111111111"
    enabled: false # Temporarily disabled
```
