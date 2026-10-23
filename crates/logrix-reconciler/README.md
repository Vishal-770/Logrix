# logrix-reconciler

Continuous gap reconciliation and chain reorganization detection engine for Logrix.

## Overview
Ensures deterministic data integrity in the face of blockchain forks, temporary node desyncs, and RPC rate-limit dropouts.

## Key Subsystems
1. **Parent-Hash Ring Buffer (`buffer.rs`)**:
   - In-memory rolling buffer tracking recent block hashes and parent continuity.
2. **Reorg Detector (`detector.rs`, `ancestor.rs`)**:
   - Compares incoming block parent hashes against the local chain state to detect fork points.
   - Bounded ancestor walking (`with_max_reorg_depth`) prevents unbounded network RPC loops during anomalous forks.
3. **Rollback Handler (`rollback.rs`)**:
   - Executes atomic SQL rollbacks by marking affected entities as soft-deleted (`is_reverted = TRUE`).
   - Publishes `event.reverted` webhook jobs into `QueueType::Webhook`.
4. **Gap Reconciler (`reconciler.rs`)**:
   - Periodically checks indexed block ranges against PostgreSQL checkpoints and schedules backfill fill-in jobs for missing ranges.
   - Configurable lag threshold (`with_lag_threshold`) and chunk size (`with_chunk_size`).
   - Supports cancellation-safe graceful shutdown loops (`run_loop_with_shutdown`).

---

## Configuration & Tuning

### 1. Declarative Manifest Configuration (`logrix.yaml`)

End users configure gap reconciliation and chain reorganization detection parameters under `indexing:` in `logrix.yaml`:

```yaml
indexing:
  ringBufferDepth: 128            # Rolling parent-hash ring buffer size for reorg detection
  reconcilerIntervalSecs: 30      # Periodic background gap audit interval in seconds
  reconcilerChunkSize: 10         # Chunk size when backfilling discovered block gaps
  reconcilerLagThreshold: 25      # Block lag behind chain head required to trigger backfill
```

| `logrix.yaml` Key | Type | Default Value | Required? | Description |
|---|---|---|---|---|
| `indexing.ringBufferDepth` | `integer` | `128` blocks | Optional | Rolling buffer depth for ancestor tracking & reorg detection |
| `indexing.reconcilerIntervalSecs` | `integer` | `30` seconds | Optional | Background gap inspection interval |
| `indexing.reconcilerChunkSize` | `integer` | `10` blocks | Optional | Sliced job size when reconciling gap intervals |
| `indexing.reconcilerLagThreshold` | `integer` | `50` blocks | Optional | Block distance behind chain head before scheduling backfill |

### 2. Environment Variables & CLI Options

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `RECONCILER_INTERVAL_SECS` | `--reconciler-interval-secs` | `30` seconds | Optional | Background gap inspection interval |
| `RECONCILER_CHUNK_SIZE` | `--reconciler-chunk-size` | `CHUNK_SIZE` (default: 10) | Optional | Block range chunk size when reconciler splits missing block intervals |
| `RECONCILER_LAG_THRESHOLD` | `--reconciler-lag-threshold` | `50` blocks | Optional | Minimum block lag behind chain head required to trigger gap filling (`DEFAULT_LAG_THRESHOLD = 50`) |
| `RING_BUFFER_DEPTH` | `--ring-buffer-depth` | `128` blocks | Optional | In-memory parent-hash ring buffer size for reorg detection (`DEFAULT_RING_BUFFER_CAPACITY = 128`) |

### 3. Programmatic Configuration

```rust
use logrix_reconciler::{
    GapReconciler, ReorgDetector,
    DEFAULT_LAG_THRESHOLD, DEFAULT_HEAD_OFFSET,
    DEFAULT_RING_BUFFER_CAPACITY, DEFAULT_MAX_REORG_DEPTH,
};
use std::time::Duration;

// 1. Configure Gap Reconciler with custom lag threshold and chunk size
let reconciler = GapReconciler::new(
    chain_id,
    store.clone(),
    queue.clone(),
    chain_client.clone(),
    Duration::from_secs(30),
)
.with_lag_threshold(DEFAULT_LAG_THRESHOLD)     // Trigger when >= 50 blocks behind head
.with_chunk_size(10);                          // Slice backfill into 10-block job chunks

// 2. Configure Reorg Detector with bounded ancestor depth
let detector = ReorgDetector::new(chain_id, chain_client, DEFAULT_RING_BUFFER_CAPACITY)
    .with_max_reorg_depth(DEFAULT_MAX_REORG_DEPTH); // Maximum ancestor search depth (default: 128)
```

