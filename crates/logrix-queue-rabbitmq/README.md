# logrix-queue-rabbitmq

RabbitMQ AMQP implementation of the `QueuePort` abstraction for Logrix.

## Overview
This crate connects to a RabbitMQ message broker using a single AMQP connection URL and manages the full queue topology automatically on startup.

## Topology Architecture

```
                    +------------------------+
                    |  Exchange:             |
                    |  "logrix.events"       |
                    |  (Kind: Direct)        |
                    +-----------+------------+
                                |
   +----------------+-----------+-----------+----------------+
   | "live"         | "backfill"            | "webhook"      | "dlq"
   v                v                       v                v
+---------------+---------------+   +---------------+---------------+
| logrix.       | logrix.        |   | logrix.       | logrix.       |
| blocks.live   | blocks.backfill|   | blocks.webhook| blocks.dlq    |
+---------------+---------------+   +---------------+---------------+
```

## Physical Queues

| Queue Name | Routing Key | Purpose |
|---|---|---|
| `logrix.blocks.live` | `live` | Real-time block jobs from chain head with sub-second SLA |
| `logrix.blocks.backfill` | `backfill` | Historical block range chunks autoscaled with KEDA |
| `logrix.blocks.webhook` | `webhook` | Asynchronous external HTTP callback notifications |
| `logrix.blocks.dlq` | `dlq` | Poison messages that failed maximum retry attempts |

## Configuration & Customization
Programmatic setup supports fine-grained tuning via `RabbitMQConfig`:

```rust
use logrix_queue_rabbitmq::{RabbitMQConfig, RabbitMQQueue};

let config = RabbitMQConfig::new("amqp://logrix:logrix@localhost:5672/%2f")
    .prefetch_count(100)
    .exchange_name("logrix.events")
    .queue_prefix("staging"); // Optional multi-tenant or staging prefix

let queue = RabbitMQQueue::connect_with_config(&config).await?;
```

Prefetch is configured per worker channel to ensure bounded in-memory delivery and fair queue distribution.
Batch publishing (`publish_batch`) automatically pipelines AMQP publishes and awaits confirmations concurrently for high-throughput block ingestion.
Secret hygiene is enforced: AMQP URLs with embedded passwords are automatically sanitized in logs.

