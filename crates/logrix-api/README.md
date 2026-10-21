# logrix-api

Production GraphQL API server, real-time WebSocket subscriptions, and Webhook management service for Logrix.

## Overview
Built with Tokio and Axum, this crate provides the developer-facing GraphQL query engine and the full REST lifecycle management API for webhooks.

## GraphQL Dynamic Schema & Chain ID Resolution
The GraphQL engine automatically serves entities indexed in PostgreSQL. When queries omit the `chainId` argument, the API defaults to the configured `CHAIN_ID` environment variable (falling back to `1` for Ethereum Mainnet if unset). This allows direct querying of non-mainnet networks (e.g. Arbitrum One `42161`) without specifying `chainId` on every field:
```graphql
{
  swaps(first: 10) {
    id
    blockNumber
    sender
    recipient
  }
}
```

---

## Configuration & Tuning (`ApiConfig`)

### 1. Declarative Manifest Configuration (`logrix.yaml`)

End users configure GraphQL limits and timeouts under `indexing:` in `logrix.yaml`:

```yaml
indexing:
  graphqlMaxDepth: 7             # Max nested field depth allowed in GraphQL queries
  graphqlMaxComplexity: 200      # Max query complexity calculation budget
  graphqlDefaultLimit: 100       # Default pagination page size when first: omitted
  graphqlMaxLimit: 1000          # Ceiling cap on first: argument
  graphqlQueryTimeoutSecs: 5     # Query execution timeout in seconds
```

| `logrix.yaml` Key | Type | Default Value | Required? | Description |
|---|---|---|---|---|
| `indexing.graphqlMaxDepth` | `integer` | `7` | Optional | Maximum nested query selection depth |
| `indexing.graphqlMaxComplexity`| `integer` | `200` | Optional | Computational complexity budget per GraphQL query |
| `indexing.graphqlDefaultLimit` | `integer` | `100` | Optional | Fallback entity limit if `first:` is omitted |
| `indexing.graphqlMaxLimit` | `integer` | `1000` | Optional | Upper bound ceiling on `first:` pagination parameter |
| `indexing.graphqlQueryTimeoutSecs`| `integer` | `5` seconds | Optional | Maximum execution time before aborting query |

### 2. Environment Variables & CLI Options

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `DATABASE_URL` | `--database-url` | `postgres://logrix:logrix@localhost:5432/logrix` | Required | PostgreSQL connection string |
| `CHAIN_ID` | `--chain-id` | `421614` (CLI) / `None` (Direct, falls back to 1) | Optional | Default network chain ID injected when queries omit `chainId` |
| `GRAPHQL_MAX_DEPTH` | `--graphql-max-depth` | `7` | Optional | Maximum nested field depth allowed in GraphQL queries |
| `GRAPHQL_MAX_COMPLEXITY` | `--graphql-max-complexity` | `200` | Optional | Maximum query complexity calculation budget |
| `GRAPHQL_DEFAULT_LIMIT` | `--graphql-default-limit` | `100` | Optional | Default entity list limit if `first:` argument is omitted |
| `GRAPHQL_MAX_LIMIT` | `--graphql-max-limit` | `1000` | Optional | Maximum upper bound for `first:` pagination parameter |
| `GRAPHQL_QUERY_TIMEOUT_SECS`| `--graphql-query-timeout-secs`| `5` seconds | Optional | Maximum execution time before canceling query |
| `PORT` | `--port` | `8080` (or `4000` in container) | Optional | HTTP listen port for GraphQL server and Webhook REST API |

### 3. Programmatic Configuration (`ApiConfig`)

```rust
use logrix_api::{server::start_api_server_with_schema, ApiConfig};
use std::net::SocketAddr;

let config = ApiConfig::new()
    .with_max_depth(10)
    .with_max_complexity(300)
    .with_default_limit(50)
    .with_max_limit(500)
    .with_query_timeout_secs(10)
    .with_chain_id(42161)
    .with_allowed_origins(vec!["https://app.logrix.io".to_string()]);

start_api_server_with_schema(
    store,
    Some("schema.graphql"),
    Some(config),
    Some(broadcaster),
    SocketAddr::from(([0, 0, 0, 0], 8080)),
).await?;
```

