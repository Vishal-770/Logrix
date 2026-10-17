# logrix-rpc-gateway

`RpcGateway` implements `ChainPort` by routing calls through `ProviderPool` and
managed `EvmChainClient` instances. The CLI creates the gateway; reconcilers and
workers use it through the core port. Dependency direction is gateway -> EVM
client -> core, not the reverse.

## Prerequisites and minimal usage

Use the workspace Rust toolchain and locked dependencies, Tokio, and one or more
HTTP(S) EVM RPC endpoints serving the requested history. No database or queue is
needed for this library. In an async Rust application depending on
`logrix-core` and `logrix-rpc-gateway`:

```rust
use logrix_core::{domain::ChainId, error::LogrixResult, ports::ChainPort};
use logrix_rpc_gateway::{GatewayConfig, ManagedProvider, ProviderPool, RpcGateway};

async fn read_height(primary: &str, fallback: &str) -> LogrixResult<u64> {
    let id = ChainId::new(42161);
    let config = GatewayConfig::default();
    let pool = ProviderPool::new(id);
    pool.add_provider(ManagedProvider::new_with_config(
        "primary", primary, 1, id, &config, None,
    )).await;
    pool.add_provider(ManagedProvider::new_with_config(
        "fallback", fallback, 2, id, &config, None,
    )).await;
    let gateway = RpcGateway::new_with_config(id, pool, None, None, &config);
    gateway.get_latest_block_number().await
}
```

Use unique provider names: the per-request tried-provider set is keyed by name.
Pass configuration to both providers and gateway; configuring the gateway alone
does not change already-created provider breakers. Constructors label the chain
but do not check remote chain IDs.

## Routing, caching, and fixed behavior

- Lowest numeric priority wins among healthy candidates; equal-priority
  candidates round-robin. Known nonzero tips more than three blocks behind the
  highest observed tip receive a fixed +10 priority penalty. This is not a
  configurable finality/staleness guarantee. EWMA latency is recorded for
  reporting, not used to prefer faster healthy providers.
- A failed operation falls through untried providers. Selected error-text
  substrings (`more than`, `exceeded`, `range`, `limit`, `too many`) instead
  return immediately without recording a breaker failure. This is a text
  heuristic, not general permanent/transient error classification.
- If no healthy candidate exists, selection still tries an untried provider
  ordered by priority and sampled cooldown. This best-effort path can call an
  open breaker. Provider cooldown doubles on failure and resets on success;
  its jitter sample does not itself sleep or enforce the breaker's recovery
  timeout. Failover may wait for HTTP timeouts and client retries.
- Only latest block height has a single-entry TTL cache (500 ms by default) and
  SingleFlight coalescing. Headers, envelopes, logs, receipts, and arbitrary RPC
  queries are not cached/coalesced by the gateway. This is not an LRU cache.
  SingleFlight's cancelled-leader cleanup spawns a Tokio task; it is not a
  global bound on waiting time or process memory.
- `effective_chunk_size()` recommends the minimum across healthy providers,
  or all providers if none are healthy. It does not split a supplied log range.
- `BulkStreamClient` stores custom/default SQD endpoint strings (defaults for
  chain IDs 1, 42161, 421614, 137, 8453). `is_supported()` only means an endpoint
  is present. `stream_logs()` always returns `Ok(None)`: no SQD or HyperSync
  transport exists. Gateway log fetching therefore uses JSON-RPC. There is no
  supported bulk throughput or zero-cost backfill claim.

## CLI-owned mappings

This library reads no flags, manifest settings, or `RPC_*` environment variables.
The [CLI factory](../logrix-cli/src/factory.rs) supplies the following values:

| Variable | CLI Flag | Description | Default | Required? | Example |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `RPC_URL` | `--rpc-url` | Primary JSON-RPC endpoint (priority 1) | `https://sepolia-rollup.arbitrum.io/rpc` | Default supplied | `https://arb1.arbitrum.io/rpc` |
| `RPC_FALLBACK_URLS` | `--rpc-fallback-urls` | Comma-delimited fallback endpoints (Tier 2/3) | None | Optional | `https://rpc.ankr.com/arbitrum,https://arb1.arbitrum.io/rpc` |
| `CU_BUDGET` | `--cu-budget` | Maximum Compute Units allowed before halting queries | None (Unlimited) | Optional | `10000000` |
| `RPC_TIMEOUT_SECS` | `--rpc-timeout-secs` | Network request timeout in seconds | `10` (CLI) / `15` (Direct) | Optional | `30` |
| `RPC_CB_THRESHOLD` | `--rpc-cb-threshold` | Consecutive failures before tripping provider circuit breaker | `5` (CLI) / `3` (Direct) | Optional | `3` |
| `RPC_COOLDOWN_SECS` | `--rpc-cooldown-secs` | Initial circuit breaker cooldown duration in seconds | `30` (CLI) / `10` (Direct) | Optional | `60` |
| `BULK_STREAM_URL` | `--bulk-stream-url` | Stored endpoint override; no implemented transport | None (then chain endpoint string if known) | Optional | `https://archive.example.com` |

