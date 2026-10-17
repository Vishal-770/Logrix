# logrix-cli

The `logrix` Rust binary wires database, queue, RPC, handler, API, and webhook
adapters into runnable roles. This is the canonical reference for arguments in
[`args.rs`](src/args.rs), flattened [`ExtendedArgs`](src/args_tuning.rs), and
[`Commands`](src/commands.rs).

## Prerequisites and minimal usage

Build from the workspace root with the workspace Rust toolchain and cached
locked dependencies:

```sh
cargo build -p logrix-cli --bin logrix --offline --locked
```

Start PostgreSQL and RabbitMQ (or provision SQS plus AWS credentials/region),
then supply their actual URLs in the environment. The examples assume the
built binary and services are available; the built-in localhost credentials
below are defaults, not provisioned services:

```sh
./target/debug/logrix migrate
./target/debug/logrix --rpc-url https://arb1.arbitrum.io/rpc --chain-id 42161 \
  --start-block 250000000 all-in-one --port 4000
```

For separate processes, run `ingester`, `processor`, and `api` with the same
chain/database/queue configuration after migration. Add `webhook-dispatcher`
when consuming webhook jobs. A handler manifest and entity schema are optional;
without them the processor indexes raw logs/ERC-20 transfers and the API uses
its core schema. WASM files referenced by a manifest must already be compiled.

## Commands and command-local arguments

Syntax: `logrix [ROOT OPTIONS] <COMMAND> [COMMAND OPTIONS]`. Root options are
not marked global in Clap: place them before the command. All root options are
optional. Explicit flags override their environment binding, then Clap defaults;
the later consumer-specific manifest rules below can override those values.
There is no automatic `.env` loading.

| Command | Command-local arguments (environment; default) | Behavior |
|---|---|---|
| `status` | `--watch` (none; false) | PostgreSQL status, optionally refreshed; does not check queue/RPC health |
| `migrate` | None | Runs database migrations |
| `ingester` | None | Polls height and publishes live/range jobs; does not fetch event logs |
| `processor` | None | Consumes backfill before live jobs; fetches logs, executes handlers, writes data; starts reconciler |
| `webhook-dispatcher` (alias `webhook`) | None | Dedicated webhook queue consumer; runs regardless of `ENABLE_WEBHOOKS` |
| `backfill` | Required `--from-block <u64>`, `--to-block <u64>`; `--dry-run` (none; false) | Inclusive range jobs; dry run prints a heuristic estimate, but still connects to queue first |
| `api` | `--port <u16>` (`PORT`; 4000) | HTTP on `0.0.0.0`; GraphQL, probes, webhook REST; dynamic subscriptions only with entity schema |
| `all-in-one` | `--port <u16>` (`PORT`; 4000) | Migrates, then spawns ingester, processor, and API; local leader elector |
| `help [COMMAND]` | Optional command | Generated help |

`-h`/`--help` prints root or command help; root `-V`/`--version` prints the
package version. Boolean flags are switches (`--enable-webhooks`, not
`--enable-webhooks true`); their environment values are parsed by Clap.
Scaffolding, build/codegen, and `export-values` belong to the TypeScript SDK,
not this binary. There is no Rust `start` or `export-values` command.

## Manifest ingestion and per-role differences

`--manifest-path` accepts native runtime or the supported SDK project subset
documented in [handlers](../logrix-handlers/README.md). It is not a universal
runtime configuration file:

- Ingester: `indexing.chunkSize`, `maxQueueDepth`, and `chainHeadPollMs` replace
  resolved flag/environment values, including explicit flags. Chunk size clamps
  to at least 1 and is further capped by the gateway's adaptive recommendation.
  Startup uses database checkpoint + 1, else `START_BLOCK`, else head minus 50
  (saturating). Manifest chain ID, RPC URLs, and data-source start blocks do not
  set those runtime settings.
- Processor: manifest contract addresses select RPC logs; engine rules/WASM
  process them. Disabled contracts still appear in RPC address selection.
  WASM limit precedence is contract > manifest indexing > CLI global > instance
  defaults. Reconciler/ring settings come only from CLI, even though the manifest
  parser accepts corresponding indexing keys. Relative WASM paths are working-
  directory-relative; a missing file can fall back to the container-mounted
  `/etc/logrix/handlers/mapping.wasm`.
- Dedicated webhook runner: if PostgreSQL connects, it seeds manifest webhooks,
  skipping URLs already active. Missing retries become zero and missing secrets
  are generated. Manifest parse/seed errors are ignored. PostgreSQL connection
  failure allows default-URL-only operation; its pool bypasses CLI pool limits.
  This runner uses raw `WEBHOOK_URL`/`WEBHOOK_SECRET` strings; processor config
  expands `${VAR}` placeholders in them. Both generate a secret if empty.
