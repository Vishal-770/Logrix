# logrix-queue-sqs

AWS SQS queue driver implementing the `QueuePort` abstraction for Logrix.

## Overview
Provides cloud-native integration with Amazon Simple Queue Service (SQS) for serverless scalability in AWS EKS and production environments.

## Queue Structure
Unlike AMQP brokers (like RabbitMQ) which route through an internal exchange, AWS SQS operates as separate HTTP REST endpoints. Logrix maps each logical `QueueType` to its dedicated SQS queue URL.

| Logical Queue | Environment Variable | Purpose |
|---|---|---|
| `QueueType::Live` | `SQS_LIVE_URL` | High-priority live block indexing from chain head |
| `QueueType::Backfill` | `SQS_BACKFILL_URL` | Historical block range batches autoscaled via KEDA SQS triggers |
| `QueueType::Webhook` | `SQS_WEBHOOK_URL` | Outgoing HTTP webhook events delivered asynchronously |
| `QueueType::DeadLetter` | `SQS_DLQ_URL` | Poison message quarantine for inspecting failures |

## Configuration & Tuning

Activate SQS by setting `QUEUE_DRIVER=sqs` or `--queue-driver sqs`.

### 1. Declarative Manifest Configuration (`logrix.yaml`)

End users configure SQS long polling and batch sizes under `indexing:` in `logrix.yaml`:

```yaml
indexing:
  sqsWaitTimeSeconds: 5          # Long-polling duration per receive call (0-20)
  sqsMaxMessages: 10             # Maximum message batch size per receive call (1-10)
```

| `logrix.yaml` Key | Type | Default Value | Required? | Description |
|---|---|---|---|---|
| `indexing.sqsWaitTimeSeconds` | `integer` | `2` (range: 0-20) | Optional | SQS long-polling wait duration per receive call |
| `indexing.sqsMaxMessages` | `integer` | `1` (range: 1-10) | Optional | Maximum messages fetched per receive call |

### 2. Environment Variables & CLI Options

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `QUEUE_DRIVER` | `--queue-driver` | `rabbitmq` | Optional | Set to `sqs` to activate AWS SQS driver |
| `SQS_LIVE_URL` | `--sqs-live-url` | `""` | Required if SQS active | SQS Queue URL for real-time live block indexing |
| `SQS_BACKFILL_URL` | `--sqs-backfill-url` | `""` (falls back to live) | Optional | SQS Queue URL for historical backfill jobs |
| `SQS_WEBHOOK_URL` | `--sqs-webhook-url` | `""` (falls back to live) | Optional | SQS Queue URL for outgoing webhook event jobs |
| `SQS_DLQ_URL` | `--sqs-dlq-url` | `""` (falls back to live) | Optional | SQS Queue URL for poison message quarantine |
| `SQS_ENDPOINT` | `--sqs-endpoint` | None | Optional | Custom AWS endpoint override (e.g. `http://localhost:4566` for LocalStack) |
| `SQS_WAIT_TIME_SECONDS` | `--sqs-wait-time-seconds` | `2` (range: 0-20) | Optional | SQS long-polling wait duration per receive call |
| `SQS_MAX_MESSAGES` | `--sqs-max-messages` | `1` (range: 1-10) | Optional | Maximum message batch count fetched per receive call |
| `AWS_REGION` | N/A | `us-east-1` (or AWS default) | Optional | AWS region for SQS client connection |

### Environment Configuration Example

```bash
QUEUE_DRIVER=sqs
SQS_LIVE_URL=https://sqs.us-east-1.amazonaws.com/123456789012/logrix-live
SQS_BACKFILL_URL=https://sqs.us-east-1.amazonaws.com/123456789012/logrix-backfill
SQS_WEBHOOK_URL=https://sqs.us-east-1.amazonaws.com/123456789012/logrix-webhook
SQS_DLQ_URL=https://sqs.us-east-1.amazonaws.com/123456789012/logrix-dlq
SQS_WAIT_TIME_SECONDS=2
SQS_MAX_MESSAGES=10
```

### 3. Programmatic Configuration (`SqsClientConfig`)

Configure `SqsClientConfig` using standard environment variables or the fluent `SqsConfigBuilder`:

```rust
use logrix_queue_sqs::SqsClientConfig;

let config = SqsClientConfig::builder()
    .region("us-west-2")
    .endpoint_url("http://localhost:4566") // Optional LocalStack override
    .live_url("https://sqs.us-west-2.amazonaws.com/123456789012/logrix-live")
    .backfill_url("https://sqs.us-west-2.amazonaws.com/123456789012/logrix-backfill")
    .webhook_url("https://sqs.us-west-2.amazonaws.com/123456789012/logrix-webhook")
    .dlq_url("https://sqs.us-west-2.amazonaws.com/123456789012/logrix-dlq")
    .build();
```

## Batch Publishing Performance

`logrix-queue-sqs` natively implements `QueuePort::publish_batch` with automatic chunking:
- Bounded to AWS SQS's maximum batch limit of 10 messages per `SendMessageBatch` call.
- Returns detailed, typed errors if any individual messages in a batch fail on the broker.
