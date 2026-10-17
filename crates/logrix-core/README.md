# logrix-core

Shared domain types, async port traits, and errors for Logrix. Database, queue,
and EVM RPC implementations live in adapter crates. Core has no `sqlx`, `lapin`,
or AWS SDK dependency, but its Kubernetes elector does use `reqwest` directly.

## Prerequisites

Use the repository's Rust workspace and `Cargo.lock`. Domain types need no
external services. Async ports and electors are normally used with Tokio;
implementing a port uses `async-trait`. A Kubernetes API is needed only when
calling the Kubernetes elector.

## Minimal example

This synchronous example uses only `logrix-core`:

```rust
use logrix_core::{BlockRangeJob, ChainId, QueueMessage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chain: ChainId = "42161".parse()?;
    let range = BlockRangeJob::new(chain, 100, 109, "orders");
    assert_eq!(range.block_count(), 10);
    let (first, second) = range.split_half();
    assert_eq!(first.to_block, 104);
    assert_eq!(second.expect("second half").from_block, 105);
    let message = QueueMessage::BackfillRange(range);
    assert_eq!(message.attempt(), 1);
    Ok(())
}
```

## Public APIs, defaults, and validation

There is no global core configuration struct or YAML loader.

| API | Defaults and semantics |
|---|---|
| `ChainId::new(u64)` / `From<u64>` | Accept any `u64`, including zero; `Default` is zero. `FromStr` trims and parses a decimal `u64`, without checking a network registry. |
| `ChainId::is_mainnet()` / `is_testnet()` | Recognize only the constants listed in `src/domain/chain.rs`; unknown chains return false for both. |
| `BlockRef::new(number, hash)` | Stores the supplied number and hash. |
| `BlockEnvelope::new(...)` | Sets `kind = EnvelopeKind::Log`, an empty `extra` map, and the supplied logs; `timestamp` is Unix seconds. `with_kind` and `with_extra` customize metadata. |
| `LiveBlockJob::new(...)` / `BlockRangeJob::new(...)` | Generate UUID job IDs and start `attempt` at 1. Range bounds are inclusive; reversed ranges are accepted and have count zero. |
| `BlockRangeJob::split_half()` | Splits at the midpoint with new IDs and attempt 1; single-block or reversed ranges return the original clone and no second half. |
| `Checkpoint::new(...)` | Records `updated_at = Utc::now()`; height, hash, and `is_finalized` are supplied by the caller, without continuity/finality checks. |
| `MessageHandle::new(receipt, message)` | Sets `source_queue = None`; `with_queue` records the logical source queue. Receipt interpretation is adapter-specific. |
| `WebhookEndpoint::new(...)` | In `domain`: generates an ID/time, sets `is_active = true` and `max_retries = 0`. `with_retries` accepts an explicit count. No URL or secret validation. |
| `WebhookPayload::new(...)` | In `domain`: sets the current Unix timestamp in seconds; `WebhookDelivery` is a delivery record with `latency_ms` in milliseconds. |

These are mutable public data structures, not validation or persistence layers.
`EventLog::idempotency_key(chain)` uses `(chain_id, block_hash, log_index)`;
`BlockEnvelope::idempotency_key()` uses `(chain_id, block_hash)`. An adapter can
use a different database conflict key; PostgreSQL uses transaction hash for logs.

`QueueMessage` uses JSON fields `schema_version` and `payload`, with tags
`v1:live_block`, `v1:backfill_range`, and `v1:custom`. It is `non_exhaustive`, so
external Rust matches need a wildcard arm. Unknown JSON variant tags still fail
deserialization; version tags do not automatically provide future-version support.

## Port contracts and limits

All ports require `Send + Sync`; import traits to call adapter trait methods.

