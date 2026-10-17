# logrix-cli

Command-line interface and orchestrator for running Logrix indexer roles.

## Overview
Compiles the unified binary (`logrix`) supporting standalone workers and all-in-one local development modes with unified graceful shutdown (SIGINT/SIGTERM) and queue fault tolerance.

## Worker Resilience & Fault Tolerance
- **Dual-Signal Graceful Shutdown**: All long-running workers (`ingester`, `processor`, `webhook`, `all-in-one`) listen concurrently for `SIGINT` (Ctrl+C) and `SIGTERM` (Kubernetes container termination / pod eviction), cleanly draining in-flight blocks and transactions before exiting.
- **Queue Fault Tolerance & Exponential Backoff**: The ingester automatically catches queue disconnections and retries publishing with exponential backoff (100ms up to 5s) instead of crashing, while the processor backs off on queue outages and resumes consuming once connectivity returns.
- **Ingestion Backpressure Throttling**: The ingester monitors total queue backlog (`Live` + `Backfill`) via `QueuePort::depth()`. If the backlog exceeds `--max-queue-depth` (default: 1,000 jobs), it pauses ingestion until processor pods burn down the queue, preventing RabbitMQ/SQS memory and disk overflow.
- **Database Saturation Protection**: Processors enforce bounded connection pools (`DATABASE_MAX_CONNECTIONS = 10`), preventing horizontally scaled pods (e.g. up to 50 pods via KEDA) from saturating PostgreSQL connection limits. Store write failures back off before nacking to prevent hot CPU spin loops.
- **Fail-Closed Kubernetes Leadership**: The Kubernetes Lease adapter is infrastructure code in `src/leader/`, outside `logrix-core`. It requires valid TLS certificates, bounds HTTP requests with a timeout, and renews leases using `resourceVersion` optimistic concurrency. HTTP errors and malformed Lease payloads keep the process in standby.

---

## Available Subcommands (Production Runtime Node)

| Subcommand | Description |
|---|---|
| `logrix ingester` | Runs the chain head poller, event log fetcher, and queue block dispatcher |
| `logrix processor` | Runs the queue consumer, event decoder, and WASM logic execution engine |
| `logrix backfill` | Triggers historical block range extraction (chunked and validated) |
| `logrix webhook` | Runs the dedicated asynchronous outbound HTTP webhook dispatcher worker |
| `logrix api` | Runs the GraphQL server, WebSocket subscriptions, and Webhook REST API |
| `logrix migrate` | Runs PostgreSQL database schema migrations |
| `logrix all-in-one` | Runs ingester, processor, api, and webhook workers concurrently in a single process |
| `logrix status` | Displays live indexing sync status, block heights, and health |

> Note: Developer project scaffolding, type codegen, and WASM compilation are managed through `@logrix/sdk` (`npx @logrix/sdk init`).

---

## Unified Configuration Reference

Configuration is resolved across multiple layers according to the following precedence order (highest to lowest priority):

1. **CLI Arguments**: Explicit flags passed directly to `logrix` binary subcommands (e.g. `--wasm-fuel-limit 5000000`).
2. **Environment Variables**: Overrides injected by Kubernetes ConfigMaps, Secrets, `.env`, or Docker Compose (e.g. `WASM_FUEL_LIMIT=5000000`).
3. **Declarative Manifest (`logrix.yaml`)**: Project-level configuration file loaded via `--manifest-path` (supports both SDK project format and runtime manifest format), or translated into Helm values via `logrix export-values`.
4. **Built-in System Defaults**: Sane, production-hardened defaults embedded within the core crates.

### 1. Declarative Manifest Configuration (`logrix.yaml`)

