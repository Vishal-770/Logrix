# logrix-core

Core domain models, ports, and common errors for the Logrix indexer platform.

## Overview
This crate defines the foundational traits and data structures adhering strictly to hexagonal clean architecture and ports-and-adapters principles. It remains completely independent of concrete database, queue, and RPC network drivers.

## Core Domain Types
- `ChainId`: Type-safe EVM network identifier with well-known constants (`ETHEREUM`, `OPTIMISM`, `BNB`, `POLYGON`, `BASE`, `ARBITRUM_ONE`, `SEPOLIA`, etc.), network kind classification (`is_mainnet()`, `is_testnet()`), and parsing (`FromStr`, `as_u64()`).
- `BlockRef` & `BlockEnvelope`: Immutable block header and event payload envelopes supporting Open/Closed Principle (OCP) extension via `extra` metadata maps, `has_logs()`, and deterministic idempotency keys.
- `QueueType`: The 4 logical indexing queues (`Live`, `Backfill`, `Webhook`, `DeadLetter`).
- `QueueMessage`: Version-tagged queue envelopes (`LiveBlockJob`, `BlockRangeJob`, `Custom`) supporting backward/forward wire compatibility.
- `MessageHandle`: Receipt metadata with mandatory `source_queue` lineage for safe acknowledgements.
- `Checkpoint` and `BlockRangeJob`: Fallible `try_new` constructors reject zero block hashes and inverted ranges.
- `WebhookEndpoint`, `WebhookPayload`, & `WebhookDelivery`: Models for outbound HTTP event subscriptions, signing envelopes, and delivery audit logs.

## Ports (Interfaces)
- `QueuePort`: Pluggable messaging trait implemented by RabbitMQ (`logrix-queue-rabbitmq`) and AWS SQS (`logrix-queue-sqs`), featuring default batch publishing (`publish_batch`).
- `ChainPort`: Blockchain interaction contract for block discovery, header queries, log extraction, and chain ID inspection (`chain_id()`). Implemented by `logrix-chain-evm` and `logrix-rpc-gateway`.
- `BlobPort`: Object and blob storage abstraction with key-value storage and default existence checks (`exists`). Implemented by `logrix-blob-s3`.
- `StorePort`: Database persistence contract for entities, checkpoints, idempotency guards, and reorg rollbacks. Implemented by `logrix-store-postgres`.
- `LeaderElectionPort`: Pure high-availability leader election contract. The local elector is in core; the Kubernetes Lease adapter lives in `logrix-cli`.

## Error Model
- `ErrorClass`: Canonical error classification (`Transient`, `RateLimited`, `Shrinkable`, `Permanent`, `Integrity`, `Fatal`, `Validation`) driving automatic resilience policies.
- `ErrorSource`: Subsystem error origin markers (`ChainRpc`, `Queue`, `Database`, `BlobStore`, `Plugin`, `Config`, `Reconciler`, `Webhook`, `Handler`, `Internal`).
- `LogrixError`: Context-rich unified error carrier with fluent context enrichment (`with_chain_id`, `with_block_number`, `with_job_id`) and structural equality (`PartialEq`, `Eq`).

---

## Configuration Architecture

In accordance with Hexagonal Architecture:
1. `logrix-core` specifies the port contracts and error types, remaining free of external I/O driver dependencies (no HTTP, `reqwest`, `sqlx`, `lapin`, or AWS SDKs).
2. Each adapter crate provides its own strongly-typed configuration struct (e.g. `PostgresConfig`, `RabbitMQConfig`, `SqsClientConfig`, `EvmClientConfig`, `GatewayConfig`, `WebhookConfig`, `ApiConfig`).
3. Adapter configuration structs provide sensible production defaults for every optional parameter and enforce typed validation.
4. The CLI runner (`logrix-cli`) binds environment variables and command-line arguments to these adapter configuration structs.

## Safety Contracts

- Use `Checkpoint::try_new` when handling runtime data. It rejects `B256::ZERO`, which cannot identify a canonical block for reorg recovery.
- Use `BlockRangeJob::try_new` when handling runtime ranges. It returns a typed validation error for `from_block > to_block`.
- Construct every `MessageHandle` with its concrete `QueueType`. Queue adapters use that lineage for acknowledgement and retry routing.
- Kubernetes Lease failures are fail-closed. The CLI adapter validates TLS, bounds request time, and renews with the Lease `resourceVersion`.
