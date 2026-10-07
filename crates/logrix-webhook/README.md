# logrix-webhook

Asynchronous, queue-backed webhook dispatcher and delivery audit engine for Logrix.

## Overview
This crate handles external HTTP callbacks for blockchain reorganizations and entity mutation events without blocking the indexing pipeline.

## Features
- **HMAC-SHA256 Signatures**: Signs outgoing payloads using `X-Logrix-Signature: t={timestamp},v1={hex_digest}` to prevent spoofing and replay attacks.
- **Delivery Audit Trail**: Every attempt records timestamp, HTTP status code, roundtrip latency, and errors into PostgreSQL `logrix_webhook_deliveries`.
- **Target Deduplication**: Guarantees that endpoints configured in both the database and via static CLI flags are only dispatched once per event.
- **Dead-Letter Routing**: Unreachable endpoints exceeding retry limits are automatically moved to the DLQ (`QueueType::DeadLetter`).

## Outgoing Payload Format

```json
{
  "id": "c7a8b9...",
  "event": "reorg",
  "chain_id": 42161,
  "timestamp": 1728310000,
  "data": {
    "depth": 3,
    "from_block": 18239010,
    "to_block": 18239013
  }
}
```

## Running the Webhook Worker

```bash
logrix webhook \
  --database-url postgres://logrix:logrix@localhost:5432/logrix \
  --rabbitmq-url amqp://logrix:logrix@localhost:5672/%2f
```
