# logrix-webhook

Asynchronous, queue-backed webhook dispatcher and delivery audit engine for Logrix.

## Overview
This crate delivers external HTTP callbacks for blockchain reorganizations and entity mutation events without blocking the indexing pipeline. The worker consumes from `QueueType::Webhook` and dispatches cryptographically signed payloads to registered subscriber URLs.

---

## Architectural Data Flow

```
   [ Blockchain Nodes / RPC Gateway ]
                  |
                  v
          [ logrix ingester ]
      +-----------+-----------+
      v                       v
[ QueueType::Live ]   [ QueueType::Backfill ]
      +-----------+-----------+
                  |
                  v (consumed by)
         [ logrix processor ]
                  |
   +--------------+--------------+
   |                             |
   v                             v
Reorg Detected?          Entity Mutated?
(e.g. fork unwound)      (e.g. Transfer, Approval)
   |                             |
   v                             v
Emits: "event.reverted"   Emits: "entity_mutation"
   +--------------+--------------+
                  |
                  v (published to)
        [ QueueType::Webhook ] (RabbitMQ / AWS SQS)
                  |
                  v (consumed by)
      [ WebhookDispatcherService ]
       - Validates events filter
       - Computes HMAC-SHA256 signature
       - Dispatches with per-endpoint retries
       - Records audit log in PostgreSQL
                  |
                  v (HTTP POST)
   [ Downstream Webhook Receivers ]
```

---

## Core Features

1. **HMAC-SHA256 Signatures**:
   - Every outgoing request contains the header `X-Logrix-Signature: t={timestamp},v1={hex_digest}`.
   - Prevents request spoofing and protects against replay attacks by rejecting timestamps outside tolerance windows.

2. **Per-Endpoint Retry Configuration**:
   - **Default (At-Most-Once)**: When `max_retries` is 0 or omitted, the worker executes a single attempt, logs the result, and acknowledges the message immediately.
   - **Custom Retries**: Endpoints can configure custom retry counts (e.g., 3, 5). Dispatches use full-jitter exponential backoff before marking failure.

3. **Event Filtering**:
   - Endpoints can subscribe to specific events (e.g. `["reorg"]` or `["Transfer"]`).
   - Wildcard subscriptions (`["*"]` or empty list `[]`) receive all events.

4. **Target Deduplication**:
   - If a URL is configured both via database registration and through static CLI flags (`--webhook-url`), it is dispatched only once per event.

5. **Delivery Audit Trail**:
   - Every delivery records its execution metrics into PostgreSQL `logrix_webhook_deliveries`:
     - Attempt number (`attempt`)
     - Retry indicator (`is_retry`)
     - HTTP response code (`status_code`)
     - Latency in milliseconds (`latency_ms`)
     - Error message if failed (`error_message`)

---

## Outgoing HTTP Request Headers & Payload

### Headers Sent to Your Server
```http
POST /your-webhook-endpoint HTTP/1.1
Host: api.yourserver.com
Content-Type: application/json
X-Logrix-Delivery-ID: 7f1396b1-419b-449e-b816-e42776c59b32
X-Logrix-Event: Transfer
X-Logrix-Chain-Id: 421614
X-Logrix-Signature: t=1728310000,v1=9a8f27b1c4e0d4a9...
```

### JSON Body Format
```json
{
  "event": "reorg",
  "chain_id": 421614,
  "timestamp": 1728310000,
  "data": {
    "fork_block": 105230,
    "fork_hash": "0xabc123...",
    "reorg_depth": 2
  }
}
```

---

## How to Verify Webhook Signatures (Receiver Example)

Every webhook is signed with HMAC-SHA256 using the endpoint's secret.

### Node.js / TypeScript Example
```typescript
import crypto from "crypto";

function verifyLogrixSignature(
  rawBody: Buffer | string,
  signatureHeader: string,
  secret: string,
  toleranceSecs: number = 300
): boolean {
  // 1. Parse header: t=1728310000,v1=abcdef...
  const parts = signatureHeader.split(",");
  let timestamp = 0;
  let expectedSig = "";

  for (const part of parts) {
    const [k, v] = part.split("=");
    if (k === "t") timestamp = parseInt(v, 10);
    if (k === "v1") expectedSig = v;
  }

  // 2. Reject stale timestamps (prevent replay attacks)
  const now = Math.floor(Date.now() / 1000);
  if (Math.abs(now - timestamp) > toleranceSecs) {
    return false;
  }

  // 3. Compute HMAC-SHA256 over `${timestamp}.${rawBody}`
  const hmac = crypto.createHmac("sha256", secret);
  hmac.update(`${timestamp}.`);
  hmac.update(rawBody);
  const computed = hmac.digest("hex");

  return crypto.timingSafeEqual(Buffer.from(computed), Buffer.from(expectedSig));
}
```

---

## Configuration & Tuning

### 1. Declarative Manifest Configuration (`logrix.yaml`)

Declare webhooks and runtime delivery parameters directly in `logrix.yaml`:

