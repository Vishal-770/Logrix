# logrix-queue-sqs

AWS SDK implementation of the core `QueuePort`, mapping each logical queue to
an SQS queue URL. It supports long polling, a local receive buffer, explicit
dead-lettering, and chunked batch send/delete operations.

## Prerequisites

Use the repository's Rust workspace and lockfile, Tokio, and pre-created SQS
queues (or a compatible local endpoint such as LocalStack). Supply an AWS region
and credentials through the SDK configuration/provider chain. Permissions must
cover send, receive, delete, change visibility, and read queue attributes as used.
The adapter does not create queues or configure redrive, retention, or scaling.
It supplies no FIFO message group/deduplication IDs; use standard queues for the
implemented path.

## Minimal example

With `logrix-queue-sqs`, `logrix-core`, and Tokio; the application explicitly
reads the queue URL and the SDK resolves credentials:

```rust
use logrix_core::{BlockRangeJob, ChainId, QueueMessage, QueuePort, QueueType};
use logrix_queue_sqs::{SqsClientConfig, SqsQueue};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = SqsClientConfig::builder()
        .region("us-east-1")
        .backfill_queue_url(std::env::var("SQS_BACKFILL_URL")?)
        .wait_time_seconds(2)
        .max_number_of_messages(1)
        .build()
        .await?;
    let queue = SqsQueue::new(config);
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

Only the queue used by this example is configured. Configure `dlq_url` as well
before calling `dead_letter`; normal processor deployments use both live and
backfill queues, plus any webhook/DLQ queues they access.

## Public configuration APIs and defaults

`SqsClientConfig` has public fields `client`, `queue_urls`,
`wait_time_seconds: i32`, and `max_number_of_messages: i32`.

| API / builder method | Default | Meaning |
|---|---|---|
| `SqsClientConfig::new(client, queue_urls)` | Wait `2 s`, receive count `1` | Use an existing SDK client and `HashMap<QueueType, String>`. |
| `new_with_params(client, queue_urls, wait, count)` | Explicit wait/count | Stores parameters without validation. |
| `builder().live_queue_url(url)` | Unset | URL for `QueueType::Live`. |
| `.backfill_queue_url(url)` | Unset | URL for `QueueType::Backfill`. |
| `.webhook_queue_url(url)` | Unset | URL for `QueueType::Webhook`. |
| `.dlq_url(url)` | Unset | URL for `QueueType::DeadLetter`. |
| `.region(region)` | AWS SDK provider chain | Region override; no crate-level `us-east-1` fallback. |
| `.endpoint_override(url)` | No explicit override | SDK endpoint override, for example `http://localhost:4566`. |
| `.wait_time_seconds(n)` | `2` seconds | Long-poll wait; SQS accepts 0-20. The adapter passes it through without validation/clamping. |
| `.max_number_of_messages(n)` | `1` message | Receive request count; clamped to 1-10 at consume time, not when stored. |

`SqsConfigBuilder::build().await` loads SDK config and creates the client, but
does not contact/validate the queues. Missing URLs are allowed;
`resolve_url(queue_type)` returns a `Permanent` error if an entry is absent.
An empty URL is still a present entry and is not rejected locally.

`SqsClientConfig::from_env(live, backfill, webhook, dlq, endpoint).await` takes
all four URLs as arguments and uses wait 2/count 1. `SqsQueue::from_env` wraps it;
`SqsQueue::from_env_with_params` also takes explicit wait/count values.
`SqsQueue::new(config)`, `config()`, and `client()` are public. Clones share the
local receive buffer. There is no adapter timeout/retry policy builder; those
settings can be supplied through a custom AWS SDK client.

### Environment, CLI, and manifest ownership

Despite their names, `from_env` methods do not parse `SQS_*` variables. They use
the AWS SDK's region/credential configuration (for example `AWS_REGION`,
`AWS_PROFILE`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and
`AWS_SESSION_TOKEN`), while the caller supplies Logrix queue settings.
`logrix-cli` owns these mappings:

