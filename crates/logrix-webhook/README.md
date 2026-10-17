# logrix-webhook

Queue-backed HTTP delivery, HMAC signing/verification, and PostgreSQL endpoint
and delivery-summary storage. `WebhookDispatcherService` consumes
`QueueType::Webhook`; indexing and webhook REST management live in other crates.

## Prerequisites and minimal usage

Use the workspace Rust toolchain/locked dependencies, Tokio, an implementation
of `QueuePort` (CLI: RabbitMQ or SQS), and reachable HTTP(S) receivers. Migrated
PostgreSQL is optional for a programmatic/default-URL-only worker and required
for registered endpoints/audit queries. From the workspace root, with the binary
built, queue/database configured, and `WEBHOOK_SECRET` set in the environment:

```sh
./target/debug/logrix migrate
./target/debug/logrix --webhook-url https://hooks.example.com/logrix \
  --webhook-max-retries 3 webhook-dispatcher
```

That URL is illustrative; replace it with a receiver. Root flags go before the
subcommand. The dedicated command runs without `ENABLE_WEBHOOKS`; setting that
flag instead starts a dispatcher inside `processor` (including all-in-one).

In an async Rust application depending on `logrix-core` and `logrix-webhook`,
the caller supplies a configured queue, optional store, URL, secret, and shutdown
future returning `()`:

```rust
use logrix_core::ports::QueuePort;
use logrix_webhook::{WebhookConfig, WebhookDispatcherService, WebhookStore};
use std::{future::Future, sync::Arc};

async fn run_worker<F: Future<Output = ()> + Send>(
    queue: Arc<dyn QueuePort>, store: Option<WebhookStore>,
    url: String, secret: String, shutdown: F,
) {
    let service = WebhookDispatcherService::new_with_config(
        queue, store, Some(url), secret, WebhookConfig::default(),
    );
    service.run_loop_with_shutdown(shutdown).await;
}
```

`process_one()` returns `Ok(false)` when the queue is empty, `Ok(true)` after
ack/dead-letter handling, and typed errors for queue/store failures. The loop
logs errors and waits its configured backoff; shutdown can cancel an in-flight
`process_one` because it races that future. It does not guarantee delivery drain.

## Events, filtering, and queue outcome

Custom queue messages become `WebhookPayload { event: job_type, chain_id,
timestamp, data: payload }`. The outer timestamp is dispatch-time Unix seconds.
Non-custom messages become event `unknown`, chain 1, with diagnostic data.

The built-in producer is `ReorgHandler`: its job type is **`event.reverted`**
and its nested payload says `event: "chain.reorg"`, with chain ID, fork block,
fork hash, depth, and an RFC3339 timestamp. HTTP `X-Logrix-Event` and the outer
body `event` therefore say `event.reverted`, not `reorg` or `chain.reorg`.
The API registration validator knows `reorg`, `checkpoint`, and dynamic entity
names; it normally rejects `event.reverted`. A registered `["reorg"]` filter
will not receive these rollback jobs. Empty `events: []` receives them.

No normal CLI entity-write or checkpoint path publishes corresponding webhook
jobs: PostgreSQL entity notifications feed GraphQL subscriptions, not this
queue. Callers can publish custom job types, but accepted API event names alone
do not establish a built-in producer.

- Database endpoints must be active. Filters match exact, case-sensitive
  job-type strings, or all events when empty. `["*"]` is literal, despite being
  accepted by API validation. The default URL is unfiltered.
- Matching database endpoints are delivered sequentially. Duplicate registered
  URLs can each receive a request. A matched database URL suppresses the same
  exact default URL, even if its database delivery fails; other filters do not.
- If any delivery fails (including partial success), the whole queue message is
  dead-lettered. If none fail, it is acknowledged, including when no endpoint
  matches and no default URL exists. It is not automatically requeued for HTTP
  retry after this dispatch. Broker redelivery or manual replay can still cause
  duplicate requests; single-attempt settings are not at-most-once guarantees.

## HTTP contract

POST body is JSON with `event`, numeric `chain_id`, Unix-seconds `timestamp`, and
`data`. Headers: `Content-Type: application/json`, `X-Logrix-Delivery-ID` (UUID),
`X-Logrix-Event`, `X-Logrix-Chain-Id`, and `X-Logrix-Signature`.
Success is HTTP 2xx. All non-2xx statuses and request errors are eligible for
configured retries. Body bytes, signature timestamp, and delivery ID are reused
within one dispatch call, but a replay creates a new ID.

## Signature verification

`sign_payload` signs `decimal_timestamp + "." + raw_body_bytes` with HMAC-SHA256
and formats `t=<seconds>,v1=<hex>`. Receivers must verify the raw body before
reserializing JSON. In a Rust receiver depending on `logrix-webhook`:

```rust
fn accept_signature(secret: &str, header: &str, raw_body: &[u8]) -> bool {
    logrix_webhook::verify_signature(secret, header, raw_body, 300)
}
```

`verify_signature` checks the digest using `Mac::verify_slice` and rejects
timestamps more than the caller's tolerance in either past or future. It does
not maintain a replay-ID cache; repeat requests inside the window can verify.
`generate_secret()` returns `whsec_` plus 32 random alphanumeric characters.
`sanitize_webhook_url` masks embedded user-info only, not query/path secrets;
HTTP error strings may still include URLs.

## Configuration: three different retry meanings

| Setting/API | Default | Meaning |
|---|---|---|
| `WebhookEndpoint.max_retries` (database, REST, manifest seeding) | 0 | Additional retries; dispatcher uses `saturating_add(1)` total attempts |
| `WebhookConfig.max_retries` / CLI `WEBHOOK_MAX_RETRIES` | 1 | Total attempts for the default URL; 0 is clamped to 1 |
| `dispatch_with_retries(..., max_attempts)` | Caller supplies | Total attempts, clamped to at least 1 |
| `WebhookHttpClient::dispatch(...)` | 1 attempt | Always one, regardless of the client's constructor argument |

