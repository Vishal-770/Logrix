# arbitrum-uniswap-indexer

An AssemblyScript mapping example for Logrix on Arbitrum One (chain ID `42161`). The manifest targets pool `0xC6962004f452bE9203591991D15f6b388e09E8D0` from block `514022000`. Review that address, block range, and RPC provider access before running an indexer.

## What this example currently does

`src/mapping.ts` parses UTF-8 event JSON with `EventLog.fromJson`, dispatches only the Swap topic prefix, and writes these schema entities:

- `Swap`: transaction/block identifiers and raw padded sender/recipient topics. `amount0`, `amount1`, `sqrtPriceX96`, `liquidity`, and `tick` are explicitly set to zero; the mapping does not decode swap data.
- `PoolDayData`: increments `txCount`; volume and latest-price/tick fields stay at their initialized zero values.
- `UserVolumeStats`: increments `totalSwaps` and records last block/timestamp; volume fields remain zero. User IDs are raw topics, not normalized 20-byte addresses.

This is an aggregation skeleton, not a finished Uniswap price/volume indexer. Although `logrix.yaml` declares Swap, Mint, and Burn, there are no Mint/Burn implementations or dispatch branches. Manifest handler names do not trigger calls: the Rust host calls only `handle_event`.

The current Rust per-event payload omits `block_timestamp`, so the SDK timestamp defaults to zero. Daily aggregation therefore currently collapses to day `0`. Replaying an already-seen Swap still increments the aggregate counters; the per-Swap deterministic ID does not make those increments idempotent. Generated bindings include signed `int256`/`int24` fields, but decode their words as unsigned hex. Correct signed decoding and replay-safe aggregation are needed before using these metrics.

## Local build workflow

Run from this directory. `package-lock.json` is present, so use the locked dependencies:

```bash
npm ci
npm run codegen
```

Before compiling, change `"@logrix/sdk"` imports in `src/generated/events.ts` and `src/generated/schema.ts` to `"@logrix/sdk/src/runtime/index"`. The mapping already uses that runtime subpath. The package root re-exports testing code that `asc` cannot parse; use the documented subpath instead of modifying installed dependencies, which a clean `npm ci` replaces. Repeat the generated-import adjustment after each codegen. Then:

```bash
npm run validate
npm run build:asc
```

The `.ts` mapping and generated bindings are **AssemblyScript**, compiled by `asc` to WASM; they cannot be run as normal Node.js/browser TypeScript. The SDK CLI itself runs under Node.js.

| Script | Result |
| --- | --- |
| `codegen` | Overwrites `src/generated/events.ts` and `src/generated/schema.ts` from ABI/schema files |
| `validate` | Checks manifest shape, ABI JSON arrays, file existence and selected constraints; does not prove mapping behavior |
| `build:asc` | Compiles `src/mapping.ts` to `build/mapping.wasm` with optimization and runtime exports |
| `build` | Invokes `logrix build`; an existing first `mapping.file` can override the compiler input with the output WASM, breaking rebuilds |
| `export-values` | Writes `indexer-values.yaml` using the CLI version installed in this project |

Use `build:asc` for repeated builds. There is no `npm test` script or bundled test suite. Optional `npx logrix test` performs basic config checks, compiles `build/test_mapping.wasm`, and can invoke Node tests; it does not execute WASM or generate bindings, and `--watch` has no effect. `npx logrix status` probes RPC connectivity, not database sync progress; it does not expand `${ALCHEMY_API_KEY}` or load `.env`, and prints URLs.

The host ABI requires exported `memory`, `allocate(size)`, and `handle_event(ptr, len) -> i32`. The host supplies UTF-8 JSON; the SDK uses `env.logrix_db_get/set/remove`, `env.logrix_emit`, and `env.logrix_log` (also registered under `logrix`). Zero is success; negative results/traps fail the invocation. Writes/emissions are staged for the Rust processor. See the [SDK README](../packages/logrix-sdk/README.md) for a generator-compatible handler and runtime/decoder limitations.

## Export with the intended SDK version

The example declares `@logrix/sdk: ^0.6.0`; its lockfile/installed package can differ from the workspace SDK (`0.7.0`). The Rust workspace is `0.7.3`, the CLI reports `0.6.0`, and generated AWS profiles use image `0.6.0`. Do not assume these are synchronized.

