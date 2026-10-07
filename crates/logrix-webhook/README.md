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

## Configuration Methods

### 1. Declarative Manifest Seeding (`logrix.yaml`)
Declare webhooks directly in your indexer manifest. Logrix seeds them into the database on startup:

```yaml
schema: "./schema.graphql"
network: "arbitrum-one"
contracts:
  - name: "UsdcToken"
    address: "0xaf88d065e77c8cC2239327C5EDb3A432268e5831"
webhooks:
  # At-most-once delivery (fast, default)
  - url: "https://api.mybackend.com/webhooks/transfers"
    events: ["Transfer"]

  # Retry-enabled delivery with custom attempts and secret from env
  - url: "https://critical.mybackend.com/webhooks/reorgs"
    secret: "${CRITICAL_WEBHOOK_SECRET}"
    events: ["reorg"]
    max_retries: 3
```

### 2. Static CLI Flag (Single Fallback)
```bash
logrix webhook \
  --database-url postgres://logrix:logrix@localhost:5432/logrix \
  --rabbitmq-url amqp://logrix:logrix@localhost:5672/%2f \
  --webhook-url https://api.mybackend.com/webhooks/default \
  --webhook-secret whsec_my_custom_secret
```

### 3. Dynamic Management REST API
Manage webhooks dynamically at runtime via `logrix-api` (Port 8080).
See `crates/logrix-api/README.md` for complete endpoint curl examples.
