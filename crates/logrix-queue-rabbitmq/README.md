# logrix-queue-rabbitmq

Lapin/AMQP 0-9-1 implementation of the core `QueuePort`. It declares a durable
direct exchange and four durable queues, publishes JSON messages, and polls
single deliveries with manual acknowledgments.

## Prerequisites

Use the repository's Rust workspace and lockfile, Tokio, and a reachable RabbitMQ
vhost. The account needs permissions to declare/bind the topology and publish,
consume, and inspect queues. Existing exchange/queue declarations must have
compatible properties. Autoscaling and broker policies are deployment concerns.

## Minimal example

With `logrix-queue-rabbitmq`, `logrix-core`, and Tokio:

```rust
use logrix_core::{BlockRangeJob, ChainId, QueueMessage, QueuePort, QueueType};
use logrix_queue_rabbitmq::{RabbitMQConfig, RabbitMQQueue};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let amqp_url = std::env::var("RABBITMQ_URL")?;
    let config = RabbitMQConfig::new(amqp_url)
        .exchange_name("orders.events")
        .queue_prefix("orders")
        .connection_timeout(Duration::from_secs(10));
    let queue = RabbitMQQueue::connect_with_config(&config).await?;
    let message = QueueMessage::BackfillRange(BlockRangeJob::new(
        ChainId::ETHEREUM,
        100,
        109,
        "orders",
    ));
    queue.publish(QueueType::Backfill, &message).await?;
    Ok(())
}
```

The application explicitly reads `RABBITMQ_URL` here. Use a distinct exchange
and prefix for a separate topology in the same vhost.

## Public configuration APIs and defaults

`RabbitMQConfig::new(amqp_url)` returns a config directly; all fields are public.

| Field / setter | Type | Default | Meaning |
|---|---|---|---|
| `amqp_url` | `String` | Required argument | Broker URL, credentials, and vhost. |
| `prefetch_count` / `.prefetch_count(n)` | `u16` | `100` (`DEFAULT_PREFETCH`) | Value passed to channel `basic_qos`; zero has AMQP unlimited semantics. |
| `exchange_name` / `.exchange_name(name)` | `String` | `logrix.events` (`DEFAULT_EXCHANGE`) | Durable direct exchange name. |
| `queue_prefix` / `.queue_prefix(prefix)` | `Option<String>` | `None` | Physical queue-name prefix; trailing dots are trimmed before adding one dot. |
| `connection_timeout` / `.connection_timeout(d)` | `Duration` | `10 s` | Timeout for connection, channel creation, topology declaration, and QoS setup together. |

There is no config validation method. URL syntax, names, permissions, and broker
constraints are checked by Lapin/RabbitMQ during setup. `connection_timeout` is
not a per-publish, consume, ack, or depth timeout.

`RabbitMQQueue::connect(url, prefetch_count)` requires an explicit prefetch
argument; `connect_with_config(&config)` uses the full config. Clones share one
connection and channel. `channel()` and `topology()` expose those resources.
`redacted_url()` / `redact_amqp_url()` mask the password in conventional
`scheme://user:password@host` URLs; the connection-error message uses that helper.

### Environment, CLI, and manifest ownership

This crate reads no environment variables or YAML. `logrix-cli` owns these
mappings and passes explicit settings to the adapter:

| Variable | CLI flag | CLI default |
|---|---|---|
| `QUEUE_DRIVER` | `--queue-driver` | `rabbitmq` |
| `RABBITMQ_URL` | `--rabbitmq-url` | `amqp://logrix:logrix@localhost:5672/%2f` |
| `RABBITMQ_PREFETCH` | `--rabbitmq-prefetch` | `100` |
| `RABBITMQ_TIMEOUT_SECS` | `--rabbitmq-timeout-secs` | `10` seconds |

The CLI factory uses the default exchange and no prefix; those two settings are
currently Rust API options. SDK fields `indexing.rabbitmqPrefetch` and
`indexing.rabbitmqTimeoutSecs` require the surrounding configuration tooling;
`RabbitMQConfig` itself does not load `logrix.yaml`.

## Topology and routing

With no prefix, the declared bindings on `logrix.events` are:

| Logical queue | Physical queue | Fixed routing key |
|---|---|---|
| `Live` | `logrix.blocks.live` | `live` |
| `Backfill` | `logrix.blocks.backfill` | `backfill` |
| `Webhook` | `logrix.blocks.webhook` | `webhook` |
| `DeadLetter` | `logrix.blocks.dlq` | `dlq` |

`queue_prefix("orders")` produces names such as `orders.logrix.blocks.live`,
but leaves routing keys unchanged. Multiple prefixed topologies on one exchange
receive copies of messages with the same routing key; a prefix alone does not
isolate pipelines. Pair it with a distinct exchange or use separate vhosts.
The declaration supplies no TTL, length limit, retry policy, or dead-letter
exchange arguments.

## Delivery semantics and fixed behavior

- `consume` uses `basic_get(no_ack = false)`, not a streaming consumer, and
  returns one delivery or `Ok(None)`. Although QoS is configured, RabbitMQ's
  consumer prefetch does not bound this `basic_get` polling path. The caller
  controls outstanding handles and polling frequency.
- Receipts are decimal delivery tags scoped to the adapter's channel. Ack/nack
  one handle on that same channel (or a clone sharing it), not a new connection.
  `source_queue` is unset. `depth` passively inspects ready messages, excluding
  deliveries already in flight.
- Publishing sets delivery mode 2 (persistent) with default publish options.
  The code awaits Lapin's publish/confirmation futures but does not enable
  publisher confirms (`confirm_select`) or inspect Ack/Nack confirmation values.
  Successful return is therefore not a configured broker-confirmed durability
  guarantee or a mandatory-routing check.
- Batch publishing submits messages before awaiting the collected confirmation
  futures, which are then awaited sequentially. There is no adapter batch-size
  cap or all-or-nothing transaction; a failed batch can have partial effects.
- `nack(true)` asks the broker to requeue without changing the job attempt.
  `nack(false)` rejects without requeue; this topology does not configure automatic
  routing to its DLQ. External broker policies can change rejection behavior.
- `dead_letter(handle, reason)` explicitly publishes the unchanged payload to
  `DeadLetter`, then acknowledges the source. These are separate operations, so
  a failure between them can produce duplicate deliveries. The reason is logged,
  not added to the payload. There is no automatic retry counter, retry ceiling,
  poison-message detection, or DLQ replay loop.

## Errors

Errors use `LogrixResult` and `ErrorSource::Queue`. Setup timeout, connection,
publish, consume, ack/nack, and depth failures are generally `Transient`.
Topology declaration/binding failures, JSON serialization/deserialization, and
invalid receipt tags are `Permanent`. Invalid JSON returns an error without a
handle; the delivery is not automatically acknowledged or moved to the DLQ.
Error classes do not schedule retries or reconnect this adapter.

## Tests and coverage

From the repository root, with dependencies already cached:

```sh
cargo test -p logrix-queue-rabbitmq --offline --locked
```

`tests/rabbitmq_tests.rs` covers config setters, conventional URL redaction,
default queue names/routing, and prefixed topology names. It does not connect to
a broker or verify polling, acknowledgments, confirms, redelivery, DLQ behavior,
or isolation of multiple running topologies.

## Related documentation

- [Configuration reference](../../docs/configuration.md)
- [Core QueuePort](../logrix-core/README.md)
- [SQS differences](../logrix-queue-sqs/README.md) and [testkit differences](../logrix-testkit/README.md)
- [Architecture](../../docs/01-architecture.md)