To use this checkout's exporter, build the SDK CLI and call it explicitly from this example directory:

```bash
npm --prefix ../packages/logrix-sdk run build
node ../packages/logrix-sdk/dist/index.js export-values
```

This assumes SDK dependencies have already been installed; it selects that exporter, not a different handler dependency. With an intentionally selected installed SDK, `npm run export-values` invokes its exporter instead. Regenerate the existing `indexer-values.yaml` before deployment; it is a stale generated artifact, not the authoritative configuration. Review exported URLs for embedded credentials before sharing values.

Current workspace export reads the fixed `build/mapping.wasm` and `schema.graphql`. It embeds base64 WASM, schema text, and a runtime manifest in Helm `config` values. All contracts use `/etc/logrix/handlers/mapping.wasm`; neither `mapping.file` nor `build --out` selects another export input. It exports the first contract address, minimum source start block, supported operational settings, and symbolic RPC references.

## Configuration and secrets

The manifest's primary RPC is public; fallbacks include Ankr and an Alchemy URL with literal `${ALCHEMY_API_KEY}`. The workspace exporter preserves references without reading local environment variables. At startup the Rust RPC gateway resolves them from the pod/process environment; missing variables are errors. Use a provider appropriate for the historical range and avoid committing credential-bearing URLs.

Export forwards this example's `ringBufferDepth`, `reconcilerIntervalSecs`, `chunkSize`, `reconcilerChunkSize`, `reconcilerLagThreshold`, `chainHeadPollMs`, `rpcTimeoutSecs`, and `databaseMaxConnections` into Helm `config` values. The chart turns them into runtime environment variables. DB/queue endpoints, image, S3, replicas and autoscaling still need infrastructure configuration.

When a reference is detected, the workspace exporter selects `config.existingSecret` (default `logrix-secrets`). This example already contains an older `deploy/secrets.example.yaml`; normal export does not refresh it. To regenerate the examples with discovered variable names:

```bash
node ../packages/logrix-sdk/dist/index.js export-values --deploy-profiles
```

That command **overwrites** `deploy/values-aws.yaml` and `deploy/secrets.example.yaml`. Newly generated Secret entries are `CHANGE_ME`, including RPC, fallback RPC, DB, and `ALCHEMY_API_KEY`. Create a private Secret in the release namespace with real values. Secret env keys override ConfigMap keys: remove keys you want inherited, or supply complete working endpoints/credentials. In particular an empty `RPC_FALLBACK_URLS` overrides the entire fallback list, and `CHANGE_ME` is not a usable endpoint. Never apply the untouched example or commit populated Secrets. Neither init nor export generates `deploy/values-local.yaml`.

For direct Rust runtime use, `MANIFEST_PATH=logrix.yaml` loads contracts/WASM and some tuning but does not select process RPC, chain ID, start block or schema path. Set `RPC_URL`, `CHAIN_ID=42161`, `START_BLOCK=514022000`, `SCHEMA_PATH`, DB/queue credentials, and operational settings explicitly. Relative mapping paths are relative to the process working directory. See the [configuration reference](../docs/configuration.md) for consumed fields and precedence.

## Deployment handoff

Build/export locally, then use pre-built, versioned Logrix containers with infrastructure tooling:

The current source chart sets `MANIFEST_PATH` from exported inline content but
does not mount that manifest into the ingester, which reads it at startup.
This is a deployment blocker: supply a consumer-managed ingester volume/mount
patch or an equivalent readable manifest before enabling it.

- [Helm chart](../deploy/helm/logrix/README.md): review values, Secret precedence, image selection, and KEDA prerequisites.
- [Raw Kubernetes](../deploy/k8s/README.md): mount manifest, schema, and WASM and configure runtime env explicitly.
- [AWS Terraform](../deploy/terraform/aws/README.md): provision infrastructure separately. The generated AWS values contain example account/role/SQS identifiers and an older image tag; replace them deliberately.

The SDK prepares files; it does not deploy services. Deployment verification also needs actual event fixtures, signed-value checks, real block timestamps, and replay tests for the custom aggregators.
