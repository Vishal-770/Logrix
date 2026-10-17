# logrix-testkit

In-memory implementations of three core ports: `MockQueuePort`, `MockChainPort`,
and `MockStorePort`. Use them to exercise application logic without network
services. This crate does not implement every core port or export a reusable
adapter-conformance harness; its compliance scenarios are integration tests.

## Prerequisites

Use the repository's Rust workspace and lockfile. Application tests need
`logrix-testkit`, `logrix-core`, and Tokio with test macros/runtime support.
The mocks have Rust dependencies but need no Docker, database, broker, or RPC
endpoint. Data is process-local and held in memory without capacity bounds.

## Minimal example

```rust
use logrix_core::{BlockRangeJob, ChainId, QueueMessage, QueuePort, QueueType};
use logrix_testkit::MockQueuePort;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queue = MockQueuePort::new();
    let message = QueueMessage::BackfillRange(BlockRangeJob::new(
        ChainId::ETHEREUM,
        100,
        109,
        "test-partition",
    ));
    queue.publish(QueueType::Backfill, &message).await?;
    let handle = queue
        .consume(QueueType::Backfill)
        .await?
        .expect("published message");
    assert_eq!(handle.message, message);
    queue.ack(&handle).await?;
    assert_eq!(queue.acked.lock().await.len(), 1);
    Ok(())
}
```

For an application test, use `#[tokio::test]` on an async test function and import
the relevant core traits (`QueuePort`, `ChainPort`, or `StorePort`).

## Public setup APIs and defaults

There is no configuration builder, environment parser, or YAML loader. All
three mocks implement `new`, `Default`, and `Clone`. Clones share the underlying
maps/queues through `Arc<tokio::sync::Mutex<_>>`.

| Mock | Setup and inspection |
|---|---|
| `MockQueuePort::new()` | Starts with empty queues, in-flight map, and public `acked`, `nacked`, `dead_lettered` history vectors. `clear().await` resets all five collections, including in-flight messages. |
| `MockChainPort::new(chain_id)` | Starts at head block 0 with no blocks. `add_block(envelope).await` inserts/replaces by height and raises the head only if the new height is higher. Public `head_block` can be set explicitly; public `chain_id` is a value, not a shared mutex. Derived `Default` uses chain ID 0. |
| `MockStorePort::new()` | Starts with no events/checkpoints and an empty public `rollbacks` vector. `total_events()` and `get_events_for_chain(chain_id)` inspect stored logs. Event iteration order is unspecified. |

The mocks do not validate chain IDs, envelope contents, receipt ownership, or
checkpoint continuity/finality. Queue history stores receipt IDs and flags/reasons,
not full message snapshots.

## Behavior and differences from real adapters

### MockQueuePort

- `publish` clones a message onto a per-`QueueType` FIFO; default `publish_batch`
  calls it sequentially. `consume` immediately removes one message into an
  in-flight map and returns a UUID receipt. Empty queues return `Ok(None)`.
- Returned handles have `source_queue = None`. The mock resolves source queues
  through its in-flight map; SQS instead requires queue-scoped receipts.
- `ack` removes the in-flight entry. `nack(true)` puts it at the front of its
  original queue; `nack(false)` drops it. SQS `nack(false)` instead hides a message
  for 300 seconds before it can reappear.
- `dead_letter` moves a known in-flight message into the dead-letter FIFO and
  records the reason in history. It does not increment attempts or modify the
  payload. Unknown/repeated receipts still return `Ok(())` and append history.
- `depth` counts only queued messages. There is no visibility timeout, delivery
  expiry/redelivery, serialization, broker confirmation, or automatic retry limit.
  Abandoned in-flight messages stay there until an explicit operation or `clear`.

### MockChainPort

- `get_block_by_number` and `fetch_block_envelope` return supplied data or
  `Ok(None)` for an unknown height. `get_latest_block_number` reads `head_block`.
- `fetch_logs(from, to, addresses)` walks an inclusive height range, filtering
  by address unless the address slice is empty. Missing heights are skipped.
- `fetch_block_envelope(number, addresses)` ignores the address argument and
  returns the stored envelope unchanged. It does not synthesize transactions,
  receipts, traces, or logs; populate envelopes explicitly. Chain mismatches and
  forks are not validated, and inserting a lower height never lowers the head.

### MockStorePort

- `write_events_and_checkpoint` holds both store locks while inserting events
  and the checkpoint. It keys events by `(chain_id, block_hash, log_index)` using
  the checkpoint's chain ID. PostgreSQL uses `(chain_id, tx_hash, log_index)`.
- A checkpoint replaces the previous value unconditionally, including an older
  height; PostgreSQL normally advances with `GREATEST`. There is no pipeline ID,
  dynamic entity storage, token-transfer table, or notification stream here.
- `rollback_to_block` physically removes later events for the selected chain,
  adjusts an existing checkpoint's height, clears its finality flag, and records
  the rollback. It does not replace its hash/timestamp or create a missing
  checkpoint. PostgreSQL soft-reverts rows instead.

These differences make the mocks useful for control-flow assertions, while
service-specific delivery, transactions, and recovery still need adapter tests.

## Errors and configuration ownership

All current mock port operations return `Ok`; there are no failure-injection
settings. Missing blocks/checkpoints use `None`, and empty log queries use an
empty vector. To test typed error handling, implement a purpose-built core port
that returns the desired `LogrixError`. No CLI or environment variable changes
mock behavior; job attempts, block heights, and timestamps come from supplied
core models (timestamps use Unix seconds where applicable).

## Tests and coverage

From the repository root, with dependencies already cached:

```sh
cargo test -p logrix-testkit --offline --locked
```

`compliance_tests.rs` covers queue consume/requeue/dead-letter flow, event and
checkpoint writes/rollback, and chain block/log lookup. `heavy_indexer_test.rs`
uses the reconciler with synthetic blocks 101-105, injects a fork, rolls back,
and replays the new block. It is an in-memory pipeline scenario, not a broker,
database, network-chaos, or throughput benchmark. Those tests do not establish
behavioral equivalence to PostgreSQL, RabbitMQ, SQS, or a real EVM provider.

## Related documentation

- [Configuration reference](../../docs/configuration.md)
- [Core contracts](../logrix-core/README.md) and [reconciler](../logrix-reconciler/README.md)
- [PostgreSQL](../logrix-store-postgres/README.md), [RabbitMQ](../logrix-queue-rabbitmq/README.md), and [SQS](../logrix-queue-sqs/README.md)
