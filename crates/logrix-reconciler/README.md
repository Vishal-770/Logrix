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

### Environment Variables & CLI Options

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `RECONCILER_INTERVAL_SECS` | `--reconciler-interval-secs` | `30` seconds | Optional | Background gap inspection interval |
| `RECONCILER_CHUNK_SIZE` | `--reconciler-chunk-size` | `CHUNK_SIZE` (default: 10) | Optional | Block range chunk size when reconciler splits missing block intervals |
| `RECONCILER_LAG_THRESHOLD` | `--reconciler-lag-threshold` | `50` blocks (CLI) / `10` (Direct) | Optional | Minimum block lag behind chain head required to trigger gap filling |
| `RING_BUFFER_DEPTH` | `--ring-buffer-depth` | `128` blocks | Optional | In-memory parent-hash ring buffer size for reorg detection |

### Programmatic Configuration

```rust
use logrix_reconciler::{GapReconciler, ReorgDetector};
use std::time::Duration;

// 1. Configure Gap Reconciler with custom lag threshold and chunk size
let reconciler = GapReconciler::new(
    chain_id,
    store.clone(),
    queue.clone(),
    chain_client.clone(),
    Duration::from_secs(30),
)
.with_lag_threshold(25)     // Trigger when >= 25 blocks behind head
.with_chunk_size(10);        // Slice backfill into 10-block job chunks

// 2. Configure Reorg Detector with bounded ancestor depth
let detector = ReorgDetector::new(chain_id, buffer)
    .with_max_reorg_depth(64); // Maximum ancestor search depth (default: 128)
```
