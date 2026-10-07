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

## Configuration
Activate SQS by setting the queue driver:

```bash
QUEUE_DRIVER=sqs
SQS_LIVE_URL=https://sqs.us-east-1.amazonaws.com/123456789012/logrix-live
SQS_BACKFILL_URL=https://sqs.us-east-1.amazonaws.com/123456789012/logrix-backfill
SQS_WEBHOOK_URL=https://sqs.us-east-1.amazonaws.com/123456789012/logrix-webhook
SQS_DLQ_URL=https://sqs.us-east-1.amazonaws.com/123456789012/logrix-dlq
```

## Local Testing
Supports custom endpoint overrides (such as LocalStack):

```bash
SQS_ENDPOINT=http://localhost:4566
```
