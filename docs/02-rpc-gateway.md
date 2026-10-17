# The Cost-Aware RPC Gateway & Bulk Acceleration

## Overview
In blockchain indexing, the RPC layer is the single largest operating expense, the most common point of failure, and the primary throughput bottleneck.

Logrix never allows decoders or listeners to call an RPC provider directly. All traffic is routed through the **RPC Gateway**:

```text
Listener (Live)       Backfiller (History)       Reconciler
       │                       │                      │
       ▼                       ▼                      ▼
┌───────────────────────────────────────────────────────────┐
│                        RPC GATEWAY                        │
│  • Scheduler: live priority > backfill, fair sharing      │
│  • CU Budget Tracker: tracks costs in native provider CU  │
│  • Provider Pool: health checks, failover, latency matrix │
│  • Error Classifier: transient vs rate-limited vs shrink  │
│  • Deduplication: single-flight in-flight call merging    │
│  • Learned Range Caps: persists max getLogs block windows │
│  • Bulk Stream Fast-Path: SQD / HyperSync integration     │
└──────┬───────────────────────┬────────────────────┬───────┘
       ▼                       ▼                    ▼
  Provider A              Provider B         Bulk Data Stream
  (Alchemy)               (Infura)           (SQD Portal / HyperSync)
```

---

## 1. Hybrid Backfill Architecture (100x Speedup)

Standard JSON-RPC nodes limit `eth_getLogs` to 2,000–10,000 block ranges and enforce aggressive requests-per-second caps. Indexing years of historical data over plain JSON-RPC can take days.

Logrix uses a **Hybrid Source Model**:
1. **Live Indexing:** Always uses standard EVM JSON-RPC (WebSocket `newHeads` + polling fallback) to track the chain tip with sub-second latency.
2. **Historical Backfill (Fast-Path):** When available, Logrix connects to open, high-throughput bulk block streams (such as SQD Portal or HyperSync) to download millions of pre-filtered event logs in seconds.
3. **Automatic Fallback:** If a bulk stream is unavailable for a chain or private network, Logrix seamlessly falls back to adaptive chunked JSON-RPC calls across its provider pool.

---

## 2. Compute Unit (CU) Budget Tracking & Dry Runs

Instead of counting requests, Logrix measures cost using the provider's native billing weights:

| RPC Method | Approximate Weight |
| :--- | :--- |
| `eth_blockNumber` | ~10 CU |
| `eth_getBlockByNumber` | ~20 CU |
| `eth_getLogs` | ~60–150 CU |

### Dry Run Cost Estimator
Before launching a large historical sync, users run:

```bash
logrix backfill --dry-run
```

Output:
```text
Backfill Plan:
  Chain:              Arbitrum Sepolia (ID: 421614)
  Block Range:        12,000,000 -> 14,500,000 (2,500,000 blocks)
  Estimated Calls:    1,250 getLogs chunks
  Estimated CU:       93,750 Compute Units
  Estimated Cost:     ~$0.05 USD
  Estimated Time:     42 seconds (Bulk Stream) / 8 minutes (JSON-RPC Pool)
Proceed? [y/N]
```

---

## 3. Adaptive Chunking & Learned Limits

When querying providers that cap block ranges, Logrix employs an adaptive search window:
- If a provider returns `"query exceeds max results"` or `"block range too large"`, Logrix cuts the window in half ($$W_{new} = W / 2$$) and retries immediately.
- On consecutive successful, sparse queries, Logrix expands the window slowly ($$W_{new} = \min(W \times 1.25, W_{preset})$$).
- Learned limits are cached per provider and chain so future jobs never trigger the same error twice.