`WebhookHttpClient::new(timeout, max_retries)` builds a retry policy, but the
actual dispatch loop limit comes from `dispatch_with_retries`'s argument. Sleeps
use `RetryPolicy` jittered exponential delays (200 ms base, 4-second cap).
Retries are inside one dispatch call, not a durable delayed-retry scheduler.

### Library configuration: `WebhookConfig`

| Field | Default | Meaning |
|---|---|---|
| `timeout` | 5 seconds | Per HTTP request timeout, not whole-dispatch timeout |
| `max_retries` | 1 | Default URL total attempt count |
| `worker_idle_sleep` | 100 ms | Pause after empty queue poll |
| `worker_error_backoff` | 500 ms | Pause after a queue/store/ack/dead-letter error |

All fields are public. `WebhookConfig::new(timeout, max_retries, idle, backoff)`
and `WebhookDispatcherService::new_with_config` accept explicit values;
`new` uses defaults. `WebhookHttpClient::default()` uses a 5-second timeout.
The HTTP pool keeps idle connections for 90 seconds, up to 10 idle per host.
This library reads no CLI environment variables or YAML itself.

### Manifest seeding is CLI-owned

A complete native manifest with a receive-all endpoint (the named environment
variable must exist) is:

```yaml
schema_version: "1"
chain_id: 421614
contracts: []
webhooks:
  - url: "https://hooks.example.com/logrix"
    secret: "${LOGRIX_ENDPOINT_SECRET}"
    events: []
    maxRetries: 3
```

`url` is required; `events` defaults to `[]`; `secret` and `maxRetries` are
optional (`max_retries` is an alias). Direct CLI seeding generates missing
secrets and defaults missing retry counts to 0. Only `webhook-dispatcher` seeds
these entries when its optional database connection succeeds; it skips URLs
already active and silently ignores manifest/seed failures. The embedded
processor worker does not seed them. SDK `export-values` is different: omitted
events/retries become `["reorg"]`/3 in the exported manifest. Runtime manifest
`indexing.webhookTimeoutSecs`/`webhookMaxRetries` are not supported; the SDK can
map those project settings to Helm environment instead.

### CLI environment and flags

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `ENABLE_WEBHOOKS` | `--enable-webhooks` | `false` | Optional | Embedded processor worker, including all-in-one; dedicated runner always runs |
| `WEBHOOK_URL` | `--webhook-url` | None | Optional | Static fallback URL for external reorg and event notifications |
| `WEBHOOK_SECRET` | `--webhook-secret` | `""` | Optional | Webhook HMAC-SHA256 signing secret for default fallback URL |
| `WEBHOOK_TIMEOUT_SECS`| `--webhook-timeout-secs`| `5` seconds | Optional | Outgoing HTTP POST request timeout |
| `WEBHOOK_MAX_RETRIES` | `--webhook-max-retries` | `1` total attempt | Optional | Default URL only, not database endpoint retry count |
| `WEBHOOK_WORKER_IDLE_MS` | `--webhook-worker-idle-ms` | `100` ms | Optional | Empty-poll sleep |
| `WEBHOOK_WORKER_BACKOFF_MS` | `--webhook-worker-backoff-ms` | `500` ms | Optional | Error backoff |

The CLI also owns `--manifest-path` / `MANIFEST_PATH` (unset), database and queue
arguments. Empty default secret causes CLI runners to generate a secret at
startup. Processor configuration expands `${VAR}` in default URL/secret, but
the dedicated runner passes those CLI strings as-is. See
[CLI reference](../logrix-cli/README.md) and [configuration](../../docs/configuration.md).

## Delivery audit and error limitations

- `WebhookStore` manages endpoints and delivery summaries using an existing
  `sqlx::PgPool`; it does not run migrations. Database operation failures return
  typed `LogrixError`s (insert failures permanent, most other operations transient).
  The dispatcher propagates endpoint-list/queue errors and does not acknowledge
  a message when listing endpoints fails.
- HTTP failures return `DeliveryResult`, not a Rust `Err`. `attempts_taken`
  reflects actual HTTP loop iterations; latency covers attempts plus retry
  sleeps, not just the last request. HTTP client construction uses `expect`.
- Database-target delivery makes one best-effort summary write after its entire
  dispatch. There is no row per retry. The SQL insert/read omits `attempt` and
  `is_retry`; reads reconstruct them as **1 and false**, losing actual retry
  history. The last recorded status may precede a later transport failure.
- Default-URL deliveries are not recorded. Audit insert failures are ignored
  and do not change ack/dead-letter decisions. The API's test ping likewise
  records best effort. This is not a complete delivery audit or replay ledger.
- The management API is provided by [logrix-api](../logrix-api/README.md), whose
  CLI port defaults to 4000. It lists active endpoints, can get/update/delete by
  ID, rotate secrets, test ping, and query summary rows. Its test ping uses one
  attempt, not endpoint retries or dispatcher configuration.

## Tests and related code

From the workspace root with cached dependencies:

```sh
cargo test -p logrix-webhook --offline --locked
```

[Tests](tests/) cover HMAC freshness/tampering, unreachable HTTP delivery,
endpoint structures, and shutdown signaling; they do not establish persisted
per-attempt audit coverage. Related: [dispatcher](src/dispatcher.rs),
[target selection](src/target.rs), [audit SQL](src/deliveries.rs),
[reorg producer](../logrix-reconciler/src/rollback.rs), and
[configuration](../../docs/configuration.md).
