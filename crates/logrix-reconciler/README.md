# logrix-reconciler

Checkpoint-lag backfill scheduling and in-memory chain-continuity detection.
This library depends on `ChainPort`, `StorePort`, and `QueuePort`; it does not
select adapters or load environment variables/manifests itself.

## Prerequisites and minimal usage

Embedding requires Tokio and implementations of those three ports. The CLI uses
the RPC gateway, migrated PostgreSQL, and RabbitMQ or SQS. In an async Rust
application depending on `logrix-core` and `logrix-reconciler`:

```rust
use logrix_core::{domain::ChainId, error::LogrixResult,
    ports::{ChainPort, QueuePort, StorePort}};
use logrix_reconciler::{GapReconciler, ReorgDetector};
use std::{sync::Arc, time::Duration};

async fn schedule_lag(
    store: Arc<dyn StorePort>, queue: Arc<dyn QueuePort>, chain: Arc<dyn ChainPort>,
) -> LogrixResult<u64> {
    let id = ChainId::new(42161);
    GapReconciler::new(id, store, queue, chain, Duration::from_secs(30))
        .with_lag_threshold(50)
        .with_chunk_size(10)
        .check_and_reconcile().await
}

fn new_detector(chain: Arc<dyn ChainPort>) -> ReorgDetector {
    ReorgDetector::new(ChainId::new(42161), chain, 128)
        .with_max_reorg_depth(256)
}
```

Retain the returned detector across a live stream and pass each envelope to
`detector.check_envelope(&envelope).await`. Handle its `Continuous`,
`GapDetected`, or `ReorgDetected` outcome explicitly; checking alone does not
invoke a rollback handler or queue gap jobs.

## Configuration and defaults

| Public entry point | Library behavior/default | CLI-owned mapping/default |
|---|---|---|
| `GapReconciler::new(..., interval)` | Caller supplies nonzero `Duration`; no default interval | `--reconciler-interval-secs` / `RECONCILER_INTERVAL_SECS`: 30 seconds |
| `.with_lag_threshold(blocks)` | 10 blocks; triggers only when lag is strictly greater | `--reconciler-lag-threshold` / `RECONCILER_LAG_THRESHOLD`: 50 blocks |
| `.with_chunk_size(blocks)` | Unset: one job for the entire discovered range; explicit zero clamps to 1 | `--reconciler-chunk-size` / `RECONCILER_CHUNK_SIZE`: unset, not inherited from `CHUNK_SIZE` |
| `RollingBlockBuffer::new(capacity)` | Caller supplies capacity, clamped to at least 1 | `--ring-buffer-depth` / `RING_BUFFER_DEPTH`: 128 block references |
| `RollingBlockBuffer::default_depth()` | 128 references | Not a finality guarantee |
| `ReorgDetector::new(..., capacity)` | Maximum ancestor walk is `max(capacity * 2, 128)` blocks | 256 with CLI's default capacity of 128 |
| `.with_max_reorg_depth(blocks)` | Explicit search bound | No CLI flag |

The manifest parser accepts `indexing.ringBufferDepth`,
`reconcilerIntervalSecs`, `reconcilerChunkSize`, and `reconcilerLagThreshold`,
but `ProcessorConfig::from_cli` does not apply them. SDK `export-values` maps
these to Helm configuration, whose environment feeds the CLI. Reading a project
manifest directly is a different path. See [configuration](../../docs/configuration.md).

## Scheduling and detection boundaries

- With no checkpoint, reconciliation does nothing. If
  `head > checkpoint + lag_threshold`, it queues the inclusive interval
  `checkpoint + 1 ..= head - 1` on `QueueType::Backfill`. It checks checkpoint
  lag, not per-block coverage; holes below the checkpoint are not discovered.
- A process-local dispatch marker suppresses previously scheduled ranges and
  resets when an observed checkpoint moves backward. It is neither durable nor
  shared between processor replicas. Lost in-flight jobs are not automatically
  redispatched while that marker remains ahead. Partial publish failure can
  cause duplicate jobs on a later scan.
- The loop's first timer tick is immediate. Scan failures are logged and the
  next tick retries. `run_loop_with_shutdown` accepts a future yielding `()` and
  observes shutdown between scans; a scan already running is awaited. A zero
  interval panics in Tokio.
- The detector starts with an empty, non-persistent buffer and accepts its first
  envelope. Contiguous parent mismatches invoke bounded RPC ancestor search;
  forward jumps report gaps. Older/duplicate heights are compared only if still
  buffered; an unbuffered older block can be reported continuous. The CLI runs
  this detector for live envelopes, not range-log backfills.

## Rollback and errors

`ReorgHandler::execute_rollback` calls `StorePort::rollback_to_block`, rewinds the
buffer, then publishes a custom webhook job. These steps are not one transaction.
In the PostgreSQL consumer, the SQL transaction marks later raw events, transfers,
and current entity rows reverted and rewinds the checkpoint number/finality. It
does not restore older entity versions, rewind the handler engine's state, or
replace the checkpoint hash with `fork_hash`.

A queue publish failure is returned after database/buffer changes have occurred.
The emitted job type is `event.reverted`; its nested data says `event: chain.reorg`.
The HTTP dispatcher uses `event.reverted` as the outer event, while the API
registration vocabulary includes `reorg`; see [webhooks](../logrix-webhook/README.md).

Direct scans and rollbacks propagate typed port errors. Ancestor search returns
integrity errors for missing headers or exhausted depth, and propagates RPC
errors; reaching genesis returns `(0, B256::ZERO)`. This is bounded detection and
replay scheduling, not universal rollback or historical-state recovery.

## Tests and related code

From the workspace root with cached dependencies:

```sh
cargo test -p logrix-reconciler --offline --locked
```

[Tests](tests/) use testkit ports for continuity, ancestor limits, rollback
orchestration, lag thresholds, and chunking. See [source](src/),
[CLI](../logrix-cli/README.md), [PostgreSQL rollback](../logrix-store-postgres/src/rollback.rs),
and [configuration](../../docs/configuration.md).