| Variable | CLI flag | CLI default |
|---|---|---|
| `QUEUE_DRIVER` | `--queue-driver` | `rabbitmq`; select `sqs` for this adapter |
| `SQS_LIVE_URL` | `--sqs-live-url` | Empty string |
| `SQS_BACKFILL_URL` | `--sqs-backfill-url` | Empty string |
| `SQS_WEBHOOK_URL` | `--sqs-webhook-url` | Empty string |
| `SQS_DLQ_URL` | `--sqs-dlq-url` | Empty string |
| `SQS_ENDPOINT` | `--sqs-endpoint` | None |
| `SQS_WAIT_TIME_SECONDS` | `--sqs-wait-time-seconds` | `2` seconds |
| `SQS_MAX_MESSAGES` | `--sqs-max-messages` | `1` |

This crate does not load YAML. SDK fields `indexing.sqsWaitTimeSeconds` and
`indexing.sqsMaxMessages` belong to the surrounding configuration tooling and
must be mapped to runtime inputs.

## Delivery semantics and fixed limits

- `consume` returns one handle, first checking a per-logical-queue local buffer.
  An empty receive returns `Ok(None)`. When SQS returns multiple messages, the
  first is returned and every extra handle has visibility changed to the fixed
  `SQS_MAX_VISIBILITY_TIMEOUT = 43_200` seconds (12 hours) before buffering.
- The first message uses the queue's configured visibility timeout. Buffered
  messages keep their extended visibility when later returned; it is not reset
  to the queue default. There is no heartbeat, configurable extension duration,
  or drop/shutdown hook to release buffered receipts. A stopped process can leave
  them hidden until visibility expires. Service rejection of an extension is
  returned as an error. Default receive count 1 avoids this buffering path.
- `ack` deletes using the returned receipt and source queue. Preserve
  `MessageHandle::source_queue`; an unscoped handle falls back to `Live`.
  Receipts are tied to a particular receive/queue and can expire; do not replace
  them with job IDs or reuse stale receipts after redelivery.
- `nack(true)` sets visibility to zero. `nack(false)` sets it to a fixed
  300 seconds; it does not delete or dead-letter the message. Neither changes
  `QueueMessage::attempt`.
- `dead_letter` sends the unchanged payload to the configured DLQ URL and then
  deletes the source. Those operations are not atomic; duplicates are possible
  if deletion fails. The reason is logged, not embedded. Automatic retry counts
  and SQS redrive policy must be managed by the caller/deployment.
- `publish_batch` and the concrete `ack_batch` split requests into chunks of at
  most `SQS_MAX_BATCH_SIZE = 10`. **Pass only one source queue to `ack_batch`.**
  Each chunk uses its first handle's source queue (or `Live`) for every receipt;
  mixed-source batches are not grouped or rejected locally.
- Batch failures may follow partial success. Failed response entries return a
  `Transient` error with a count, not per-entry IDs/details; there is no selective
  retry or rollback of completed entries/chunks. SQS payload limits are enforced
  by the service, not prevalidated here.
- `depth` queries `ApproximateNumberOfMessages` (visible messages only). A missing
  or unparsable count becomes zero; it is not an exact total of buffered/in-flight
  messages. Consumers must tolerate duplicate/out-of-order deliveries.

## Errors

Methods return `LogrixResult` with `ErrorSource::Queue`. Missing URL mappings,
JSON serialization/parsing, and batch-entry construction errors are `Permanent`;
missing received bodies/receipts are `Integrity`. SDK operations and partial
batch failures are generally `Transient`, including service validation failures.
Malformed messages are not automatically acknowledged or dead-lettered. An error
does not undo earlier receives, visibility changes, or successful batch entries.

## Tests and coverage

From the repository root, with dependencies cached; the environment settings
make region selection explicit and disable EC2 metadata discovery:

```sh
AWS_REGION=us-east-1 AWS_EC2_METADATA_DISABLED=true cargo test -p logrix-queue-sqs --offline --locked
```

`tests/sqs_tests.rs` covers JSON round trips, handle source metadata, batch-entry
builders, URL resolution, config builder setup, and the 43,200-second constant.
It does not send SQS requests or verify real visibility changes, receipt expiry,
buffer recovery, partial batch outcomes, or redrive behavior.

## Related documentation

- [Configuration reference](../../docs/configuration.md)
- [Core QueuePort](../logrix-core/README.md)
- [RabbitMQ differences](../logrix-queue-rabbitmq/README.md) and [testkit differences](../logrix-testkit/README.md)
- [Deployment manifests](../../deploy/)
