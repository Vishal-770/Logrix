# logrix-store-postgres

PostgreSQL storage adapter for entities, checkpoints, and real-time subscription notifications.

## Overview
Implements `StorePort` using `sqlx` and PostgreSQL for transactionally safe, auditable indexing storage.

## Features
- **Dynamic Entity Tables**: Stores user-defined entities in `logrix_entities` with JSONB indexing, soft-reversion tracking (`is_reverted`), and paginated querying (`get_entities_by_type`).
- **Chain Checkpoints**: Persists continuous sync heights in `logrix_checkpoints` to allow safe pod restarts.
- **Postgres NOTIFY Event Stream**: Broadcasts `EntityMutationEvent` JSON payloads via `pg_notify` for live WebSocket GraphQL subscriptions.
- **Configurable Connection Pool**: `PostgresConfig` enforces bounded per-pod defaults (`max_connections = 10`, `min_connections = 2`) with automatic overrides via `DATABASE_MAX_CONNECTIONS` and `DATABASE_MIN_CONNECTIONS`, preventing database connection starvation when processor clusters scale out.
- **Liveness & Readiness Pings**: Built-in `.ping()` method supports container health checks and Kubernetes readiness probes.
- **Embedded Migrations**: Versioned database migrations executed automatically on boot.

---

## Configuration & Tuning (`PostgresConfig`)

### 1. Declarative Manifest Configuration (`logrix.yaml`)

End users configure database pool bounds under `indexing:` in `logrix.yaml`:

```yaml
indexing:
  databaseMaxConnections: 50     # Connection pool upper limit
  databaseMinConnections: 5      # Connection pool idle lower bound
```

| `logrix.yaml` Key | Type | Default Value | Required? | Description |
|---|---|---|---|---|
| `indexing.databaseMaxConnections` | `integer` | `10` connections | Optional | Connection pool concurrency ceiling |
| `indexing.databaseMinConnections` | `integer` | `2` connections | Optional | Minimum idle database connections maintained |

### 2. Environment Variables & CLI Options

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `DATABASE_URL` | `--database-url` | `postgres://logrix:logrix@localhost:5432/logrix` | Required | PostgreSQL connection string |
| `DATABASE_MAX_CONNECTIONS` | `--database-max-connections` | `10` connections | Optional | Maximum concurrent connections allocated in pool |
| `DATABASE_MIN_CONNECTIONS` | `--db-min-connections` | `2` connections | Optional | Minimum idle connections preserved in pool |

### 3. Programmatic Configuration (`PostgresConfig`)

| Field | Builder Method | Default Value | Required? | Description |
|---|---|---|---|---|
| `database_url` | `PostgresConfig::new()` | N/A | Required | Target database URL |
| `pipeline_id` | `PostgresConfig::new()` | N/A | Required | Unique indexer pipeline namespace identifier |
| `max_connections` | `.max_connections()` | `10` | Optional | Connection pool upper bound |
| `min_connections` | `.min_connections()` | `2` | Optional | Connection pool lower bound |
| `acquire_timeout` | `.acquire_timeout()` | `10s` | Optional | Maximum wait duration to checkout a pool connection |
| `idle_timeout` | `.idle_timeout()` | `600s` (10m) | Optional | Duration before idle connections are closed |

```rust
use logrix_store_postgres::{PostgresConfig, PostgresStore};
use std::time::Duration;

let config = PostgresConfig::new("postgres://user:pass@localhost:5432/logrix", "mainnet-pipeline")
    .max_connections(25)
    .min_connections(5)
    .acquire_timeout(Duration::from_secs(15))
    .idle_timeout(Duration::from_secs(300));

let store = PostgresStore::connect_with_config(&config).await?;
```
