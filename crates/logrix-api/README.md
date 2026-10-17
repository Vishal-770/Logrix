# logrix-api

Axum HTTP service with core or generated GraphQL queries, optional dynamic
entity subscriptions, probes/metrics, and webhook management routes. It uses
`PostgresStore` directly; it is a library, with runnable roles in `logrix-cli`.

## Prerequisites and minimal usage

Use the workspace Rust toolchain/locked dependencies and a migrated PostgreSQL
database. From the workspace root, with the binary built and `DATABASE_URL` set:

```sh
./target/debug/logrix migrate
./target/debug/logrix --chain-id 42161 api --port 4000
curl -s http://localhost:4000/graphql -H 'Content-Type: application/json' \
  -d '{"query":"{ health { status version } checkpoint(chainId: 42161) { lastIndexedBlock } }"}'
```

The example uses the core schema (no `SCHEMA_PATH`). For embedding, this complete
function belongs in a Tokio application depending on `logrix-api` and
`logrix-store-postgres`; the caller supplies an already-connected/migrated store:

```rust
use logrix_api::{start_api_server_with_schema, ApiConfig, SubscriptionBroadcaster};
use logrix_store_postgres::PostgresStore;
use std::{net::SocketAddr, sync::Arc};

async fn serve(store: Arc<PostgresStore>, schema_path: &str) -> std::io::Result<()> {
    let broadcaster = Arc::new(SubscriptionBroadcaster::default());
    broadcaster.clone().start_postgres_listener(store.pool().clone());
    let config = ApiConfig { chain_id: Some(42161), ..Default::default() };
    start_api_server_with_schema(
        store, Some(schema_path), Some(config), Some(broadcaster),
        SocketAddr::from(([0, 0, 0, 0], 4000)),
    ).await
}
```

For a minimal dynamic schema, save this as `schema.graphql` and run
`./target/debug/logrix --schema-path ./schema.graphql --chain-id 42161 api`:

```graphql
type Account {
  id: ID!
  owner: String
  balance: String
}
```

Query `{ accounts(first: 10) { id owner balance } }` via `POST /graphql`.
The data comes from indexed `Account` entity JSON, not on-demand RPC reads;
`balance` here is an exact decimal string supplied by your handler.

## Library configuration and CLI ownership

`ApiConfig` does not read environment variables or manifests. The caller supplies
`SocketAddr`; the library has no default port. The CLI `api` and `all-in-one`
commands bind `0.0.0.0` and default `--port` / `PORT` to **4000**.

| Public field | Library default | CLI flag / environment | Scope |
|---|---|---|---|
| `max_depth` | 7 | `--graphql-max-depth` / `GRAPHQL_MAX_DEPTH` | Dynamic schema depth |
| `max_complexity` | 200 | `--graphql-max-complexity` / `GRAPHQL_MAX_COMPLEXITY` | Dynamic schema complexity |
| `default_limit` | 100 rows | `--graphql-default-limit` / `GRAPHQL_DEFAULT_LIMIT` | Dynamic root entity lists |
| `max_limit` | 1000 rows | `--graphql-max-limit` / `GRAPHQL_MAX_LIMIT` | Dynamic root entity list cap |
| `query_timeout_secs` | 5 seconds | `--graphql-query-timeout-secs` / `GRAPHQL_QUERY_TIMEOUT_SECS` | HTTP GraphQL execution timeout |
| `chain_id` | `None` (dynamic query fallback 1) | `--chain-id` / `CHAIN_ID`: CLI default 421614 | Default dynamic query chain, not subscription filter |

The CLI also owns `DATABASE_URL`/pool flags and `--schema-path` / `SCHEMA_PATH`
(default absent). Put root flags before `api`. SDK GraphQL indexing settings
reach this configuration through `export-values` -> Helm -> environment, not by
passing a manifest to this library or the Rust API role. See
[CLI reference](../logrix-cli/README.md) and [configuration](../../docs/configuration.md).

## Schemas, HTTP routes, and limitations

- Without a schema path, or if the supplied path does not exist, startup uses
  the core schema: `transfers(chainId:, filter:)`, `checkpoint(chainId:)`, and
  `health`. Core queries require explicit chain IDs; they do not use
  `ApiConfig.chain_id`. `build_schema` does not apply configurable depth,
  complexity, or dynamic pagination limits. HTTP timeout still applies.
