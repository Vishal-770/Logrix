# Resilience, reconciliation, and reorg boundaries

This guide describes mechanisms and their boundaries, not an exactly-once,
zero-loss, or universal recovery guarantee.

## Errors and retries

`ErrorClass` has `Transient`, `RateLimited { retry_after }`, `Shrinkable`,
`Permanent`, `Integrity`, and `Fatal` variants. Classification helpers do not
automatically retry, dead-letter, stop a process, or rewind state. Each caller
chooses its behavior; see [core](../crates/logrix-core/README.md).

The [resilience primitives](../crates/logrix-resilience/README.md) provide
breakers, token buckets, a retry budget and retry-delay calculation. Their
defaults are not the same as EVM client or supervised gateway policy. The caller
must execute requests, sleep, and report breaker outcomes.

Queue transports have different semantics:

- [RabbitMQ](../crates/logrix-queue-rabbitmq/README.md) uses pull consumption,
  explicit acknowledgments and explicit publish-then-ack dead-letter movement.
  The adapter does not impose an automatic maximum retry count/DLX policy.
- [SQS](../crates/logrix-queue-sqs/README.md) uses receipt handles and visibility.
  Buffered extra messages receive 12-hour visibility; there is no processing
  heartbeat. `nack(false)` delays visibility 300 seconds rather than discarding.
- Common processor failures requeue jobs. Replays and duplicate deliveries are
  possible; SQL row keys do not make arbitrary handler aggregates replay-safe.

## Backpressure

The ingester compares live plus backfill queue depth with `MAX_QUEUE_DEPTH`.
At or above the threshold it waits; failed depth reads become zero. This limits
scheduling pressure, not database connections, queue storage or all process memory.
Zero depth limit throttles even empty queues.

Adaptive range sizing responds to RPC results, not a global database-write
latency controller. Processor code handles backfill before live work. KEDA/HPA
templates scale from their configured queue/CPU signals; they do not coordinate
RPC budgets or database pool capacity. Size per-pool connection limits and
replica caps together; state and budgets are process-local.

## Live continuity and reorgs

Each processor's `ReorgDetector` keeps an in-memory rolling block-reference
buffer. Default CLI capacity is 128; default ancestor walk limit is 256 for
that capacity. The first envelope initializes the buffer. Gaps, duplicates,
older references and parent mismatches follow different detector branches;
range-log backfills do not traverse this live path.

On a detected fork, rollback orchestration:

1. Calls the store's rollback operation.
2. Rewinds the in-memory buffer.
3. Publishes a custom `event.reverted` webhook job with nested `chain.reorg` data.

These are not one database/queue transaction. A publication failure can follow
successful database mutation. PostgreSQL's rollback transaction marks later
raw events, transfers and current entities reverted, and adjusts checkpoint
height/finality. It does not restore previous entity versions, handler memory,
or checkpoint hash. There is no global pause/lock coordinating other writers.

Notification is a summary job, not one durable record per reverted event.
API subscription vocabulary includes `reorg` but the producer uses
`event.reverted`; empty endpoint event lists receive all jobs, whereas literal
`*` does not. No normal entity/checkpoint webhook producer is wired merely
because those event names can be registered. Read the
[webhook contract](../crates/logrix-webhook/README.md) before depending on it.

## Checkpoint-lag reconciliation

Each processor's reconciler compares a checkpoint with chain head. No checkpoint
means no scheduled work. If lag is strictly greater than the threshold (CLI 50),
it schedules checkpoint+1 through head-1. The CLI interval defaults to 30 seconds;
an unset reconciler chunk publishes the whole range. `CHUNK_SIZE` does not supply
its default.

Dispatch tracking is in-memory and suppresses already scheduled ranges; it is
not a durable acknowledgment of successful coverage. A highest-height checkpoint
does not establish contiguous indexing, so holes below it are not found. This
is lag repair, not an arbitrary historical gap or corruption audit.

## Shutdown and HA

Standalone loops observe signals at different await points. All-in-one returns
after a signal or task exit without joining/draining all workers; API serving
has no configured graceful-shutdown future, and webhook processing can be
cancelled in flight. Do not describe shutdown as a universal atomic flush.

The Kubernetes elector does not safely acquire/renew/expire Leases; it is not
an HA guarantee. Use [deployment precautions](../deploy/README.md) for one
ingester with no update overlap. Multiple processors also do not establish shared
handler/reconciler state coordination.

See [reconciler](../crates/logrix-reconciler/README.md),
[PostgreSQL](../crates/logrix-store-postgres/README.md),
[handlers](../crates/logrix-handlers/README.md), and [configuration](configuration.md)
for public controls, errors and the tests that actually exist.
