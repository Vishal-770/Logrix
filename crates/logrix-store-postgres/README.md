# logrix-store-postgres

PostgreSQL storage adapter for entities, checkpoints, and real-time subscription notifications.

## Overview
Implements `StorePort` using `sqlx` and PostgreSQL for transactionally safe, auditable indexing storage.

## Features
- **Dynamic Entity Tables**: Stores user-defined entities in `logrix_entities` with JSONB indexing and soft-reversion tracking (`is_reverted`).
- **Chain Checkpoints**: Persists continuous sync heights in `logrix_checkpoints` to allow safe pod restarts.
- **Postgres NOTIFY Event Stream**: Broadcasts `EntityMutationEvent` JSON payloads via `pg_notify` for live WebSocket GraphQL subscriptions.
- **Embedded Migrations**: Versioned database migrations executed automatically on boot.