- Existing `.graphql`/`.gql` or `.yaml`/`.yml` entity definitions generate a
  dynamic schema (replacing core queries). Invalid existing files/build failures
  return `io::ErrorKind::InvalidData`. SDL object types define entities; their
  fields must match stored entity JSON. No database schema migration is run here.
- For an entity `Account`, query `account(id:, chainId:)` or `accounts(where:,
  orderBy:, orderDirection:, first:, skip:, after:, chainId:)`. Omitted query
  chain uses `ApiConfig.chain_id.unwrap_or(1)`. Dynamic `health` is a string,
  unlike the core health object. See [filter/query source](src/query_builder.rs).
  `after` means `entity_id > cursor`, irrespective of requested ordering; it is
  not a general stable multi-column cursor. JSON field ordering is textual.
- SDL relationship/list/index metadata is parsed, but the current object builder
  omits `@derivedFrom` fields, does not use list wrappers, and does not implement
  relationship resolvers. `@index` parsing does not create database indexes.
- `POST /graphql` executes queries; `GET /` and `/graphiql` serve GraphiQL.
  Dynamic schemas with a broadcaster additionally expose `/ws` and per-entity
  subscriptions such as `onAccount(chainId:)`. Without a subscription chain
  argument, all chains match. HTTP query timeout is not a WebSocket timeout.
- Broadcaster capacity defaults to 1024. PostgreSQL LISTEN bridges the
  `logrix_entity_mutations` channel and reconnects, but notifications are not a
  durable replay log; lagging subscribers silently lose missed broadcast items.
- `GET /healthz` returns `OK`. `/readyz` checks `SELECT 1` only when a store
  extension is present (the startup helper supplies it); bare routers otherwise
  report OK. `/metrics` exposes API/pool/database counts, but failed count queries
  become zero. It is not a chain-sync readiness probe.
- Low-level router constructors do not add store extensions, webhook management,
  or PostgreSQL listeners automatically. `start_api_server_with_schema` adds
  store and webhook routes. CORS is permissive; no authentication middleware or
  graceful-shutdown future is configured by these startup helpers.

---

## Webhook Management API (`/api/v1/webhooks`)

### 1. Register a New Webhook Endpoint
`POST /api/v1/webhooks`

Creates an active subscriber. Creation accepts `url`, optional `events` (default
`[]`), and `max_retries` (default 0 additional retries). The server always
generates a `whsec_...` secret; supplying a custom secret is not supported by the
request DTO. The full secret is returned on creation and rotation only.

#### Request:
```bash
curl -X POST http://localhost:4000/api/v1/webhooks \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://api.mybackend.com/webhooks/logrix",
    "events": [],
    "max_retries": 3
  }'
```

#### Successful Response (`201 Created`):
```json
{
  "id": "e89104b2-23c4-42b7-8ec9-652a926d8597",
  "url": "https://api.mybackend.com/webhooks/logrix",
  "secret": "whsec_9a8f27b1c4e0d4a938fe1029ba837190",
  "events": [],
  "max_retries": 3,
  "created_at": "2026-10-07T14:30:00Z"
}
```

#### Event Validation Guardrail (`400 Bad Request`):
The startup helper recognizes `reorg`, `checkpoint`, and loaded dynamic entity
names. The validator also accepts `*`, but the dispatcher treats it literally,
not as a wildcard; `[]` is the working receive-all filter. Invalid events and
URLs return 400 with a plain string body, not an `{"error": ...}` envelope.
URL checks require HTTP(S) and reject selected literal IP/local-host forms;
they do not resolve DNS or comprehensively block private destinations.

The built-in reorg producer emits `event.reverted`, not `reorg`; registration
validation normally rejects that name. Entity names/checkpoint being accepted
does not mean the indexer publishes corresponding webhook jobs. See the
[webhook event/filter boundaries](../logrix-webhook/README.md).

---

### 2. List Registered Webhooks
`GET /api/v1/webhooks`

Returns all active endpoints. Secrets are strictly masked for security.

```bash
curl http://localhost:4000/api/v1/webhooks
```

#### Response:
```json
[
  {
    "id": "e89104b2-23c4-42b7-8ec9-652a926d8597",
    "url": "https://api.mybackend.com/webhooks/logrix",
    "masked_secret": "whsec_...7190",
    "events": [],
    "is_active": true,
    "max_retries": 3,
    "created_at": "2026-10-07T14:30:00Z"
  }
]
```

