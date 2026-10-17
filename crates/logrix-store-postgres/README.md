# logrix-store-postgres

SQLx/PostgreSQL implementation of `StorePort`, plus concrete APIs for ERC-20
transfers and JSONB entities. It provides raw-event/checkpoint transactions,
explicit rollback, embedded migrations, and best-effort entity notifications.

## Prerequisites

Use the repository's Rust workspace and lockfile, a Tokio runtime, and a reachable
PostgreSQL database. The connecting role needs table access; the role running
`migrate()` also needs permission to apply the embedded DDL. The adapter does not
create a database or apply migrations merely by connecting.

## Minimal example

With `logrix-store-postgres`, `logrix-core`, and Tokio:

```rust
use logrix_core::{ChainId, StorePort};
use logrix_store_postgres::{PostgresConfig, PostgresStore};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")?;
    let config = PostgresConfig::new(database_url, "orders")
        .max_connections(10)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(600));
    let store = PostgresStore::connect_with_config(&config).await?;
    store.migrate().await?;
    store.ping().await?;
    let checkpoint = store.get_checkpoint(ChainId::ETHEREUM).await?;
    println!("Checkpoint: {checkpoint:?}");
    Ok(())
}
```

The example reads `DATABASE_URL` explicitly; the adapter constructor does not.
In a deployment, migrations can instead be run as a separate setup step.

## Public configuration APIs and defaults

`PostgresConfig::new(database_url, pipeline_id)` returns a config directly.
All fields are public, with consuming setters for the four pool options:

| Field / setter | Type | Initial value | Meaning |
|---|---|---|---|
| `database_url` | `String` | Required constructor argument | SQLx PostgreSQL connection URL. |
| `pipeline_id` | `String` | Required constructor argument | Checkpoint namespace only. |
| `max_connections` / `.max_connections(n)` | `u32` | Parsed `DATABASE_MAX_CONNECTIONS`, otherwise `10` | Pool upper bound per store/pool. |
| `min_connections` / `.min_connections(n)` | `u32` | Parsed `DATABASE_MIN_CONNECTIONS`, otherwise `2` | SQLx pool minimum target. |
| `acquire_timeout` / `.acquire_timeout(d)` | `Duration` | `10 s` | Pool connection acquisition timeout. |
| `idle_timeout` / `.idle_timeout(d)` | `Duration` | `600 s` | Idle connection timeout. |

Environment values that fail `u32` parsing silently use the fallback; explicit
setters override the constructor's environment-derived values. There is no
adapter-level validation method for URLs, pool bounds, durations, or pipeline
IDs. Choose a positive maximum and a minimum no greater than it; SQLx and
PostgreSQL enforce their own constraints. The schema limits `pipeline_id` to
64 characters. Numeric IDs/heights are cast to signed PostgreSQL `BIGINT` without
range validation, so supply values within that range.

`PostgresStore::connect(url, pipeline_id)` uses `PostgresConfig::new`.
`connect_with_config(&config)` creates a pool; `from_pool(pool, pipeline_id)`
reuses an existing one. `pool()`, `pipeline_id()`, `migrate()`, and `ping()` are
public. `ping()` executes `SELECT 1`, not a schema/migration completeness check.

### Environment, CLI, and manifest ownership

Only `DATABASE_MAX_CONNECTIONS` and `DATABASE_MIN_CONNECTIONS` are read by this
crate. The CLI separately parses:

| Variable | CLI flag | CLI default |
|---|---|---|
| `DATABASE_URL` | `--database-url` | `postgres://logrix:logrix@localhost:5432/logrix` |
| `DATABASE_MAX_CONNECTIONS` | `--database-max-connections` | `10` |
| `DATABASE_MIN_CONNECTIONS` | `--database-min-connections` | `2` |

CLI call sites apply both pool setters and use pipeline ID `default`. This crate
does not read `logrix.yaml`; SDK fields such as `indexing.databaseMaxConnections`
and `indexing.databaseMinConnections` require the surrounding tooling to map them
to runtime settings. A direct Rust config does not inherit YAML values.

