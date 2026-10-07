# logrix-rpc-gateway

Cost-aware, multi-provider JSON-RPC gateway with latency-weighted priority routing, singleflight deduplication, and circuit breakers.

## Overview
Wraps JSON-RPC calls across Ethereum and EVM-compatible chains to ensure 99.99% uptime, prevent rate-limit crashes, and minimize compute unit (CU) costs.

## Features
- **Priority Tiering**: Primary provider (Priority 1) with automatic failover to Priority 2, 3 fallback providers.
- **Latency-Weighted Routing**: Selects the fastest healthy provider using an exponential moving average (`avg_latency_ms`).
- **Circuit Breakers**: Disables failing providers with exponential cooldown backoff (10s to 120s with full jitter).
- **Singleflight Deduplication**: Coalesces duplicate concurrent RPC calls (e.g., identical `eth_getBlockByNumber`) into a single in-flight network request.
- **Compute Unit (CU) Budget Tracker**: Halts historical backfills when configured provider CU budgets are reached.

## Configuration

```bash
RPC_URL="https://arb-mainnet.g.alchemy.com/v2/YOUR_KEY"
RPC_FALLBACK_URLS="https://rpc.ankr.com/arbitrum,https://arb1.arbitrum.io/rpc"
CU_BUDGET=10000000
```
