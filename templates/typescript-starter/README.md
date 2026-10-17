# Low-level AssemblyScript starter

This template demonstrates the raw Logrix guest ABI for Arbitrum Sepolia (chain ID `421614`). Despite the folder name, `assembly/index.ts` is **AssemblyScript**, not ordinary Node.js/browser TypeScript. `asc` compiles it to WebAssembly; `i32`, `i64`, `usize`, `heap.alloc`, and `@external` belong to the guest language.

This is a separate, low-level template from `logrix init`. It has no `@logrix/sdk` dependency, SDK `logrix.yaml`, GraphQL schema, generated entity bindings, or CLI codegen/export scripts.

## Files and actual scripts

```text
assembly/index.ts    raw host declarations and demonstration handler
manifest.yaml        native Rust runtime manifest (contracts/wasm_handler)
asconfig.json        debug/release compiler settings, raw bindings
package.json         AssemblyScript ^0.27.34 build scripts
```

| Command | Behavior |
| --- | --- |
| `npm install` | Installs the declared compiler locally; this template currently has no lockfile |
| `npm run asbuild:debug` | Compiles `assembly/index.ts` with the debug target to `build/handler.wasm` |
| `npm run asbuild:release` | Compiles with the release target to the same path |
| `npm run build` | Alias for `asbuild:release` |

The config also requests `build/handler.wat`, raw bindings, and debug source maps for the debug target. There are no `test`, `validate`, `codegen`, or `export-values` scripts. Once dependencies are deliberately resolved, keep the resulting lockfile for reproducible builds.

### Current compilation blocker

The checked-in source does **not** currently compile with AssemblyScript 0.27.x: both calls to `String.UTF8.decode` pass pointer/length arguments, whereas that API expects an `ArrayBuffer` and an optional boolean. The pointer-based API is `String.UTF8.decodeUnsafe(ptr, len)`. Correct those two calls (or copy into buffers) before expecting the build scripts to succeed. This README documents the existing template; a working binary is not supplied here.

## Guest/host contract

The intended WASM module exports `memory`, `allocate(size: usize): usize`, and `handle_event(ptr: usize, len: usize): i32`. On wasm32, `usize` maps to `i32`. The Rust host writes a UTF-8 JSON EVM log into allocated linear memory, then calls `handle_event`. Zero is success; negative results and traps fail that invocation and discard staged guest changes. There is no exported per-event `handleTransfer` API.

Unlike SDK-generated mappings, this template declares imports directly under **`logrix`**:

| Import | Arguments and result (all wasm32 `i32`) |
| --- | --- |
| `logrix_db_get` | `(key_ptr, key_len, out_ptr, max_len) -> bytes_written`; `-1` missing/invalid input, `-2` buffer too small |
| `logrix_db_set` | `(key_ptr, key_len, value_ptr, value_len) -> status` |
| `logrix_emit` | `(entity_ptr, entity_len, payload_ptr, payload_len) -> status` |
| `logrix_log` | `(level, message_ptr, message_len) -> status` |

The current Rust linker registers these under both `logrix` and `env`, also supplies `logrix_db_remove`, and supplies AssemblyScript `env.abort`. SDK wrappers normally use `env`; this starter does not import them. These are staged state/entity operations, not arbitrary SQL or guest network/filesystem access. The host engine currently drops staged diagnostic logs rather than forwarding them with the processing result.

## What the demonstration actually indexes

`manifest.yaml` targets `0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d`, declares the Transfer event, and points at **`build/handler.wasm`**. It contains both a declarative `TokenTransfer` rule and a WASM handler, so the engine can run both paths for a contract log.

The guest reads event JSON but never parses or uses its fields. Instead it increments an `i64` counter at the fixed key `account_balance:0xUSDC` and emits `AccountBalance` with only `{"balance":...}`. This is not ERC-20 balance accounting: it ignores sender, recipient and transfer amount, has a 64-byte read buffer, ignores host error statuses, and increments again on replay. The emitted payload has no stable `id` or corresponding GraphQL schema here. Persistence/query behavior needs an explicit entity design; successful compilation alone does not make this a complete indexer.

The declarative rule extracts address/block, padded topic values, and raw data bytes `[0..32]` into hex strings, not a decoded decimal transfer amount. The current mapper treats the name `Transfer` as a permissive match rather than comparing its canonical topic hash. Use verified topic matching and fixtures when adapting this template.

## Runtime configuration

After correcting and building the guest, load this native manifest with the **Rust runtime** using `MANIFEST_PATH` / `--manifest-path`. This is a different executable from the npm SDK's `logrix` CLI. The handler's relative `build/handler.wasm` path is resolved from the runtime working directory, not automatically relative to the manifest file.

Set process configuration explicitly: `CHAIN_ID=421614`, `CONTRACT_ADDRESS` for the target above, `RPC_URL`, an appropriate `START_BLOCK`, `DATABASE_URL`, queue driver/URLs, and WASM limits. Supply `SCHEMA_PATH` only after providing a schema for the entities you intend to query. The manifest's `chain_id` does not configure the CLI process's `CHAIN_ID`; this manifest contains no RPC/database/queue credentials or initial-block configuration. See [runtime configuration and precedence](../../docs/configuration.md).

Use literal `${NAME}` references for secrets where supported and inject variables into the runtime process/pod. Manifest string placeholders are expanded during Rust manifest loading; the RPC gateway also expands references in primary/fallback URL settings. Missing/invalid/unclosed references fail those loading paths. The runtime does not automatically load `.env`. This template has no Secret-generation step; provide secrets through deployment tooling rather than committing populated credentials.

## Deployment handoff

Use a corrected, pre-built WASM artifact with a versioned Logrix container image. Mount the manifest and WASM at matching paths and provide backing services/environment through infrastructure tooling:

- [Helm chart](../../deploy/helm/logrix/README.md)
- [Raw Kubernetes](../../deploy/k8s/README.md)
- [AWS Terraform](../../deploy/terraform/aws/README.md)

SDK `export-values` is not directly compatible with this layout: it expects `logrix.yaml`, `schema.graphql`, and the fixed `build/mapping.wasm`, and rewrites all exported handler paths to `/etc/logrix/handlers/mapping.wasm`. Use explicit deployment mounts/values or deliberately adapt the project; simply running the exporter here is not a working deployment flow.

Version numbers are not uniform: this starter's package is `1.0.0` with compiler `^0.27.34`; the Rust workspace is `0.7.3`, SDK package `0.7.0`, and SDK-generated projects still declare `^0.6.0` and an AWS runtime image tag `0.6.0`. This starter itself selects no runtime image. Choose and test the guest/runtime combination explicitly.