## Persistence semantics and fixed limits

- `write_events_and_checkpoint(events, checkpoint)` upserts raw events and the
  selected checkpoint in one transaction. Empty event slices still advance the
  checkpoint. Event conflict keys are `(chain_id, tx_hash, log_index)`; the chain
  ID is taken from the checkpoint. Checkpoint heights use `GREATEST`, with hash
  and finality replaced on equal or greater heights. This does not detect gaps.
- `pipeline_id` namespaces only `logrix_checkpoints`. Events, transfers, and
  entities are shared across pipelines for the same chain; distinct pipeline IDs
  do not isolate their data or rollback effects. Use separate databases/schemas
  when independent data ownership is required.
- `save_transfers_batch`, `save_entities_batch`, and `delete_entity` are separate
  operations from the raw-event/checkpoint transaction. Calling them in sequence
  does not make all mutations atomic with a checkpoint. Transfers use the same
  event-style conflict key; entities use `(chain_id, entity_type, entity_id)`.
  Transfer amounts are decimal strings backed by `NUMERIC(78, 0)`;
  `TokenTransfer::amount_u256()` returns `None` for an invalid amount.
- `rollback_to_block(chain, height)` transactionally marks later events,
  transfers, and entities `is_reverted = true`, then resets only this pipeline's
  checkpoint height/finality. It leaves the checkpoint hash unchanged and does
  not create a missing checkpoint. It does not restore earlier entity versions
  overwritten by an upsert; replay is needed to rebuild them.
- `get_entity` and `get_entities_by_type` exclude reverted entities. Entity page
  limits clamp to 1-1000 and offsets to at least zero, ordered by block descending
  then entity ID ascending. Entity type/ID schema limits are 128/256 characters.
- `get_transfers` excludes reverted rows, defaults to limit 50/offset 0, clamps
  limits to 1-1000 and offsets to at least zero, and compares address filters
  case-insensitively. `query_entities_raw(sql)` executes caller-supplied SQL
  without automatic chain/reversion filters or a pagination cap; use trusted SQL.

### Entity notifications

After an entity upsert, a separate statement sends JSON on the fixed PostgreSQL
channel `logrix_entity_mutations`. Each payload contains `chain_id`,
`entity_type`, `entity_id`, `block_number`, and `data`. Only serialized payloads
strictly smaller than 7900 bytes are sent; larger ones are skipped while the
entity remains stored. Notification failures are logged and do not fail the
save. Deletes and rollbacks emit no notification. This is a live hint stream,
not durable replay or an atomic event/checkpoint subscription mechanism.

## Errors

Operations return `LogrixResult` with `ErrorSource::Database`. Connection,
transaction, query, and ping failures are generally `Transient`; migration
failures are `Permanent`, and an invalid stored checkpoint hash is `Integrity`.
These labels do not implement retries or distinguish every SQL error. Missing
checkpoints/entities return `Ok(None)`. `query_entities_raw` currently substitutes
defaults for missing/undecodable result columns; select the full record shape
when those fields matter. Notification errors are the best-effort exception above.

## Tests and coverage

From the repository root, with dependencies already cached:

```sh
cargo test -p logrix-store-postgres --offline --locked
```

`tests/store_model_tests.rs` covers transfer/entity serialization, amount parsing,
filter defaults, and config setters without a live database. It does not verify
migrations, SQL execution, transaction rollback, concurrent checkpoints, pipeline
sharing, or notification delivery against PostgreSQL.

## Related documentation

- [Configuration reference](../../docs/configuration.md)
- [Core StorePort](../logrix-core/README.md) and [testkit differences](../logrix-testkit/README.md)
- [API subscriptions](../logrix-api/README.md) and [reconciler](../logrix-reconciler/README.md)
- [Embedded migrations](migrations/)
