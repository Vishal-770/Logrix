# logrix-chain-evm

EVM client implementation for log extraction, chunking, and JSON-RPC decoding.

## Overview
Connects to EVM execution nodes and archive nodes to extract logs matching indexed contracts.

## Features
- **Adaptive AIMD Chunker (`chunker.rs`)**: Dynamically expands and halves `eth_getLogs` block query ranges in response to node response size limits.
- **Token Bucket Rate Limiter**: Client-side rate limiting to stay within provider request quotas.
- **Exponential Retry Policy**: Automatic retry backoff on transient network dropouts and 429 rate limit errors.
- **Dynamic ABI Event Decoder (`decoder.rs` & `events.rs`)**: Parses contract events from human-readable signatures or full ABI JSON definitions.
- **Zero Floating-Point Truncation**: All EVM values and token amounts (`DecodedTransfer::amount_u256`) are represented via `alloy_primitives::U256` or exact decimal strings.

---

## Configuration & Tuning (`EvmClientConfig`)

The EVM client runtime is configured via `EvmClientConfig`. All options provide production defaults, making manual tuning optional:

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
| `max_retries` | `.with_retries()` | `3` attempts | Optional | Maximum retry attempts for transient RPC errors |
| `retry_initial_backoff` | `.with_retries()` | `200ms` | Optional | Initial exponential backoff delay |
| `retry_max_backoff` | `.with_retries()` | `5s` | Optional | Maximum capped retry delay |

### Programmatic Usage

```rust
use logrix_chain_evm::{EvmClient, EvmClientConfig};
use std::time::Duration;

// 1. Using default configuration
let default_config = EvmClientConfig::default();

// 2. Custom configuration using fluent builder methods
let custom_config = EvmClientConfig::default()
    .with_timeout(Duration::from_secs(30))
    .with_chunk_bounds(100, 1, 500)
    .with_rate_limit(50, 25.0)
    .with_retries(5, Duration::from_millis(500), Duration::from_secs(10));

let client = EvmClient::new_with_config("https://arb1.arbitrum.io/rpc", custom_config)?;
```
