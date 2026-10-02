# Resilience, Backpressure, and Blockchain Reorgs

## 1. The Unified Error Taxonomy

Every error encountered across the RPC, Queue, Database, Blob Store, or Plugins is classified once at the edge into one of six canonical error classes:

```rust
pub enum ErrorClass {
    /// Network blips, temporary 5xx, node briefly behind -> retry with exponential backoff & jitter
    Transient,
    /// 429 Too Many Requests -> honor Retry-After, shift load to alternative providers
    RateLimited { retry_after: Option<Duration> },
    /// eth_getLogs range too large -> halve chunk size and retry immediately
    Shrinkable,
    /// Unparseable event, bad handler logic -> park job in DLQ with context, do not loop
    Permanent,
    /// Parent hash mismatch, missing checkpoint -> pause partition, trigger reorg/reconciliation
    Integrity,
    /// Invalid chain ID, bad database credentials -> halt process and trigger high-priority alert
    Fatal,
}
```

---

## 2. Reorg Detection & Automatic Rollback

Reorgs are a normal occurrence in EVM blockchains. Logrix handles them deterministically:

```text
       Block 101 (Hash: 0xAAA) ──► Block 102 (Hash: 0xBBB) ──► Tip: Block 103 (Hash: 0xCCC)
                                            ▲
                                            │ Reorg detected! New Tip points to 0xDDD
                                            │
                                   Block 102 (Hash: 0xDDD) ──► Block 103 (Hash: 0xEEE)
```

### Detection:
The `logrix-listener` tracks the parent hash of each incoming block header against the stored hash in the local database. If `new_header.parent_hash != tip_header.hash`, a reorg is signaled.

### Rollback Protocol:
1. **Pause Ingestion:** Listener notifies the singleton Controller; new block decoding is paused.
2. **Locate Fork Point:** Traverses backwards until a common block ancestor is found.
3. **Atomic Database Rollback:** In a single database transaction:
   - Deletes all event rows with `block_number > fork_block`.
   - Reverts state updates (for stateful handlers).
   - Resets checkpoints to `fork_block`.
4. **Dispatch Revert Notifications:** Emits `event.reverted` webhooks for all rolled-back events so external systems stay synchronized.
5. **Re-Index Canon:** Re-queues the canonical branch for normal decoding.

---

## 3. Two-Layer Backpressure (AIMD)

When decoders produce database writes faster than PostgreSQL can commit them, or when RPC providers rate-limit requests:

1. **In-Pod Adaptive Concurrency (AIMD):**
   - Each decoder pod measures write latency and RPC response status.
   - Upon encountering latency spikes or 429s, it multiplicatively cuts its local worker threads ($$C_{new} = C / 2$$).
   - When latency stabilizes, it additively ramps up workers ($$C_{new} = C + 1$$).

2. **Cross-Pod Controller Regulation:**
   - The Controller monitors overall PostgreSQL connection pool saturation and queue backlog.
   - It clamps the KEDA scaling target `logrix_desired_replicas` so that autoscaling pods never overwhelm database capacity.

---

## 4. Self-Healing: The Reconciler Loop

The Controller runs a continuous background reconciliation loop:
- Audits stored checkpoint ranges against the canonical chain tip.
- Identifies any missing gaps caused by dropped messages or node network partitions.
- Re-enqueues missing block ranges with high priority.
- Verifies hash continuity across stored blocks to catch silent disk or RPC corruption.