---

### 3. Update a Webhook Endpoint
`PATCH /api/v1/webhooks/:id`

Update URL, event filters, additional retry count, or `is_active`. Paused
endpoints disappear from the active list; `GET /api/v1/webhooks/{id}` can still
retrieve one by ID. Missing IDs on get/update/rotation/test return 404.

```bash
curl -X PATCH http://localhost:4000/api/v1/webhooks/e89104b2-23c4-42b7-8ec9-652a926d8597 \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://new-api.mybackend.com/webhooks",
    "max_retries": 5,
    "is_active": true
  }'
```

---

### 4. Rotate Webhook Signing Secret
`POST /api/v1/webhooks/:id/rotate-secret`

Generates a new secure secret in-place without deleting the endpoint or losing delivery audit logs.

```bash
curl -X POST http://localhost:4000/api/v1/webhooks/e89104b2-23c4-42b7-8ec9-652a926d8597/rotate-secret
```

#### Response:
```json
{
  "id": "e89104b2-23c4-42b7-8ec9-652a926d8597",
  "secret": "whsec_8849bca710e28f3910cfa09827418290"
}
```

---

### 5. Instant Test Ping
`POST /api/v1/webhooks/:id/test`

Dispatches `test.ping` immediately using one attempt and the API's default
5-second webhook HTTP client, bypassing event filters and active status. HTTP
delivery failure is represented by `success: false` in the result, not a route
error status. Its audit write is best effort.

```bash
curl -X POST http://localhost:4000/api/v1/webhooks/e89104b2-23c4-42b7-8ec9-652a926d8597/test
```

#### Response:
```json
{
  "id": "2b9f310a-491a-4f51-b01c-628a01f928e1",
  "status_code": 200,
  "success": true,
  "latency_ms": 42,
  "error_message": null
}
```

---

### 6. Delete a Webhook Endpoint
`DELETE /api/v1/webhooks/:id`

Permanently deletes the webhook endpoint.

```bash
curl -X DELETE http://localhost:4000/api/v1/webhooks/e89104b2-23c4-42b7-8ec9-652a926d8597
```

---

### 7. Query Delivery Audit Logs
`GET /api/v1/webhooks/deliveries`

Inspect delivery attempts with optional query filters:
- `endpoint_id=<uuid>`: Filter by specific endpoint
- `success=true|false`: Filter by success or failure
- `limit=<int>`: Default 50, upper cap 100; negative values are not lower-clamped

```bash
# Query only failed deliveries
curl "http://localhost:4000/api/v1/webhooks/deliveries?success=false&limit=10"
```

#### Response:
```json
[
  {
    "id": "c1f7a091-8840-4b11-9a99-018274982a10",
    "endpoint_id": "e89104b2-23c4-42b7-8ec9-652a926d8597",
    "event_type": "event.reverted",
    "payload": {
      "event": "event.reverted",
      "chain_id": 421614,
      "timestamp": 1791360000,
      "data": { "event": "chain.reorg", "chain_id": 421614, "fork_block": 105230, "reorg_depth": 2 }
    },
    "status_code": 500,
    "success": false,
    "attempt": 1,
    "is_retry": false,
    "error_message": "HTTP status 500",
    "latency_ms": 850,
    "created_at": "2026-10-07T14:35:12Z"
  }
]
```

The delivery example is illustrative. Current SQL stores one summary per
database-endpoint dispatch, not each HTTP attempt. It omits `attempt` and
`is_retry`; reads always return 1/false, even after retries. Default-URL delivery
is not audited, and dispatcher/test audit write failures are ignored. Deletion
returns 204 when found, 404 otherwise. Database operation failures normally
return 500 with a string body; GraphQL resolver failures return GraphQL errors,
and execution timeout returns `Query execution timed out`.

## Tests and related references

From the workspace root with cached dependencies:

```sh
cargo test -p logrix-api --offline --locked
```

[Tests](tests/) cover schema parsing/building, query construction, middleware,
pluralization, and webhook validation. Some dynamic-schema tests build with a
lazy pool rather than executing real database queries. Related:
[source](src/), [webhook dispatch](../logrix-webhook/README.md),
[CLI](../logrix-cli/README.md), and [configuration](../../docs/configuration.md).