- `ENABLE_WEBHOOKS` starts an embedded dispatcher in `processor` (also the
  processor inside `all-in-one`); that path does not seed manifest webhooks.
  `all-in-one` always uses a local leader elector even if leader election is
  enabled. `api`, `backfill`, `migrate`, and `status` do not ingest the manifest.

RPC primary/fallback strings expand `${VAR}` in the gateway factory. Manifest
string values expand placeholders during parsing. Missing/invalid placeholders
are configuration errors where parsing is propagated.

SDK `export-values` maps network and operational project settings to Helm
values; the chart then injects CLI environment variables. That export step is
distinct from direct Rust manifest loading and has its own defaults. See
[configuration](../../docs/configuration.md),
[SDK converter](../../packages/logrix-sdk/src/cli/manifest_converter.ts), and
[chart ConfigMap](../../deploy/helm/logrix/templates/configmap.yaml).

## Complete root option/environment reference

Values below are CLI defaults, not necessarily library defaults. `None` means
omitted, `""` an empty string. Resource counts/units are described per row.

### 1. Database & Persistence (PostgreSQL)

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `DATABASE_URL` | `--database-url` | `postgres://logrix:logrix@localhost:5432/logrix` | Default supplied | PostgreSQL connection string |
| `DATABASE_MAX_CONNECTIONS` | `--database-max-connections` | `10` | Optional | Maximum connections per configured database pool |
| `DATABASE_MIN_CONNECTIONS` | `--database-min-connections` | `2` | Optional | Minimum idle database connections in pool |

### 2. Message Queue (RabbitMQ / AWS SQS)

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `QUEUE_DRIVER` | `--queue-driver` | `rabbitmq` | Optional | Queue driver to use: `rabbitmq` or `sqs` |
| `RABBITMQ_URL` | `--rabbitmq-url` | `amqp://logrix:logrix@localhost:5672/%2f` | Default supplied | RabbitMQ AMQP connection URL |
| `RABBITMQ_PREFETCH` | `--rabbitmq-prefetch` | `100` | Optional | Consumer channel prefetch QoS limit |
| `RABBITMQ_TIMEOUT_SECS` | `--rabbitmq-timeout-secs` | `10` | Optional | RabbitMQ connection timeout in seconds |
| `SQS_LIVE_URL` | `--sqs-live-url` | `""` | Required if SQS | AWS SQS Live Block Queue URL |
| `SQS_BACKFILL_URL` | `--sqs-backfill-url` | `""` | Needed for backfill use | AWS SQS Backfill Queue URL; no live-URL fallback |
| `SQS_WEBHOOK_URL` | `--sqs-webhook-url` | `""` | Needed for webhook use | AWS SQS Webhook Queue URL; no live-URL fallback |
| `SQS_DLQ_URL` | `--sqs-dlq-url` | `""` | Needed for dead-letter use | AWS SQS Dead Letter Queue URL; no live-URL fallback |
| `SQS_ENDPOINT` | `--sqs-endpoint` | None | Optional | Custom AWS endpoint override (e.g. LocalStack) |
| `SQS_WAIT_TIME_SECONDS` | `--sqs-wait-time-seconds` | `2` (range: 0-20) | Optional | SQS long-polling wait duration in seconds |
| `SQS_MAX_MESSAGES` | `--sqs-max-messages` | `1` (range: 1-10) | Optional | SQS receive batch size |

