# logrix-handlers

Manifest parsing, declarative log-to-entity rules, and Wasmtime guest execution.
`UserLogicEngine` returns staged state mutations and entities; persistence is the
caller's responsibility.

## Prerequisites and minimal usage

Use the workspace Rust toolchain and cached locked dependencies. Embedding needs
Tokio for engine state locks; WASM execution needs a compiled guest matching the
ABI below. Declarative-only operation requires no external services. The CLI
processor additionally needs its database, queue, and RPC adapters.

In an async Rust application depending on `logrix-core` and `logrix-handlers`:

```rust
use logrix_core::{domain::EventLog, error::LogrixResult};
use logrix_handlers::{Manifest, UserLogicEngine};

async fn map_log(yaml: &str, log: &EventLog) -> LogrixResult<usize> {
    let manifest = Manifest::from_yaml_str(yaml)?;
    let engine = UserLogicEngine::new(manifest)?;
    // Declarative-only example. WASM requires a mutable engine and file loading.
    let staging = engine.process_log(log).await?;
    let (_, _, entities) = engine.commit_staging(staging).await;
    // This commits only engine memory. Persist entities in the caller.
    Ok(entities.len())
}
```

Keep one engine across events when retaining state. Creating an engine does not
load `wasm_handler` files automatically; the CLI processor loads them at startup.

## Native runtime manifest

This complete manifest uses a topic hash to match ERC-20 Transfer logs:

```yaml
schema_version: "1"
chain_id: 421614
indexing:
  wasmFuelLimit: 5000000
  wasmMaxMemoryMb: 64
contracts:
  - name: "Token"
    address: "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d"
    enabled: true
    wasm_handler: null
    declarative:
      - event: "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
        entity: "TransferLog"
        fields:
          id: "log.tx_hash"
          blockNumber: "log.block_number"
          amount: "log.data[0..32].as_u256"
```

`id: log.tx_hash` is only suitable if each transaction emits one relevant log;
the mapper cannot concatenate transaction hash and log index for a unique ID.
Use WASM for a composite ID when multiple logs can occur.

| `logrix.yaml` Key | Type | Default Value | Required? | Description |
|---|---|---|---|---|
| `schema_version`, `chain_id`, `contracts` | string, integer, array | None | Required | Parsed metadata and contracts; version string is not validated |
| `indexing.wasmFuelLimit` | integer | Unset | Optional | Overrides global fuel setting when loading a handler |
| `indexing.wasmMaxMemoryMb` | integer | Unset | Optional | Overrides memory check threshold, in units of 1024 * 1024 bytes |
| `contracts[].name` | `string` | N/A | Required | Human-readable contract name |
| `contracts[].address` | `string` | N/A | Required | Target smart contract Ethereum address (`0x...`) |
| `contracts[].events` | `string[]` | `[]` | Optional | Retained metadata; not an execution filter |
| `contracts[].declarative` | `rule[]` | `[]` | Optional | Zero-code event-to-entity mapping rules |
| `contracts[].wasm_handler` | `string` | None | Optional | Path to compiled WebAssembly handler binary (`.wasm`) |
| `contracts[].enabled` | `boolean` | `true` | Optional | Skips engine logic; does not remove address from CLI RPC queries |
| `contracts[].fuel_limit` | `integer` | Inherited | Optional | Per-contract Wasmtime fuel budget |
| `contracts[].max_memory_mb`| `integer` | Inherited | Optional | Per-contract linear-memory check threshold in MiB, not an allocation cap |

Each declarative rule requires `event`, `entity`, and a `fields` string map.
Native manifests can also have `webhooks` (default `[]`): entries require `url`,
and accept `secret`, `events` (default `[]`), and `maxRetries`/`max_retries`
(unset). Only the dedicated CLI webhook runner seeds these into PostgreSQL;
missing retry count there becomes zero additional retries.

## Direct SDK project-manifest subset

`Manifest::from_yaml_str` also accepts this shape (the WASM file must exist when
the caller loads it):

```yaml
specVersion: "0.1.0"
network:
  chainId: 421614
dataSources:
  - name: "Token"
    source:
      address: "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d"
    mapping:
      file: "./build/mapping.wasm"
      enabled: true
      fuelLimit: 5000000
      maxMemoryMb: 64
      eventHandlers:
        - event: "Transfer(address,address,uint256)"
```

The Rust adapter consumes `specVersion`, `network.chainId`, data-source `name`,
`source.address`, and the shown mapping fields. `mapping` and `source` are
required; `mapping.file` defaults to `./build/mapping.wasm`, event handlers to
`[]`, and data sources to `[]`. `webhooks` and `indexing` use the same structures
as native manifests. Each converted contract has an empty declarative rule list.

Unknown fields are ignored. In particular, direct Rust loading does not use SDK
`network.rpcUrl`, fallback URLs, `source.startBlock`, ABI declarations, schema
paths, or named `eventHandlers[].handler` dispatch. `events` is metadata; WASM
runs for every log at an enabled registered address, so guests must filter logs.
Native `start_block` is also ignored. Paths are relative to the process working
directory, not rebased to the manifest directory.