| Port | Contract |
|---|---|
| `ChainPort` | Chain ID, latest height, optional block reference/envelope, and address-filtered log queries. |
| `QueuePort` | Publish/consume one message, ack/nack, explicit dead-lettering, depth, and batch publish. Default `publish_batch` sends sequentially and stops on the first error. |
| `StorePort` | Atomically write raw events with a checkpoint, read a checkpoint, and roll back. Dynamic entity/transfer APIs belong to concrete adapters, not this trait. |
| `BlobPort` | Byte-buffer `put`, optional `get`, `delete`, and `exists`. Default `exists` calls `get`, which may download the object. |
| `SinkPort` | Emit event slices or a revert notification. |
| `LeaderElectionPort` | `try_acquire_or_renew`, cached `is_leader`, and `step_down`; actual coordination guarantees depend on the implementation. |

Traits provide no worker loops, automatic retries, payload-size limits, or
cross-adapter transactions. For example, PostgreSQL soft-reverts rows on rollback
rather than physically deleting them as the store trait's comment describes.

## Leader electors and environment ownership

`LocalLeaderElector::new()` starts as leader; derived `Default` starts false.
`try_acquire_or_renew()` always sets true, and `step_down()` sets false. Separate
instances do not coordinate.

`KubernetesLeaseElector::new(lease_name, namespace, identity, api_url, token)`
starts false and has the following resolution rules:

| Input | Resolution |
|---|---|
| API URL | Explicit `Some(url)`, otherwise `https://{KUBERNETES_SERVICE_HOST}:{KUBERNETES_SERVICE_PORT}`; absent values default to `127.0.0.1` and `443`. |
| Token | Explicit token, then `KUBERNETES_BEARER_TOKEN`, then the in-cluster service-account token file. File-read failure leaves no token. |
| Lease name, namespace, identity | Required constructor arguments; core does not parse `LEASE_NAME`, `K8S_NAMESPACE`, or `POD_NAME`. |
| Lease duration | `LeaseSpec` deserialization defaults a missing duration to 15 seconds; the elector does not enforce it. |

The current elector performs only an HTTP GET. A successfully decoded Lease
makes it leader only when `holderIdentity` matches its identity. It does not
create/update/renew a Lease, evaluate expiry, or perform resource-version CAS.
Transport errors return `Ok(false)`, but non-success HTTP responses and invalid
Lease JSON fall through to `Ok(true)`. `step_down` only changes local state.
The HTTP client accepts invalid TLS certificates and sets no explicit request
timeout. This implementation does not establish exclusive distributed leadership.

The CLI owns `ENABLE_LEADER_ELECTION`, `LEASE_NAME`, `K8S_NAMESPACE`, and
`POD_NAME` mappings. Core itself reads only the Kubernetes variables above;
adapter settings and YAML/CLI bindings are documented with their owners.

## Errors

`LogrixResult<T>` is `Result<T, LogrixError>`. Errors carry `class`, `source`,
`message`, and optional chain/block/job context, set with `with_chain_id`,
`with_block_number`, and `with_job_id`. Classes are `Transient`, `RateLimited`
(optional `retry_after: Duration`), `Shrinkable`, `Permanent`, `Integrity`, and
`Fatal`. `is_retryable()` is true for the first three; it does not execute retries.
`ErrorSource` identifies the subsystem (RPC, queue, database, blob, plugin,
configuration, reconciler, webhook, handler, or internal).

## Tests and coverage

From the repository root, with dependencies already cached:

```sh
cargo test -p logrix-core --offline --locked
```

`domain_tests.rs` covers message round trips, ranges, idempotency keys,
checkpoints, metadata, and errors. `ports_tests.rs` exercises default batch/exists
methods and port/model helpers. `leader_tests.rs` covers local lifecycle and an
unreachable loopback API. It does not test HTTP error/invalid-JSON leadership,
Lease expiry, competing Kubernetes clients, or real service coordination.

## Related documentation

- [Configuration reference](../../docs/configuration.md)
- [Architecture](../../docs/01-architecture.md)
- [PostgreSQL](../logrix-store-postgres/README.md), [RabbitMQ](../logrix-queue-rabbitmq/README.md), [SQS](../logrix-queue-sqs/README.md), and [S3](../logrix-blob-s3/README.md)
- [Testkit](../logrix-testkit/README.md) and [resilience](../logrix-resilience/README.md)