```yaml
specVersion: "0.1.0"
name: "my-indexer"

network:
  name: "arbitrum-one"
  chainId: 42161
  rpcUrl: "https://arb-mainnet.g.alchemy.com/v2/${API_KEY}"
  fallbackRpcUrls:
    - "https://arb1.arbitrum.io/rpc"

indexing:
  # Ingestion & Polling
  chunkSize: 1000
  chainHeadPollMs: 1500
  maxQueueDepth: 1000

  # Reorg & Gap Reconciliation
  ringBufferDepth: 128
  reconcilerIntervalSecs: 30
  reconcilerChunkSize: 50
  reconcilerLagThreshold: 50

  # Gateway & RPC Resilience
  rpcTimeoutSecs: 15
  rpcCbThreshold: 5
  rpcCooldownSecs: 30

  # Store & Queues
  databaseMaxConnections: 25
  rabbitmqPrefetch: 100

  # WASM & API
  wasmFuelLimit: 5000000
  wasmMaxMemoryMb: 256
  graphqlMaxDepth: 7
```

---

### 2. Environment Variables & CLI Options Reference

### 1. Database & Persistence (PostgreSQL)

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `DATABASE_URL` | `--database-url` | `postgres://logrix:logrix@localhost:5432/logrix` | Required | PostgreSQL connection string |
| `DATABASE_MAX_CONNECTIONS` | `--database-max-connections` | `10` | Optional | Maximum database connection pool size per pod |
| `DATABASE_MIN_CONNECTIONS` | `--database-min-connections` | `2` | Optional | Minimum idle database connections in pool |

### 2. Message Queue (RabbitMQ / AWS SQS)

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `QUEUE_DRIVER` | `--queue-driver` | `rabbitmq` | Optional | Queue driver to use: `rabbitmq` or `sqs` |
| `RABBITMQ_URL` | `--rabbitmq-url` | `amqp://logrix:logrix@localhost:5672/%2f` | Required if RabbitMQ | RabbitMQ AMQP connection URL |
| `RABBITMQ_PREFETCH` | `--rabbitmq-prefetch` | `100` | Optional | Consumer channel prefetch QoS limit |
| `RABBITMQ_TIMEOUT_SECS` | `--rabbitmq-timeout-secs` | `10` | Optional | RabbitMQ connection timeout in seconds |
| `SQS_LIVE_URL` | `--sqs-live-url` | `""` | Required if SQS | AWS SQS Live Block Queue URL |
| `SQS_BACKFILL_URL` | `--sqs-backfill-url` | `""` | Optional | AWS SQS Backfill Queue URL (falls back to live) |
| `SQS_WEBHOOK_URL` | `--sqs-webhook-url` | `""` | Optional | AWS SQS Webhook Queue URL (falls back to live) |
| `SQS_DLQ_URL` | `--sqs-dlq-url` | `""` | Optional | AWS SQS Dead Letter Queue URL (falls back to live) |
| `SQS_ENDPOINT` | `--sqs-endpoint` | None | Optional | Custom AWS endpoint override (e.g. LocalStack) |
| `SQS_WAIT_TIME_SECONDS` | `--sqs-wait-time-seconds` | `2` (range: 0-20) | Optional | SQS long-polling wait duration in seconds |
| `SQS_MAX_MESSAGES` | `--sqs-max-messages` | `1` (range: 1-10) | Optional | SQS receive batch size |

