# logrix-core

Core domain models, ports, and common errors for the Logrix indexer platform.

## Overview
This crate defines the foundational traits and data structures adhering to clean architecture and hexagonal ports-and-adapters principles.

## Core Domain Types
- `ChainId`: Type-safe Ethereum / EVM network identifier with well-known constants (Ethereum, Arbitrum, Optimism, Polygon, Base, etc.).
- `QueueType`: The 4 logical indexing queues (`Live`, `Backfill`, `Webhook`, `DeadLetter`).
- `QueueMessage`: Version-tagged queue envelopes (`LiveBlockJob`, `BlockRangeJob`, `Custom`).
- `WebhookEndpoint` & `WebhookDelivery`: Models for outbound HTTP event subscriptions and delivery logs.

## Ports (Interfaces)
- `QueuePort`: Pluggable messaging trait implemented by RabbitMQ, AWS SQS, and in-memory test mocks.
- `ChainPort`: Blockchain interaction contract for block discovery and log extraction.
- `StorePort`: Database persistence contract for entities, checkpoints, and rollbacks.
- `LeaderElectionPort`: High-availability leader election contract implemented by Kubernetes Leases and local locks.