---

## Webhook Management API (`/api/v1/webhooks`)

### 1. Register a New Webhook Endpoint
`POST /api/v1/webhooks`

Creates a new webhook subscriber. If `secret` is not provided, Logrix automatically generates a cryptographically secure key (`whsec_...`).

#### Request:
```bash
curl -X POST http://localhost:8080/api/v1/webhooks \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://api.mybackend.com/webhooks/logrix",
    "events": ["reorg", "Transfer"],
    "max_retries": 3
  }'
```

#### Successful Response (`201 Created`):
```json
{
  "id": "e89104b2-23c4-42b7-8ec9-652a926d8597",
  "url": "https://api.mybackend.com/webhooks/logrix",
  "secret": "whsec_9a8f27b1c4e0d4a938fe1029ba837190",
  "events": ["reorg", "Transfer"],
  "max_retries": 3,
  "created_at": "2026-10-07T14:30:00Z"
}
```

#### Event Validation Guardrail (`400 Bad Request`):
If an unknown event or typo is submitted, the API rejects the request immediately:
```json
{
  "error": "Invalid event 'Transfr'. Available: reorg, checkpoint, Transfer, Approval"
}
```

---

### 2. List Registered Webhooks
`GET /api/v1/webhooks`

Returns all active endpoints. Secrets are strictly masked for security.

```bash
curl http://localhost:8080/api/v1/webhooks
```

#### Response:
```json
[
  {
    "id": "e89104b2-23c4-42b7-8ec9-652a926d8597",
    "url": "https://api.mybackend.com/webhooks/logrix",
    "masked_secret": "whsec_9a8f...7190",
    "events": ["reorg", "Transfer"],
    "is_active": true,
    "max_retries": 3,
    "created_at": "2026-10-07T14:30:00Z"
  }
]
```

---

### 3. Update a Webhook Endpoint
`PATCH /api/v1/webhooks/:id`

Update the destination URL, change event filters, adjust retry settings, or pause deliveries (`is_active: false`).

```bash
curl -X PATCH http://localhost:8080/api/v1/webhooks/e89104b2-23c4-42b7-8ec9-652a926d8597 \
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
curl -X POST http://localhost:8080/api/v1/webhooks/e89104b2-23c4-42b7-8ec9-652a926d8597/rotate-secret
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

Dispatches a synthetic `test.ping` payload immediately to the destination, verifies connectivity, and returns roundtrip metrics.

```bash
curl -X POST http://localhost:8080/api/v1/webhooks/e89104b2-23c4-42b7-8ec9-652a926d8597/test
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
curl -X DELETE http://localhost:8080/api/v1/webhooks/e89104b2-23c4-42b7-8ec9-652a926d8597
```

---

### 7. Query Delivery Audit Logs
`GET /api/v1/webhooks/deliveries`

Inspect delivery attempts with optional query filters:
- `endpoint_id=<uuid>`: Filter by specific endpoint
- `success=true|false`: Filter by success or failure
- `limit=<int>`: Bounded result count (default: 50, max: 100)

```bash
# Query only failed deliveries
curl "http://localhost:8080/api/v1/webhooks/deliveries?success=false&limit=10"
```

#### Response:
```json
[
  {
    "id": "c1f7a091-8840-4b11-9a99-018274982a10",
    "endpoint_id": "e89104b2-23c4-42b7-8ec9-652a926d8597",
    "event_type": "reorg",
    "payload": {
      "event": "reorg",
      "chain_id": 421614,
      "data": { "fork_block": 105230, "reorg_depth": 2 }
    },
    "status_code": 500,
    "success": false,
    "attempt": 4,
    "is_retry": true,
    "error_message": "HTTP status 500",
    "latency_ms": 850,
    "created_at": "2026-10-07T14:35:12Z"
  }
]
```
