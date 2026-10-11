# logrix-store-postgres

PostgreSQL storage adapter for entities, checkpoints, and real-time subscription notifications.

## Overview
Implements `StorePort` using `sqlx` and PostgreSQL for transactionally safe, auditable indexing storage.

## Features
- **Dynamic Entity Tables**: Stores user-defined entities in `logrix_entities` with JSONB indexing, soft-reversion tracking (`is_reverted`), and paginated querying (`get_entities_by_type`).
- **Chain Checkpoints**: Persists continuous sync heights in `logrix_checkpoints` to allow safe pod restarts.
- **Postgres NOTIFY Event Stream**: Broadcasts `EntityMutationEvent` JSON payloads via `pg_notify` for live WebSocket GraphQL subscriptions.
- **Configurable Connection Pool**: `PostgresConfig` builder allows fine-grained customization of `max_connections`, `min_connections`, acquire timeouts, and idle timeouts.
- **Liveness & Readiness Pings**: Built-in `.ping()` method supports container health checks and Kubernetes readiness probes.
- **Embedded Migrations**: Versioned database migrations executed automatically on boot.