```yaml
indexing:
  webhookTimeoutSecs: 5          # Outgoing HTTP POST request timeout in seconds
  webhookMaxRetries: 3           # Default retry attempts for webhook deliveries

webhooks:
  # Fast at-most-once delivery
  - url: "https://api.mybackend.com/webhooks/transfers"
    events: ["Transfer"]

  # Retry-enabled delivery with custom attempts and secret from env
  - url: "https://critical.mybackend.com/webhooks/reorgs"
    secret: "${CRITICAL_WEBHOOK_SECRET}"
    events: ["reorg"]
    max_retries: 5
```

| `logrix.yaml` Key | Type | Default Value | Required? | Description |
|---|---|---|---|---|
| `indexing.webhookTimeoutSecs` | `integer` | `5` seconds | Optional | Outgoing delivery HTTP request timeout |
| `indexing.webhookMaxRetries` | `integer` | `1` attempt | Optional | Global retry attempts ceiling |
| `webhooks[].url` | `string` | N/A | Required | Destination HTTP/HTTPS callback endpoint |
| `webhooks[].events` | `string[]` | `["reorg"]` | Optional | Subscribed event filters |
| `webhooks[].secret` | `string` | None | Optional | HMAC-SHA256 signature signing secret |
| `webhooks[].max_retries` | `integer` | `1` | Optional | Per-endpoint retry count override |

### 2. Environment Variables & CLI Options

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `ENABLE_WEBHOOKS` | `--enable-webhooks` | `false` | Optional | Opt-in toggle to activate webhook worker loop in all-in-one runner |
| `WEBHOOK_URL` | `--webhook-url` | None | Optional | Static fallback URL for external reorg and event notifications |
| `WEBHOOK_SECRET` | `--webhook-secret` | `""` | Optional | Webhook HMAC-SHA256 signing secret for default fallback URL |
| `WEBHOOK_TIMEOUT_SECS`| `--webhook-timeout-secs`| `5` seconds | Optional | Outgoing HTTP POST request timeout |
| `WEBHOOK_MAX_RETRIES` | `--webhook-max-retries` | `1` attempt | Optional | Maximum retry delivery attempts before logging failure |

### 3. Programmatic Configuration (`WebhookConfig`)

| Field | Default Value | Required? | Description |
|---|---|---|---|
| `timeout` | `5s` (`DEFAULT_TIMEOUT`) | Optional | Outgoing delivery HTTP client request timeout |
| `max_retries` | `1` (`DEFAULT_MAX_RETRIES`) | Optional | Max consecutive delivery retries |
| `worker_idle_sleep` | `100ms` (`DEFAULT_WORKER_IDLE_SLEEP`) | Optional | Sleep duration when webhook queue is empty |
| `worker_error_backoff`| `500ms` (`DEFAULT_WORKER_ERROR_BACKOFF`) | Optional | Backoff delay before retrying polling after network error |
| `pool_idle_timeout` | `90s` (`DEFAULT_POOL_IDLE_TIMEOUT`) | Optional | HTTP connection pool idle connection timeout |
| `pool_max_idle_per_host` | `10` (`DEFAULT_POOL_MAX_IDLE_PER_HOST`) | Optional | Maximum idle TCP connections preserved per destination host |
| `max_concurrent_dispatches` | `50` (`DEFAULT_MAX_CONCURRENT_DISPATCHES`) | Optional | Maximum simultaneous outbound HTTP dispatches governed by semaphore |
| `retry_initial_delay` | `200ms` (`DEFAULT_RETRY_INITIAL_DELAY`) | Optional | Initial delay for exponential retry backoff |
| `retry_max_delay` | `4s` (`DEFAULT_RETRY_MAX_DELAY`) | Optional | Maximum delay ceiling for retry backoff |

---

## Alternative Dispatch Methods

### CLI Standalone Runner
```bash
logrix webhook \
  --database-url postgres://logrix:logrix@localhost:5432/logrix \
  --rabbitmq-url amqp://logrix:logrix@localhost:5672/%2f \
  --webhook-url https://api.mybackend.com/webhooks/default \
  --webhook-secret whsec_my_custom_secret \
  --webhook-timeout-secs 10 \
  --webhook-max-retries 3
```

### Dynamic Management REST API
Manage webhooks dynamically at runtime via `logrix-api` (Port 8080).
See `crates/logrix-api/README.md` for complete endpoint curl examples.

---

## Programmatic Worker & Graceful Shutdown

The `WebhookDispatcherService` integrates seamlessly with container shutdown signals (SIGINT / SIGTERM):

```rust
use logrix_webhook::WebhookDispatcherService;
use std::sync::Arc;

let dispatcher = WebhookDispatcherService::new(
    queue,
    Some(store),
    Some("https://api.mybackend.com/webhooks".into()),
    "whsec_production_secret",
);

// Runs until cancellation signal is received, draining in-flight HTTP deliveries
dispatcher.run_loop_with_shutdown(tokio::signal::ctrl_c()).await;
```

## Security & Secret Hygiene

- **Constant-Time Verification**: `verify_signature` performs constant-time comparison (`Mac::verify_slice`) preventing timing side-channel attacks.
- **Credential Masking in Logs**: Webhook endpoints and HTTP warnings automatically sanitize sensitive embedded credentials via `sanitize_webhook_url`.
