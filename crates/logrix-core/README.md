# logrix-core

Core domain models, ports, and common errors for the Logrix indexer platform.

## Overview
This crate defines the foundational traits and data structures adhering strictly to hexagonal clean architecture and ports-and-adapters principles. It remains completely independent of concrete database, queue, and RPC network drivers.

## Core Domain Types
- `ChainId`: Type-safe EVM network identifier with well-known constants (`ETHEREUM`, `OPTIMISM`, `BNB`, `POLYGON`, `BASE`, `ARBITRUM_ONE`, `SEPOLIA`, etc.), network kind classification (`is_mainnet()`, `is_testnet()`), and parsing (`FromStr`, `as_u64()`).
- `BlockRef` & `BlockEnvelope`: Immutable block header and event payload envelopes supporting Open/Closed Principle (OCP) extension via `extra` metadata maps, `has_logs()`, and deterministic idempotency keys.
- `QueueType`: The 4 logical indexing queues (`Live`, `Backfill`, `Webhook`, `DeadLetter`).
- `QueueMessage`: Version-tagged queue envelopes (`LiveBlockJob`, `BlockRangeJob`, `Custom`) supporting backward/forward wire compatibility.
- `WebhookEndpoint`, `WebhookPayload`, & `WebhookDelivery`: Models for outbound HTTP event subscriptions, signing envelopes, and delivery audit logs.

## Ports (Interfaces)
- `QueuePort`: Pluggable messaging trait implemented by RabbitMQ and AWS SQS, featuring default batch publishing (`publish_batch`).
- `ChainPort`: Blockchain interaction contract for block discovery, header queries, log extraction, and chain ID inspection (`chain_id()`).
- `BlobPort`: Object and blob storage abstraction with key-value storage and default existence checks (`exists`).
- `StorePort`: Database persistence contract for entities, checkpoints, idempotency guards, and reorg rollbacks.
- `LeaderElectionPort`: High-availability leader election contract implemented by Kubernetes Leases and local in-memory fallbacks.

## Error Model
- `ErrorClass`: Canonical error classification (`Transient`, `RateLimited`, `Shrinkable`, `Permanent`, `Integrity`, `Fatal`) driving automatic resilience policies.
- `ErrorSource`: Subsystem error origin markers (`ChainRpc`, `Queue`, `Database`, `BlobStore`, `Plugin`, `Config`, `Reconciler`, `Webhook`, `Handler`, `Internal`).
- `LogrixError`: Context-rich unified error carrier with fluent context enrichment (`with_chain_id`, `with_block_number`, `with_job_id`) and structural equality (`PartialEq`, `Eq`).
