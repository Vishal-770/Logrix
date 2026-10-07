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

## Configuration
Only a single connection string is required:

```bash
RABBITMQ_URL="amqp://logrix:logrix@localhost:5672/%2f"
```

Prefetch is configured per worker channel (default: 100) to ensure bounded in-memory delivery and fair queue distribution.