### 3. Chain & JSON-RPC Gateway

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `RPC_URL` | `--rpc-url` | `https://sepolia-rollup.arbitrum.io/rpc` | Required | Primary EVM JSON-RPC URL (Tier 1) |
| `RPC_FALLBACK_URLS` | `--rpc-fallback-urls` | None | Optional | Fallback JSON-RPC URLs (comma-separated, Tier 2/3) |
| `CHAIN_ID` | `--chain-id` | `421614` | Optional | Target EVM chain ID (e.g. 1, 42161, 8453) |
| `CONTRACT_ADDRESS` | `--contract-address` | `0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d` | Optional | Target contract address (defaults to Arbitrum Sepolia USDC) |
| `START_BLOCK` | `--start-block` | None (indexed checkpoint or tip) | Optional | Initial block to start indexing if no checkpoint exists |
| `CHUNK_SIZE` | `--chunk-size` | `10` blocks | Optional | Maximum block query range per `eth_getLogs` batch |
| `CHAIN_HEAD_POLL_MS` | `--chain-head-poll-ms` | `1500` ms | Optional | Polling interval in ms when ingester reaches chain head |
| `MAX_QUEUE_DEPTH` | `--max-queue-depth` | `1000` jobs | Optional | Ingester backpressure threshold before throttling |
| `CU_BUDGET` | `--cu-budget` | None (Unlimited) | Optional | Compute Unit budget limit before halting queries |
| `BULK_STREAM_URL` | `--bulk-stream-url` | None | Optional | Bulk stream archive endpoint override (e.g. SQD / HyperSync) |
| `RPC_TIMEOUT_SECS` | `--rpc-timeout-secs` | `10` seconds | Optional | EVM JSON-RPC HTTP client timeout |
| `RPC_CB_THRESHOLD` | `--rpc-cb-threshold` | `5` failures | Optional | Consecutive RPC failures before tripping circuit breaker |
| `RPC_COOLDOWN_SECS` | `--rpc-cooldown-secs` | `30` seconds | Optional | Circuit breaker cooldown before attempting recovery probe |

### 4. Reorg Detection & Gap Reconciler

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `RING_BUFFER_DEPTH` | `--ring-buffer-depth` | `128` blocks | Optional | In-memory parent-hash ring buffer size for reorg detection |
| `RECONCILER_INTERVAL_SECS` | `--reconciler-interval-secs` | `30` seconds | Optional | Background gap inspection polling interval |
| `RECONCILER_CHUNK_SIZE` | `--reconciler-chunk-size` | `CHUNK_SIZE` (10) | Optional | Block chunk size when reconciler splits missing block ranges |
| `RECONCILER_LAG_THRESHOLD` | `--reconciler-lag-threshold` | `50` blocks | Optional | Minimum block lag behind chain head before reconciling |

### 5. WebAssembly & Handler Runtime

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `MANIFEST_PATH` | `--manifest-path` | None | Optional | Path to YAML manifest file (`logrix.yaml`) |
| `SCHEMA_PATH` | `--schema-path` | None | Optional | Path to GraphQL schema file (`schema.graphql`) |
| `WASM_FUEL_LIMIT` | `--wasm-fuel-limit` | `1,000,000` instructions | Optional | Default instruction fuel budget per event handler run |
| `WASM_MAX_MEMORY_MB` | `--wasm-max-memory-mb` | `128` MB | Optional | Linear memory upper ceiling per WASM instance |

### 6. GraphQL API Server

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `GRAPHQL_MAX_DEPTH` | `--graphql-max-depth` | `7` | Optional | Maximum nested query depth |
| `GRAPHQL_MAX_COMPLEXITY` | `--graphql-max-complexity` | `200` | Optional | Maximum query complexity budget |
| `GRAPHQL_DEFAULT_LIMIT` | `--graphql-default-limit` | `100` | Optional | Default query pagination size |
| `GRAPHQL_MAX_LIMIT` | `--graphql-max-limit` | `1000` | Optional | Maximum query pagination size ceiling |
| `GRAPHQL_QUERY_TIMEOUT_SECS`| `--graphql-query-timeout-secs`| `5` seconds | Optional | Maximum execution time per GraphQL query |

### 7. Webhook Dispatcher

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `ENABLE_WEBHOOKS` | `--enable-webhooks` | `false` | Optional | Toggle to run webhook dispatcher loop in all-in-one |
| `WEBHOOK_URL` | `--webhook-url` | None | Optional | Fallback HTTP destination for event callbacks |
| `WEBHOOK_SECRET` | `--webhook-secret` | `""` | Optional | Signing secret for HMAC-SHA256 signature verification |
| `WEBHOOK_TIMEOUT_SECS` | `--webhook-timeout-secs` | `5` seconds | Optional | Outgoing HTTP request timeout |
| `WEBHOOK_MAX_RETRIES` | `--webhook-max-retries` | `1` attempt | Optional | Maximum delivery retry attempts |

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