### 3. Chain & JSON-RPC Gateway

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `RPC_URL` | `--rpc-url` | `https://sepolia-rollup.arbitrum.io/rpc` | Default supplied | Primary EVM JSON-RPC URL (priority 1) |
| `RPC_FALLBACK_URLS` | `--rpc-fallback-urls` | `[]` | Optional | Comma-separated fallback URLs, assigned increasing priorities |
| `CHAIN_ID` | `--chain-id` | `421614` | Optional | Target EVM chain ID (e.g. 1, 42161, 8453) |
| `CONTRACT_ADDRESS` | `--contract-address` | `0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d` | Optional | Target contract address (defaults to Arbitrum Sepolia USDC) |
| `START_BLOCK` | `--start-block` | None | Optional | Ingester starts at checkpoint + 1, else this value, else head - 50 |
| `CHUNK_SIZE` | `--chunk-size` | `10` blocks | Optional | Ingester cap and backfill job size; zero clamps to 1; not reconciler chunk size |
| `CHAIN_HEAD_POLL_MS` | `--chain-head-poll-ms` | `1500` ms | Optional | Polling interval in ms when ingester reaches chain head |
| `MAX_QUEUE_DEPTH` | `--max-queue-depth` | `1000` jobs | Optional | Ingester backpressure threshold before throttling |
| `CU_BUDGET` | `--cu-budget` | None (Unlimited) | Optional | Process-local estimated CU counter; blocks future gateway calls once exhausted |
| `BULK_STREAM_URL` | `--bulk-stream-url` | None | Optional | Stored archive endpoint override; streaming is unimplemented and uses RPC fallback |
| `RPC_TIMEOUT_SECS` | `--rpc-timeout-secs` | `10` seconds | Optional | EVM JSON-RPC HTTP client timeout |
| `RPC_CB_THRESHOLD` | `--rpc-cb-threshold` | `5` failures | Optional | Consecutive RPC failures before tripping circuit breaker |
| `RPC_COOLDOWN_SECS` | `--rpc-cooldown-secs` | `30` seconds | Optional | Circuit breaker cooldown before attempting recovery probe |

### 4. Reorg Detection & Gap Reconciler

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `RING_BUFFER_DEPTH` | `--ring-buffer-depth` | `128` blocks | Optional | In-memory parent-hash ring buffer size for reorg detection |
| `RECONCILER_INTERVAL_SECS` | `--reconciler-interval-secs` | `30` seconds | Optional | Background gap inspection polling interval |
| `RECONCILER_CHUNK_SIZE` | `--reconciler-chunk-size` | None | Optional | Unset sends one whole lag range; explicit value clamps to at least 1 block |
| `RECONCILER_LAG_THRESHOLD` | `--reconciler-lag-threshold` | `50` blocks | Optional | Schedules only when head - checkpoint is strictly greater than this |

### 5. WebAssembly & Handler Runtime

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `MANIFEST_PATH` | `--manifest-path` | None | Optional | Path to YAML manifest file (`logrix.yaml`) |
| `SCHEMA_PATH` | `--schema-path` | None | Optional | Entity schema: `.graphql`, `.gql`, `.yaml`, or `.yml` |
| `WASM_FUEL_LIMIT` | `--wasm-fuel-limit` | `1,000,000` fuel units | Optional | Global handler metering budget; manifest/contract can override |
| `WASM_MAX_MEMORY_MB` | `--wasm-max-memory-mb` | `128` MiB | Optional | Initial/post-call memory check threshold; not a hard allocation ceiling |

### 6. GraphQL API Server

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `GRAPHQL_MAX_DEPTH` | `--graphql-max-depth` | `7` | Optional | Dynamic schema maximum nested query depth |
| `GRAPHQL_MAX_COMPLEXITY` | `--graphql-max-complexity` | `200` | Optional | Dynamic schema query complexity budget |
| `GRAPHQL_DEFAULT_LIMIT` | `--graphql-default-limit` | `100` rows | Optional | Dynamic root entity list default page size |
| `GRAPHQL_MAX_LIMIT` | `--graphql-max-limit` | `1000` rows | Optional | Dynamic root entity list page cap |
| `GRAPHQL_QUERY_TIMEOUT_SECS`| `--graphql-query-timeout-secs`| `5` seconds | Optional | HTTP GraphQL execution timeout, not a subscription timeout |

### 7. Webhook Dispatcher

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `ENABLE_WEBHOOKS` | `--enable-webhooks` | `false` | Optional | Starts embedded processor dispatcher, including in all-in-one |
| `WEBHOOK_URL` | `--webhook-url` | None | Optional | Fallback HTTP destination for event callbacks |
| `WEBHOOK_SECRET` | `--webhook-secret` | `""` | Optional | Signing secret for HMAC-SHA256 signature verification |
| `WEBHOOK_TIMEOUT_SECS` | `--webhook-timeout-secs` | `5` seconds | Optional | Outgoing HTTP request timeout |
| `WEBHOOK_MAX_RETRIES` | `--webhook-max-retries` | `1` total attempt | Optional | Default URL total attempt limit, clamped to at least 1; DB endpoints use their own additional retry count |
| `WEBHOOK_WORKER_IDLE_MS` | `--webhook-worker-idle-ms` | `100` ms | Optional | Sleep after an empty webhook queue poll |
| `WEBHOOK_WORKER_BACKOFF_MS` | `--webhook-worker-backoff-ms` | `500` ms | Optional | Sleep after a webhook queue/store/ack error |

