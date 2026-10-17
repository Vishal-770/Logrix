# logrix-chain-evm

EVM JSON-RPC adapter implementing `logrix_core::ports::ChainPort`, with log
parsing, an adaptive range-size recommendation, and ABI event decoding.
`logrix-rpc-gateway` wraps this client to add provider selection.

## Prerequisites and minimal usage

Use the workspace Rust toolchain and locked dependencies. Network calls require
an HTTP(S) EVM RPC endpoint serving the requested history (archive access depends
on the node/provider). No database or queue is required by this crate.

This function belongs in an async Rust application depending on `logrix-chain-evm`
and `logrix-core`; the caller supplies its endpoint and runs it on Tokio:

```rust
use logrix_chain_evm::{EvmChainClient, EvmClientConfig};
use logrix_core::{domain::ChainId, error::LogrixResult, ports::ChainPort};
use std::time::Duration;

async fn read_height(rpc_url: &str) -> LogrixResult<u64> {
    let config = EvmClientConfig::default()
        .with_timeout(Duration::from_secs(20));
    let client = EvmChainClient::new_with_config(ChainId::new(42161), rpc_url, config);
    client.get_latest_block_number().await
}
```

Other `ChainPort` methods are `get_block_by_number`, `fetch_logs` (inclusive
range; empty addresses means no address filter), and `fetch_block_envelope`.
`call_rpc_batch` accepts method/JSON parameter pairs and orders results by request
ID. `AbiEventDecoder` accepts human-readable signatures or ABI JSON;
`decode_erc20_transfer` returns `Option<DecodedTransfer>` and stores amounts as
decimal strings. Decoder timestamps are processing time, not block timestamps.

## Library configuration: `EvmClientConfig`

The library does not read CLI flags, manifests, or `RPC_*` environment variables.
All fields are public; builders below update selected fields.

| Field | Builder Method | Default Value | Required? | Description |
|---|---|---|---|---|
| `timeout` | `.with_timeout()` | `15s` | Optional | HTTP request timeout for JSON-RPC network calls |
| `pool_idle_timeout` | N/A | `90s` | Optional | Maximum idle connection retention duration in HTTP pool |
| `pool_max_idle_per_host` | N/A | `20` | Optional | Maximum idle sockets kept open per RPC host |
| `chunk_initial` | `.with_chunk_bounds()` | `500` blocks | Optional | Initial block range size for adaptive chunking queries |
| `chunk_min` | `.with_chunk_bounds()` | `5` blocks | Optional | Minimum block range floor when halving after RPC errors |
| `chunk_max` | `.with_chunk_bounds()` | `2000` blocks | Optional | Maximum block range ceiling when expanding under light load |
| `chunk_additive_step` | N/A | `50` blocks | Optional | Additive increase step size upon consecutive successful queries |
| `chunk_success_threshold` | N/A | `3` queries | Optional | Consecutive successful queries before increasing chunk size |
| `chunk_target_latency` | N/A | `800ms` | Optional | Latency threshold above which chunk expansion halts |
| `rate_limit_capacity` | `.with_rate_limit()` | `100` tokens | Optional | Burst capacity for client-side token bucket rate limiter |
| `rate_limit_refill_rate` | `.with_rate_limit()` | `50.0` tokens/sec | Optional | Token refill rate per second |
| `max_retries` | `.with_retries()` | `3` retries | Optional | Single-call retry count after the first attempt (up to 4 attempts) |
| `retry_initial_backoff` | `.with_retries()` | `200ms` | Optional | Initial exponential backoff delay |
| `retry_max_backoff` | `.with_retries()` | `5s` | Optional | Maximum capped retry delay |

`new_supervised[_with_config]`, used by managed gateway providers, forces
`max_retries = 1` (up to two single-call attempts), with 50 ms initial and 100 ms
maximum backoff. Batch calls do not use that retry loop.

## CLI-owned mappings

The [CLI](../logrix-cli/README.md) constructs clients with
`RPC_TIMEOUT_SECS` / `--rpc-timeout-secs` (10 seconds), overriding the library's
15-second timeout. It separately uses `CHUNK_SIZE` / `--chunk-size` (10 blocks)
to cap ingester jobs; this does not set `EvmClientConfig.chunk_initial`.
Manifest `indexing.chunkSize`, `maxQueueDepth`, and `chainHeadPollMs` override
ingester tuning only. SDK-to-Helm export is a separate configuration path; see
[configuration](../../docs/configuration.md).

## Fixed behavior, errors, and limitations

- `fetch_logs` sends the supplied range as one request. It does not automatically
  split ranges; callers must use `chunker().chunk_size()` or implement splitting.
  The CLI ingester uses the recommendation, and processors halve certain failed
  range jobs based on error text.
- Single calls retry transport failures, HTTP 429/5xx, and JSON-RPC error objects.
  Integer `Retry-After` seconds are honored for 429/5xx. Other HTTP client errors
  return permanent errors; malformed response JSON returns a transient error
  without retrying at that point. Exhausted JSON-RPC errors are permanent.
- A null block result means `Ok(None)`. Required block hashes are checked, but
  individual log parsing is permissive: malformed fields may become zero/empty
  values and invalid topics may be skipped. ABI decode failures return `None`.
- Envelope fetching first batches header and logs, then tries sequential calls
  on any batch error. Batch errors are explicit, but a matching response item
  without `result` becomes JSON null. Batch size and response memory are not
  bounded by this crate.
- The supplied chain ID labels results; construction does not verify the remote
  node's chain ID. Rate limiting is local to a client, not provider-wide quota
  enforcement. Client construction can panic if the HTTP client cannot be built.

## Tests and related code

From the workspace root, with dependencies already cached:

```sh
cargo test -p logrix-chain-evm --offline --locked
```

Tests cover ABI decoding, parsing, chunking, and mocked RPC status/error behavior.
See [source](src/), [tests](tests/), [gateway](../logrix-rpc-gateway/README.md),
and the [configuration guide](../../docs/configuration.md).