Both formats recognize optional `indexing` keys `ringBufferDepth`,
`reconcilerIntervalSecs`, `reconcilerChunkSize`, `reconcilerLagThreshold`,
`chunkSize`, `chainHeadPollMs`, `maxQueueDepth`, `wasmFuelLimit`, and
`wasmMaxMemoryMb`, with snake_case aliases. Parsing a key does not imply a CLI
consumer: the ingester applies only chunk size, queue depth, and head poll;
the engine applies only the WASM keys. Reconciler keys are not applied directly
by the processor. RPC, database, queue, GraphQL, and webhook timeout indexing
keys are not part of this Rust structure.

SDK `export-values` is separate: its TypeScript converter maps operational keys
to Helm values/environment, chooses a global minimum start block, and writes a
native manifest pointing at `/etc/logrix/handlers/mapping.wasm`. It does not
copy the SDK `indexing` block into that native manifest. Its webhook defaults
also differ (`["reorg"]` and three retries when omitted). See
[configuration](../../docs/configuration.md) and [converter source](../../packages/logrix-sdk/src/cli/manifest_converter.ts).

## Execution limits, ABI, and persistence boundaries

- `WasmHandlerInstance::new` reads `WASM_FUEL_LIMIT` (default 25,000,000 fuel
  units) and `WASM_MAX_MEMORY_MB` (default 64 MiB); invalid values fall back.
  `with_limits` takes fuel and bytes explicitly. `with_fuel_limit` and
  `with_max_memory` update them. Fuel is Wasmtime metering, not a wall-clock SLA.
- Engine load-time precedence, highest first: per-contract limits, manifest
  indexing limits, `with_global_limits`, instance environment/defaults. The CLI
  supplies global limits of 1,000,000 fuel and 128 MiB via its flags/environment.
  Thus a manifest can override an explicit CLI fuel/memory flag.
- Memory is checked after instantiation and after a successful event call.
  There is no Wasmtime resource limiter restricting `memory.grow`, and the first
  check is after allocation. These settings are checks, not hard allocation or
  process-memory ceilings. Host state/staging are not bounded; the engine clones
  its full state for each WASM invocation. Epoch interruption is enabled, but
  this crate does not drive an epoch timer.
- Guests export `handle_event(i32, i32) -> i32`, normally `memory`, and optionally
  `allocate(i32) -> i32` or `alloc(i32) -> i32`. Without an allocator, the input
  offset is 1024. The input is raw `EventLog` JSON, not ABI-decoded parameters.
  Nonnegative returns succeed; negative returns are transient plugin errors.
- Host imports under `logrix` and `env` are `logrix_db_get`, `logrix_db_set`,
  `logrix_db_remove`, `logrix_emit`, and `logrix_log`; see
  [host signatures](src/wasm/host_funcs.rs). No WASI/network access is registered.
- Declarative matching accepts empty/`*` patterns and literal topic0 hashes
  (case-insensitive, optional `0x`). Bare `Transfer` and `Approval` currently
  match any log, irrespective of topic. Arbitrary event signatures are not
  resolved or hashed by the mapper.
- Expressions: `log.address`, `log.block_number`, `log.tx_hash`, `log.log_index`,
  `log.data`, `log.topics[n]`, and `log.data[start..end]`. Topic/data selections
  accept `.as_u256` and produce decimal strings; raw selections produce hex.
  Unknown expressions become literal strings; invalid selections produce null.
  An over-32-byte U256 slice falls back to zero.
- Failed guest execution returns no staging; successful staging still needs an
  explicit commit. `commit_staging` updates only the engine's in-memory map and
  returns mutations/removals/entities. It is not a database transaction or reorg
  undo log. The CLI commits memory before saving emitted entities, performs
  separate database writes/checkpoint updates, and does not persist arbitrary
  state mutations. Startup warmup seeds entity JSON, not all previous state.

## Errors and tests

`Manifest::from_file` reports read failures as transient configuration errors;
invalid YAML/known-field types and missing/invalid `${VAR}` placeholders are
permanent configuration errors. Placeholder expansion visits YAML string values
before deserialization, including ignored fields. WASM compile/missing-export
errors are permanent plugin errors; file reads are transient; instantiation,
fuel traps, and memory-check failures are fatal plugin errors. Unknown manifest
keys are not validation errors. Declarative mapping has no typed validation
error channel.

From the workspace root with cached dependencies:

```sh
cargo test -p logrix-handlers --offline --locked
```

See [tests](tests/) for manifest conversion/placeholders, load-time limit
precedence, declarative extraction, and WASM staging/traps. Related:
[CLI](../logrix-cli/README.md), [API schema](../logrix-api/README.md),
[webhooks](../logrix-webhook/README.md), and [configuration](../../docs/configuration.md).