### 8. High Availability & Cloud Object Storage

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `ENABLE_LEADER_ELECTION` | `--enable-leader-election` | `false` | Optional | Enable Kubernetes Lease leader election for Ingester |
| `LEASE_NAME` | `--lease-name` | `logrix-ingester-lease` | Optional | Kubernetes Lease resource name |
| `K8S_NAMESPACE` | `--k8s-namespace` | `default` | Optional | Kubernetes namespace for Lease resource |
| `POD_NAME` | `--pod-name` | `local-instance` | Optional | Pod identity string for Lease holder |
| `S3_BUCKET` | `--s3-bucket` | None | Optional | AWS S3 bucket for compressed raw block archiving |
| `S3_ENDPOINT` | `--s3-endpoint` | None | Optional | Custom S3 endpoint override (MinIO, LocalStack, R2) |
| `S3_PREFIX` | `--s3-prefix` | None | Optional | Object key prefix for stored blobs |

`QUEUE_DRIVER` selects SQS only for case-insensitive `sqs`; every other string
selects RabbitMQ. Supply all SQS URLs for operations the role uses; empty URLs
are passed through, not replaced. SQS poll/batch ranges in the table are service
constraints, not Clap range validation. AWS region/credential discovery is owned
by the AWS SDK, not additional flattened CLI options.

`RUST_LOG` is read by tracing-subscriber (fallback `info,logrix=debug`), with no
CLI flag. There is no `RPC_MAX_RETRIES` CLI/environment binding. Adapter-specific
credentials and runtime settings outside these argument structs are documented
by their adapter crates.

## Fixed behavior and error boundaries

- Ingester retries queue publication with exponential sleeps from 100 ms to
  5 seconds. Backpressure is active at `live_depth + backfill_depth >= limit`;
  depth-query errors become zero, and limit zero throttles even an empty queue.
  This is an ingestion throttle, not a queue/disk/memory bound.
- Processor consumes backfill first, then live. Consume errors back off from
  500 ms to 5 seconds; common fetch/write failures nack for requeue with 500 ms
  sleeps. Some ack/nack/publish errors are ignored. Range splitting recognizes
  selected error-message strings, not all provider failures.
- Live envelopes run continuity detection; range-log backfills do not. The
  reconciler checks lag above a checkpoint, not holes below it. Database rollback,
  handler memory, queue notification, entities, transfers, and checkpoint writes
  are not one cross-system transaction. See [reconciler](../logrix-reconciler/README.md)
  and [handlers](../logrix-handlers/README.md) for exact limits.
- S3 archiving applies to live block envelopes; serialization/compression/upload
  failures are ignored. Range backfill does not archive blobs through that path.
- Standalone ingester/processor/webhook loops observe SIGINT/SIGTERM. Shutdown
  handling differs by await point; webhook shutdown can cancel an in-flight
  `process_one`. `all-in-one` returns on a signal or first task termination
  without joining/draining all spawned tasks. API serving has no configured
  graceful-shutdown future. There is no universal flush/drain guarantee.
- Startup adapter/manifest errors generally propagate out of `main`; malformed
  contract addresses panic before dispatching any command. The dedicated webhook
  runner's optional store/seeding exceptions are described above. In all-in-one,
  child task errors are logged and the parent returns `Ok(())` after selection.
- Pool limits apply per pool, not to the aggregate across processes; all-in-one
  opens multiple pools. The status and dedicated webhook paths do not use the
  CLI pool-limit builder. Zero reconciler interval is not rejected by Clap and
  causes a Tokio timer panic. These flags do not establish deployment capacity
  or availability guarantees.
- `status --watch` refreshes every 3 seconds. Its queries are not filtered by
  the selected CLI chain, query errors fall back to zero/missing values, and
  displayed health/zero-gap labels are fixed text, not validated health results.

The API's default listen port is 4000 in both `api` and `all-in-one`. Query-depth,
complexity, pagination, chain-default, and subscription behavior differ between
core and dynamic schemas; see [API](../logrix-api/README.md). For actual webhook
event names, filtering and retry/audit semantics, see [webhooks](../logrix-webhook/README.md).

## Tests and related references

From the workspace root, using cached dependencies:

```sh
cargo test -p logrix-cli --offline --locked
```

[Tests](tests/) cover ingestion tuning/placeholder expansion and queue resilience;
the all-in-one module also tests pool settings. These do not certify complete
multi-service shutdown or rollback. Related:
[configuration](../../docs/configuration.md), [EVM client](../logrix-chain-evm/README.md),
[RPC gateway](../logrix-rpc-gateway/README.md), and [root setup](../../README.md).
