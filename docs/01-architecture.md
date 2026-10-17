# Architecture and pipeline

This is the current implementation, not the original platform blueprint.
[Configuration](configuration.md) separates runtime wiring from library APIs.

## Runtime processes

The Rust `logrix` binary has separate roles or an `all-in-one` composition:

```text
HTTP EVM RPC providers
          |
          v
RpcGateway -> EvmChainClient
    |                 ^
    v                 |
ingester -> live/backfill queues -> processor -> PostgreSQL
                                      |            |
                                      |            +-> API GraphQL/REST
                                      |            +-> LISTEN -> WebSocket subscribers
                                      +-> optional S3 live envelopes
                                      +-> reorg webhook jobs -> dispatcher -> HTTP targets
```

The ingester polls head height and schedules jobs; it does not extract event
logs. The processor consumes backfill before live jobs, fetches data, performs
handler work and writes results. Each processor starts its own lag reconciler.
There is no separate singleton Controller, listener/decoder CLI pair, or global
database-aware autoscaling coordinator.

`all-in-one` migrates and starts the same principal roles in one process, with
local leadership. Split-role deployments need a migration step first. Runtime
services are PostgreSQL plus RabbitMQ or SQS; there is no embedded SQLite or
production in-memory queue mode.

## Ports and concrete adapters

`logrix-core` defines `ChainPort`, `QueuePort`, `StorePort`, `BlobPort`, `SinkPort`,
and `LeaderElectionPort`, plus EVM-shaped domain models and typed errors.
Concrete database/queue/blob adapters implement those contracts. The API uses
`PostgresStore` directly for entity/transfer queries and subscriptions.

The boundaries are useful extension seams, not a claim that core is completely
vendor/chain independent: it contains EVM types and a concrete Kubernetes HTTP
elector. That elector is incomplete for safe HA acquisition/renewal. See
[core](../crates/logrix-core/README.md) before enabling multiple ingesters.

## Processing paths

| Stage | Live block envelope | Backfill range |
| --- | --- | --- |
| RPC | Fetch block/header and address-filtered logs | Fetch address-filtered logs for a numeric range |
| Continuity | In-memory parent/hash detector | Does not run the live envelope detector |
| Handler | Raw log JSON to WASM or declarative field rules | Same per-log handler facilities |
| Native decode | Built-in ERC-20 Transfer extraction | Built-in ERC-20 Transfer extraction |
| Persistence | Raw events/checkpoint plus separate handler/entity/transfer writes | Similar separate writes; checkpoint can advance beyond holes |
| Archive | Optional best-effort compressed envelope upload | Does not use the live-envelope archive path |

Manifest event-name lists are not general RPC topic filters or WASM dispatch.
Guests must filter topic0/address and dispatch inside `handle_event`.
Declarative expressions operate on raw log fields, not arbitrary decoded
`args.*`. The independent Rust ABI decoder and SDK codegen are not a universal
runtime ABI/proxy-discovery pipeline.

## Persistence and notification boundaries

- Raw event writes and their checkpoint share one PostgreSQL transaction.
- Entity writes, token transfers, engine state commit, queue acknowledgment,
  blob upload and webhook publication do not share that transaction.
- Engine state is in-memory. Warmup seeds stored entity JSON, not a general
  durable handler-state log. Replays of custom aggregate mutations need review.
- Rollback marks later rows reverted and rewinds checkpoint height/finality;
  it does not restore overwritten entity versions, old engine memory or the
  checkpoint's canonical fork hash.
- `pipeline_id` namespaces checkpoints, not all data. Use explicit deployment
  isolation rather than assuming it provides per-tenant event/entity storage.
- Entity notifications are best effort, subject to payload and broadcast limits,
  and are not a durable stream. Entity subscriptions and HTTP webhooks are
  separate facilities.

For exact semantics and errors, see [PostgreSQL](../crates/logrix-store-postgres/README.md),
[handlers](../crates/logrix-handlers/README.md), [reorgs](03-resilience-reorgs.md),
and [API](../crates/logrix-api/README.md).

## Extension status

Supported handler mechanisms are declarative rules and Wasmtime guests matching
the documented host ABI. The supplied SDK targets AssemblyScript. Other guest
languages require compatible exports/imports and their own verified toolchain.
There is no JavaScript-in-WASM runtime, remote gRPC plugin service, runtime
Rust dynamic-library loading, or automatic factory/proxy registration action.
Bulk SQD/HyperSync endpoint selection exists but transport is unimplemented.

Deployment resources and scalers are configuration templates, not proof of
production safety or dependency availability. See [deployment](../deploy/README.md).
