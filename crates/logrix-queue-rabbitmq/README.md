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

## Configuration & Tuning

### Environment Variables & CLI Options

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `RABBITMQ_URL` | `--rabbitmq-url` | `amqp://logrix:logrix@localhost:5672/%2f` | Required | RabbitMQ broker connection URL with credentials |
| `RABBITMQ_PREFETCH` | `--rabbitmq-prefetch` | `100` | Optional | Consumer channel prefetch QoS count |
| `RABBITMQ_TIMEOUT_SECS`| `--rabbitmq-timeout-secs`| `10` seconds | Optional | Connection attempt and channel setup timeout |
| `QUEUE_DRIVER` | `--queue-driver` | `rabbitmq` | Optional | Set to `rabbitmq` to activate RabbitMQ AMQP driver |

### Programmatic Configuration (`RabbitMQConfig`)

Fine-grained control over exchanges, queues, and channel parameters:

| Field | Builder Method | Default Value | Required? | Description |
|---|---|---|---|---|
| `amqp_url` | `RabbitMQConfig::new()` | N/A | Required | Target AMQP connection URL |
| `prefetch_count` | `.prefetch_count()` | `100` | Optional | Channel prefetch limit |
| `exchange_name` | `.exchange_name()` | `logrix.events` | Optional | Custom direct exchange name |
| `queue_prefix` | `.queue_prefix()` | None | Optional | Prefix for multi-tenant or staged queues |
| `connection_timeout` | `.connection_timeout()` | `10s` | Optional | Network timeout for broker connection |

```rust
use logrix_queue_rabbitmq::{RabbitMQConfig, RabbitMQQueue};
use std::time::Duration;

let config = RabbitMQConfig::new("amqp://logrix:logrix@localhost:5672/%2f")
    .prefetch_count(100)
    .exchange_name("logrix.events")
    .queue_prefix("staging") // Optional multi-tenant or staging prefix
    .connection_timeout(Duration::from_secs(15));

let queue = RabbitMQQueue::connect_with_config(&config).await?;
```

Prefetch is configured per worker channel to ensure bounded in-memory delivery and fair queue distribution.
Batch publishing (`publish_batch`) automatically pipelines AMQP publishes and awaits confirmations concurrently for high-throughput block ingestion.
Secret hygiene is enforced: AMQP URLs with embedded passwords are automatically sanitized in logs.