Primary priority is 1; fallback URL entries get priorities 2, 3, etc., in order.
The CLI applies failure threshold 5, success threshold 2, recovery timeout
30 seconds, cooldown bounds 30..120 seconds, and EVM timeout 10 seconds.
Managed providers force supervised EVM single-call retries: one retry after the
first attempt, 50 ms initial/100 ms maximum backoff. `RPC_MAX_RETRIES` is not
read. See [EVM configuration](../logrix-chain-evm/README.md) for library settings.

Direct Rust loading of an SDK manifest does not apply `network.rpcUrl`,
fallback URLs, or RPC/CU/bulk indexing settings. SDK `export-values` maps those
to Helm values and then chart environment for the CLI. See
[configuration](../../docs/configuration.md).

## Library configuration: `GatewayConfig`

All fields are public; builders update selected fields. `RpcGateway` consumes
the cache TTL; `ManagedProvider` consumes breaker and cooldown settings:

| Field | Builder Method | Default Value | Required? | Description |
|---|---|---|---|---|
| `block_cache_ttl` | `.with_block_cache_ttl()` | `500ms` | Optional | TTL for caching latest block height queries |
| `circuit_breaker_failure_threshold` | `.with_circuit_breaker()` | `3` | Optional | Consecutive errors required to trip circuit breaker |
| `circuit_breaker_success_threshold` | `.with_circuit_breaker()` | `2` | Optional | Consecutive successful probes to close circuit breaker |
| `circuit_breaker_recovery_timeout` | `.with_circuit_breaker()` | `10s` | Optional | Cooldown duration before attempting half-open probe |
| `initial_cooldown_secs` | `.with_cooldown_bounds()` | `10s` | Optional | Initial backoff cooldown period |
| `max_cooldown_secs` | `.with_cooldown_bounds()` | `120s` | Optional | Maximum ceiling for exponential cooldown backoff |

## CU accounting and errors

`RpcGateway::new[_with_config]` accepts optional `max_cu_budget` and
`bulk_endpoint` separately from `GatewayConfig`. Budget defaults to `None`.
`CuBudgetTracker` counts successful gateway operations with fixed estimates:
`eth_blockNumber` 10, `eth_getBlockByNumber` 20, `eth_getLogs` 75, other methods
15. Envelope fetching is charged as a get-block operation even when it also
fetches logs. Failed calls and internal retries are not counted; cache hits do
not add CUs. The counter is process-local and resets on construction.

The budget check happens before execution, not by reserving projected cost.
The last operation and concurrent calls can overshoot; it is not a provider bill
or strict spend cap. Once exhausted, uncached gateway operations return a
rate-limited `ChainRpc` error without an automatic refill/resume mechanism.
This affects live queries too, not just backfill. The CLI dry-run estimate uses
95 CU/chunk and $1/million CU as heuristics, not measured billing/duration.

An empty pool returns a permanent configuration-like `ChainRpc` error; exhausted
fallbacks return the last error. Query-limit-text errors return immediately;
other failures record provider failure and try the next untried name, including
errors classified permanent by the EVM client. SingleFlight followers receive
transient wrapper errors when the shared call fails. Normal missing blocks
remain `Ok(None)`; transport failures are not synthesized as empty results by
this gateway.

`masked_url`/`redact_rpc_url` mask embedded user-info only; path/query tokens
are not comprehensively redacted. Avoid treating them as a general secret
sanitizer. No availability, latency, or memory-use guarantee follows from the
pool, retry, or cache mechanisms.

## Tests and related code

From the workspace root with cached dependencies:

```sh
cargo test -p logrix-rpc-gateway --offline --locked
```

[Provider tests](tests/provider_tests.rs) cover priority/round-robin routing,
staleness, counters, and SingleFlight; [failover tests](tests/failover_tests.rs)
use mocked HTTP endpoints for fallback, cache, and error paths. Related:
[source](src/), [CLI reference](../logrix-cli/README.md),
[reconciler](../logrix-reconciler/README.md), and
[configuration](../../docs/configuration.md).
