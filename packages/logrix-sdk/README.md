# @logrix/sdk

Developer CLI and AssemblyScript guest library for [Logrix](https://github.com/Vishal-770/Logrix) blockchain indexers. The CLI scaffolds projects, generates bindings, validates files, compiles WASM, and exports Helm values. Deployment is performed separately with Helm, Kubernetes, or Terraform.

## Quick start

Use Node.js and npm for the CLI, and AssemblyScript 0.27.x for handlers. From a directory where a new project can be created:

```bash
npx @logrix/sdk init my-indexer --yes --network arbitrum-one
cd my-indexer
npm install
npm run codegen
```

**Current compilation workaround:** after codegen, change imports/re-exports of `"@logrix/sdk"` to `"@logrix/sdk/src/runtime/index"` in `src/mapping.ts`, `src/generated/events.ts`, and `src/generated/schema.ts`. The package root currently re-exports testing source with optional properties and nullish-coalescing syntax that `asc` cannot parse. Codegen overwrites the generated files, so repeat this adjustment after each generation. Then:

```bash
npm run validate
npm run build:asc
npm run export-values
```

Review the generated ERC-20 ABI, contract address, and start block before indexing. `init` always writes the Transfer/Approval sample; selecting a different contract name or address does not discover its ABI. A start block of zero can imply a large backfill.

The generated scripts are `codegen`, `validate`, `build`, `build:asc`, and `export-values`. There is no generated `test` script. Use `npx logrix test` for the CLI's limited local checks, described below.

`build:asc` is used here because `logrix build` currently selects the first data source's `mapping.file` as its compiler input if that file already exists. With the default manifest, a second build can therefore try to compile `build/mapping.wasm` as source. The direct compiler script always uses `src/mapping.ts`.

### Versions in this checkout

These versions are independent; inspect the installed package and selected container image:

| Component | Current value |
| --- | --- |
| Rust workspace | `0.7.3` |
| SDK package | `0.7.0` |
| SDK CLI `--version` | hard-coded `0.6.0` |
| Dependency written by `init` | `@logrix/sdk: ^0.6.0` |
| Image tag written to `deploy/values-aws.yaml` | `0.6.0` |

Installing a generated project does not automatically select this checkout's SDK. Check its lockfile and review the AWS image tag before deployment. The chart defaults to `latest`; select a tested, versioned image explicitly.

## AssemblyScript, generated files, and the host ABI

Handler `.ts` files are **AssemblyScript**, a TypeScript-like language compiled by `asc` to WebAssembly. Types such as `i32`, `u64`, `usize`, `load<u8>`, and `heap.alloc` are guest-language features. These files are not ordinary Node.js or browser TypeScript. The package root exports AssemblyScript runtime/testing source; importing it into a normal TypeScript application does not provide a JavaScript indexer client.

`init` creates:

```text
logrix.yaml
schema.graphql
abis/TokenContract.json
src/mapping.ts
deploy/values-aws.yaml
deploy/secrets.example.yaml
package.json
tsconfig.json
.gitignore
README.md
```

Neither `init` nor `export-values` creates `deploy/values-local.yaml`.

| Step | Actual output |
| --- | --- |
| `codegen` | `src/generated/events.ts` and `src/generated/schema.ts` (overwritten) |
| `build` / `build:asc` | default `build/mapping.wasm` |
| `test` | `build/test_mapping.wasm` when compilation succeeds |
| `export-values` | default `indexer-values.yaml`; missing AWS/Secret example files in `deploy/` |

Event bindings are named `<DataSource>_<Event>Event` and `<DataSource>_<Event>Params`. The first event with a given name also gets an unscoped alias such as `TransferEvent`. Address constants and `isFromContract` help with manual routing. Entity bindings implement constructors, static `load(id)` / `remove(id)`, and instance `save()`. `@derivedFrom` fields are omitted from stored payloads; codegen does not generate reverse-lookup methods.

The Rust host serializes each EVM `EventLog` as UTF-8 JSON into exported guest `memory`, calls `allocate(size)` (or `alloc`, with a legacy fixed-address fallback), then calls `handle_event(ptr, len) -> i32`. Export `allocate` explicitly. Return zero on success; negative results and traps fail execution and discard that invocation's staged writes. Named manifest handlers such as `handleTransfer` are not called directly by the host: `handle_event` must parse and dispatch.

SDK imports use the `env` namespace. The host registers the following functions under both `env` and `logrix`:

| Import | WASM signature / behavior |
| --- | --- |
| `logrix_db_get` | `(key_ptr, key_len, out_ptr, max_len) -> i32`; bytes written, `-1` missing/invalid input, `-2` insufficient output buffer |
| `logrix_db_set` | `(key_ptr, key_len, val_ptr, val_len) -> i32`; stage key/value state |
| `logrix_db_remove` | `(key_ptr, key_len) -> i32`; stage removal |
| `logrix_emit` | `(entity_ptr, entity_len, json_ptr, json_len) -> i32`; stage an entity payload |
| `logrix_log` | `(level, msg_ptr, msg_len) -> i32`; stage a diagnostic (0 debug, 1 info, 2 warning, otherwise error) |

All ABI arguments and results above are `i32`; pointers refer to guest linear memory. The host also supplies AssemblyScript's `env.abort`. There are no guest HTTP/RPC or filesystem imports in this linker. The guest library's `log` methods call the staging API; the current engine does not forward those staged diagnostics into its returned processing buffer.

### Generator-compatible Transfer handler

After `codegen` with the default ERC-20 ABI and schema, and the generated-import workaround above, this can replace `src/mapping.ts` for a Transfer-only example. It parses the real payload, matches topic0, and fills the generated entity. Approval events are intentionally skipped by this example; the scaffold includes a separate Approval handler.

```typescript
import { EventLog, log } from "@logrix/sdk/src/runtime/index";
import { TransferEvent } from "./generated/events";
import { Transfer } from "./generated/schema";

export { allocate } from "@logrix/sdk/src/runtime/index";

export function handle_event(ptr: i32, len: i32): i32 {
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = load<u8>(ptr + i);
  }
  const raw = EventLog.fromJson(String.UTF8.decode(bytes.buffer));
  if (raw.topics.length < 3) return 0;
  if (raw.topics[0] != "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef") {
    return 0;
  }
  const event = new TransferEvent(raw);
  const id = event.transactionHash + "-" + event.logIndex.toString();
  const entity = new Transfer(id);
  entity.blockNumber = event.blockNumber;
  entity.fromAddress = event.params.from.toHexString();
  entity.toAddress = event.params.to.toHexString();
  entity.amount = event.params.value;
  entity.transactionHash = event.transactionHash;
  entity.timestamp = event.blockTimestamp;
  entity.save();
  log.info("Indexed Transfer: " + id);
  return 0;
}
```

For multiple contracts, check `raw.address` and use scoped event classes. Event names are not present in the raw EVM payload, so searching its JSON for `"Transfer"` is not a dispatcher.

### Runtime API and current limits

| API | Implemented behavior / boundary |
| --- | --- |
| `EventLog.fromJson(json)` | Parses address, topics, data, block/hash/index and `tx_hash` (or `transaction_hash`). This is a simple field extractor, not a general JSON parser. Numeric metadata passes through `i32` conversions. |
| `store.get(type, id)` | Reads `type:id` from host state, returning raw JSON or `""`. The SDK uses a fixed 8192-byte read buffer and treats non-positive host results as empty, including buffer-too-small errors. |
| `store.set(type, id, json)` | Stages state and emits an entity; persistence is handled by the Rust processor after execution. |
| `store.remove(type, id)` | Stages state removal and emits an `id` / `_deleted` payload. |
| `BigInt` | String-backed decimal/hex conversion; `fromString`, `fromI32`, `fromU64`, `zero`, `plus`, `minus`, `times`, `div`, comparisons and string/hex output. It is not native JavaScript `bigint`. |
| `Address` | Lowercases hex and extracts the last 20 bytes of a 32-byte topic. It does not strictly validate address input. |
| `Bytes` | Hex/UTF-8/byte-array conversions, equality and length; input is not strictly validated. |
| `crypto.keccak256(bytes)` | Guest implementation accepting and returning `Bytes`; not a host RPC/hash service. |
| `formatUnits(value, decimals = 18)` | Decimal-string formatting without floating point. |
| `decodeDynamicString` / `decodeDynamicBytes` | Basic Solidity head/tail decoding for non-indexed string/bytes data; malformed/short data can yield empty values. |
| `log.debug/info/warning/error(message)` | Single-string diagnostic wrappers around `logrix_log`. |
| `createMockEventLog` | Exported source helper, not a Node.js mock host or a WASM execution test runner. |

Important limitations when adapting handlers:

- Rust's current per-log JSON has no `block_timestamp`; the SDK's `blockTimestamp` therefore defaults to zero. The handler above stores that zero, not a real block time. Large metadata values can also be truncated by the SDK parser's `i32` intermediate.
- `BigInt` arithmetic/comparison helpers assume normalized non-negative decimal operands. `minus` can produce a negative string, but subsequent signed arithmetic is not implemented generally. `div(0)` returns zero. Codegen converts Solidity `int*` words as unsigned hex, without two's-complement sign decoding. Do not treat signed Uniswap amounts/ticks as correctly decoded values.
- ABI generation is a limited decoder, not full Solidity ABI support: arrays/tuples, indexed dynamic values and some scalar types (including bool bindings) require review/custom decoding. Entity serialization concatenates strings without general JSON escaping; nullable/list/relation types are not fully modeled.
- The package root's `createMockEventLog` re-export currently makes even unused root imports fail `asc` parsing. Use the runtime subpath for handlers and generated imports until that testing source is made AssemblyScript-compatible. Compile a real guest and provide host imports to test execution; `node --test` cannot directly execute AssemblyScript handlers.

## Configuration: SDK to Helm versus direct runtime

See [configuration reference](../../docs/configuration.md) for runtime environment variables and precedence. An SDK project manifest is not a complete replacement for infrastructure values or the Rust process environment.

```yaml
specVersion: 0.1.0
network:
  name: arbitrum-one
  chainId: 42161
  rpcUrl: "https://arb-mainnet.g.alchemy.com/v2/${ALCHEMY_API_KEY}"
  fallbackRpcUrls:
    - "https://arb1.arbitrum.io/rpc"
dataSources:
  - kind: ethereum/contract
    name: TokenContract
    network: arbitrum-one
    source:
      address: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"
      abi: TokenContract
      startBlock: 0
    mapping:
      kind: wasm/assemblyscript
      file: ./build/mapping.wasm
      enabled: true
      fuelLimit: 5000000
      maxMemoryMb: 128
      abis:
        - name: TokenContract
          file: ./abis/TokenContract.json
      eventHandlers:
        - event: Transfer(address indexed,address indexed,uint256)
          handler: handleTransfer
webhooks:
  - url: "https://hooks.example.com/logrix"
    events: [reorg]
    secret: "${WEBHOOK_SECRET}"
    maxRetries: 3
indexing:
  ringBufferDepth: 128
  reconcilerIntervalSecs: 30
  chunkSize: 10
  rpcTimeoutSecs: 15
  databaseMaxConnections: 20
```

This illustrates the export format, not complete deployment credentials. The webhook dispatcher and its backing services must also be configured.

### `export-values` conversion

- Writes `config.chainId`, symbolic `rpcUrl`, comma-joined `rpcFallbackUrls`, first `contractAddress`, minimum source `startBlock` (as a string), `ringBufferDepth` (default 128), and `reconcilerIntervalSecs` (default 30).
- Embeds `schema.graphql` as `config.schemaContent`, the fixed **`build/mapping.wasm`** as `config.wasmBase64`, and a converted runtime YAML manifest as `config.manifestContent`. Every contract points to **`/etc/logrix/handlers/mapping.wasm`**, regardless of `mapping.file` or `build --out`. Separate binaries per data source are not bundled.
- The embedded manifest contains `schema_version`, `chain_id`, contract names/addresses, event strings, `start_block`, `wasm_handler`, optional `enabled` / `fuel_limit` / `max_memory_mb`, and webhooks with `max_retries`. The Rust contract type currently ignores per-contract `start_block`; the exported global `config.startBlock` is what configures initial ingestion.
- The first webhook URL also becomes `config.webhookUrl`, and any webhook enables the chart's webhook component. Named handlers, ABI files, and the SDK `indexing` object are not copied into the runtime manifest.

The following `indexing` keys are forwarded to same-named Helm `config` values:

| Group | Keys |
| --- | --- |
| Ingestion/reconciliation | `chunkSize`, `reconcilerChunkSize`, `reconcilerLagThreshold`, `chainHeadPollMs`, `maxQueueDepth` |
| RPC/backfill | `rpcTimeoutSecs`, `rpcCbThreshold`, `rpcCooldownSecs`, `cuBudget`, `bulkStreamUrl` |
| Database/queues | `databaseMaxConnections`, `databaseMinConnections`, `rabbitmqPrefetch`, `rabbitmqTimeoutSecs`, `sqsWaitTimeSeconds`, `sqsMaxMessages` |
| WASM | `wasmFuelLimit`, `wasmMaxMemoryMb` |
| API | `graphqlMaxDepth`, `graphqlMaxComplexity`, `graphqlDefaultLimit`, `graphqlMaxLimit`, `graphqlQueryTimeoutSecs` |
| Webhooks | `webhookTimeoutSecs`, `webhookMaxRetries` |

The chart translates these values to environment variables. Passing validation does not mean every arbitrary `indexing` key is forwarded: the converter uses the explicit list above. Infrastructure such as queue driver/URLs, database URL, S3, image, resource requests, replicas, and KEDA belongs in separate Helm values/Secrets.

### Direct Rust runtime loading

The Rust CLI is a different executable also named `logrix`. Supplying `MANIFEST_PATH` (or `--manifest-path`) accepts SDK `logrix.yaml` or native `manifest.yaml`. SDK conversion there reads the chain ID, contract addresses, event strings, mapping file/enable/limits, webhooks, and a limited `ManifestIndexing` structure. It does **not** configure the process's RPC URL, chain ID, database/queue, start block, or schema path from the SDK network/source sections. Set the runtime environment/flags explicitly, including `CHAIN_ID`, `RPC_URL`, `START_BLOCK`, `SCHEMA_PATH`, and service credentials. Relative WASM paths are resolved against the process working directory.

Of the manifest tuning fields, the ingester actually consumes `chunkSize`, `maxQueueDepth`, and `chainHeadPollMs`; the handler engine consumes `wasmFuelLimit` / `wasmMaxMemoryMb` and per-contract overrides. Parsed manifest ring-buffer/reconciler settings do not override the current processor configuration. RPC, DB, queue, GraphQL, and webhook tuning require runtime env/flags. Manifest WASM defaults override CLI-global limits, and per-contract limits override both.

### Secret placeholders and startup

Use literal `${NAME}` references in YAML (single-quote them in shell arguments). Export does not read `.env` or interpolate `process.env`. RPC/fallback/webhook URL references stay symbolic. A webhook secret that is exactly `${NAME}` is preserved; a literal or mixed secret is replaced by `${WEBHOOK_SECRET}` for the first webhook, `${WEBHOOK_SECRET_2}` for the second, etc. Supply the intended secret separately.

When these recognized references exist, export sets `config.existingSecret` to the top-level SDK `existingSecret` value or `logrix-secrets`. A newly written Secret example has `CHANGE_ME` entries for `RPC_URL`, `RPC_FALLBACK_URLS`, `DATABASE_URL`, and discovered variable names. Existing examples are not updated unless `--deploy-profiles` is used; that flag overwrites both example profiles.

Prepare a private Secret with real values and the correct namespace/name before deployment. Secret env entries override ConfigMap env entries: leaving `RPC_URL`, `RPC_FALLBACK_URLS`, or `DATABASE_URL` as `CHANGE_ME` breaks startup, and an empty fallback value disables the exported fallback list. Remove keys you intend to inherit from the ConfigMap, or provide the complete desired value. Never apply the untouched example or commit populated credentials. Literal RPC URLs containing keys are copied into values; use placeholders instead.

At runtime, the RPC gateway expands primary/fallback references, processor configuration expands its webhook URL/secret, and manifest loading recursively expands string values. Missing variables, invalid names, and unclosed references produce configuration errors on those paths; `.env` files are not automatically loaded. The dedicated webhook dispatcher seeds manifest subscriptions separately and currently skips manifest parse failures, so do not rely on it alone to diagnose missing variables.

## CLI reference

Run project commands in the directory containing `logrix.yaml`. All commands support `-h, --help`; top-level `-V, --version` prints the CLI's hard-coded version. `logrix help [command]` displays command help. There are no SDK deploy/start/serve commands or common `--manifest`, `--config`, or `--env-file` options.

### `logrix init [name]`

| Flags | Behavior |
| --- | --- |
| `-y, --yes` | Skip prompts; non-TTY stdin also selects non-interactive mode. Default project name: `my-indexer`. Existing target directories are rejected. |
| `--network <network>` | One of the presets below; missing/unknown values select Arbitrum One in non-interactive mode (interactive mode prompts). No custom chain-ID flag. |
| `--rpc <url>` | Override the preset RPC URL. |
| `--contract-name <name>` | ABI/source name; default `TokenContract`. |
| `--address <address>` | Default Arbitrum USDC address, even if another network is selected; set it explicitly for that chain. |
| `--start-block <block>` | Default `0`. |
| `--fallback-rpc <urls...>` | Space-separated URL arguments. |
| `--webhook-url <url>` | Adds a subscription with `reorg` and `Transfer`, retry count 3. |
| `--reorg-depth <depth>` | Writes `indexing.ringBufferDepth`, default 128. |
| `--reconciler-interval <secs>` | Writes `indexing.reconcilerIntervalSecs`, default 30. |

Preset names: `ethereum`, `arbitrum-one`, `base`, `optimism`, `polygon`, `bsc`, `avalanche`, `linea`, `scroll`, `blast`, `zksync`, `gnosis`, `fantom`, `celo`, `polygon-zkevm`, `sepolia`, `holesky`, `arbitrum-sepolia`, `base-sepolia`, `optimism-sepolia`, `polygon-amoy`, `bsc-testnet`, `avalanche-fuji`, `linea-sepolia`, `scroll-sepolia`, `blast-sepolia`. Presets are static, not availability guarantees. Edit the manifest for other chains and validate numeric/address inputs; CLI flags do not apply every interactive input check.

### `logrix add <type> [nameOrUrl]`

Types are `contract` and `webhook`. Options share one parser; options for the other type have no effect.

| Contract flags | Behavior |
| --- | --- |
| `--address <address>` | Target address; non-interactive default is the zero address. |
| `--abi <path>` | Copies local ABI to `abis/<name>.json`; takes precedence over fetching. |
| `--fetch-abi` | Tries the configured chain's explorer, then Sourcify; failure writes the ERC-20 placeholder. Review the result. |
| `--etherscan-key <key>` | Used only with ABI fetch. |
| `--start-block <block>` | Default `0`. |

Contract addition only edits the ABI/manifest. It writes a Transfer mapping named `handle<Name>Transfer` even for non-ERC-20 ABIs; it does not implement a handler or routing. Edit these and regenerate bindings yourself.

| Webhook flags | Behavior |
| --- | --- |
| `--events <events>` | Comma-separated names, default `reorg`. |
| `--secret <secret>` | Written to `logrix.yaml`; prefer a single-quoted literal reference such as `'${WEBHOOK_SECRET}'`. |
| `--max-retries <retries>` | Default `3`; exported as `max_retries`. |

### `logrix codegen` / `logrix validate` / `logrix status`

These have no command-specific flags. `codegen` emits the two binding files; missing/bad ABI files can merely warn and produce incomplete bindings. `validate` checks YAML/Zod shape, address syntax, ABI JSON arrays, the existence of `src/mapping.ts`, selected limits, URL shape, and an `@entity` pattern. It does not compile handlers, verify named functions/signatures, or fully validate GraphQL/ABI semantics.

`status` makes live RPC calls: primary block height/chain ID, fallback block heights, and primary `eth_getCode` per contract. Its sync delta is head minus configured start block, not database progress. Fallback chain IDs are not checked. It neither expands `${NAME}` nor loads `.env`; it prints the primary RPC URL and configured webhook URLs, so avoid credential-bearing literal URLs in diagnostics output.

### `logrix build`

| Flags | Behavior |
| --- | --- |
| `--entry <file>` | Default `src/mapping.ts`; must exist, but an existing first `mapping.file` overrides the compiler input. |
| `--out <file>` | Default `build/mapping.wasm`; does not update the manifest or exporter's fixed input path. |
| `--debug` | Omits `--optimize`; does not enable source maps/debug metadata itself. |

Uses local `node_modules/.bin/asc`, falling back to `asc` on PATH, with `--config tsconfig.json --exportRuntime`. Use generated `npm run build:asc` for reliable repeated builds with the default paths.

### `logrix export-values`

| Flags | Behavior |
| --- | --- |
| `--out <file>` | Default `indexer-values.yaml`; parent directory must already exist. |
| `--chain-id <id>` | Override exported chain ID. |
| `--rpc <url>` | Override exported RPC URL; references stay symbolic. |
| `--deploy-profiles` | Overwrite `deploy/values-aws.yaml` and `deploy/secrets.example.yaml`. Despite help text, no `values-local.yaml` is generated. |

Requires `logrix.yaml`, `schema.graphql`, and `build/mapping.wasm`. It does not run validation/codegen/build or deploy anything.

### `logrix test [testFile]`

`--watch` is accepted but has **no effect**. The command checks for data sources and schema existence, compiles the fixed `src/mapping.ts` to `build/test_mapping.wasm` with `-O3 --runtime stub --noAssert`, and optionally invokes `node --test` on the supplied file or a `tests/` glob. It does not run `validate`, run `codegen`, execute WASM, or provide a TypeScript/AssemblyScript Node loader. Generate bindings first; create `build/` first if absent. Test-file glob expansion depends on the shell/Node version. Use an explicit runnable JavaScript test file for predictable Node execution.

For **SDK contributors**, the SDK package itself does have `npm test`: from `packages/logrix-sdk`, it builds the CLI and runs `dist/manifest_converter.test.js`.

## Deployment handoff

Export is an application bundle for Helm. Review Secrets, backing infrastructure, autoscaling prerequisites, and a versioned runtime image before applying it. The generated AWS profile contains example account/role/SQS identifiers and an older image tag; it does not provision AWS resources. With default chart values KEDA is enabled, so a cluster needs its CRDs or an explicit override.

**Current chart integration blocker:** exported `manifestContent` sets
`MANIFEST_PATH` for the ingester, but the chart only mounts that file in other
components. The ingester reads it at startup and fails when absent. Apply a
consumer-managed ingester ConfigMap volume/mount patch or supply an equivalent
readable manifest; the exported bundle is not deployable unchanged with the
current source chart/runtime combination. See the chart's file-mount section.

- [Helm chart setup and values](../../deploy/helm/logrix/README.md)
- [Raw Kubernetes deployment](../../deploy/k8s/README.md)
- [AWS Terraform infrastructure](../../deploy/terraform/aws/README.md)
- [Runtime configuration reference](../../docs/configuration.md)

Use pre-built, versioned containers for runtime deployment. SDK commands prepare source and configuration locally; Helm/Kubernetes/Terraform apply deployment resources.
