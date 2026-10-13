# logrix-cli

Command-line interface and orchestrator for running Logrix indexer roles.

## Overview
Compiles the unified binary (`logrix`) supporting standalone workers and all-in-one local development modes with unified graceful shutdown (SIGINT/SIGTERM) and queue fault tolerance.

## Worker Resilience & Fault Tolerance
- **Dual-Signal Graceful Shutdown**: All long-running workers (`ingester`, `processor`, `webhook`, `all-in-one`) listen concurrently for `SIGINT` (Ctrl+C) and `SIGTERM` (Kubernetes container termination / pod eviction), cleanly draining in-flight blocks and transactions before exiting.
- **Queue Fault Tolerance & Exponential Backoff**: The ingester automatically catches queue disconnections and retries publishing with exponential backoff (100ms up to 5s) instead of crashing, while the processor backs off on queue outages and resumes consuming once connectivity returns.
- **Ingestion Backpressure Throttling**: The ingester monitors total queue backlog (`Live` + `Backfill`) via `QueuePort::depth()`. If the backlog exceeds `--max-queue-depth` (default: 1,000 jobs), it pauses ingestion until processor pods burn down the queue, preventing RabbitMQ/SQS memory and disk overflow.
- **Database Saturation Protection**: Processors enforce bounded connection pools (`DATABASE_MAX_CONNECTIONS = 10`), preventing horizontally scaled pods (e.g. up to 50 pods via KEDA) from saturating PostgreSQL connection limits. Store write failures back off before nacking to prevent hot CPU spin loops.

## Available Subcommands
- `logrix init`: Scaffolds a new indexer project with TypeScript/AssemblyScript handlers.
- `logrix ingester`: Runs the chain head poller and block dispatcher. Supports `--chunk-size` (or `CHUNK_SIZE` env, default: 10) for strict RPC provider range constraints such as Alchemy.
- `logrix processor`: Runs the event decoding and WASM logic execution engine.
- `logrix backfill`: Triggers historical block range extraction.
- `logrix webhook`: Runs the dedicated asynchronous webhook dispatcher worker.
- `logrix api`: Runs the GraphQL server, WebSocket subscriptions, and Webhook management REST API.
- `logrix all-in-one`: Starts ingester, processor, api, and webhook runner concurrently in a single process.
- `logrix status`: Displays live indexing sync status, block heights, and health.

## Ingester Configuration
- `--chunk-size` (`CHUNK_SIZE`, default: 10): Configures the maximum block range per `eth_getLogs` batch. Defaults to 10 to comply with Alchemy Free tier limits and avoid HTTP 400 errors.
- `--max-queue-depth` (`MAX_QUEUE_DEPTH`, default: 1000): Threshold for pausing ingestion when downstream processors are backed up.
- `--chain-head-poll-ms` (`CHAIN_HEAD_POLL_MS`, default: 1500): Polling interval in milliseconds when ingester reaches chain head.

## Tuning & Resilience Configuration
- `--reconciler-chunk-size` (`RECONCILER_CHUNK_SIZE`): Block chunk size when reconciler splits missing block ranges.
- `--reconciler-lag-threshold` (`RECONCILER_LAG_THRESHOLD`, default: 50): Block lag threshold before reconciliation kicks in.
- `--rabbitmq-prefetch` (`RABBITMQ_PREFETCH`, default: 100): Consumer prefetch QoS limit.
- `--rabbitmq-timeout-secs` (`RABBITMQ_TIMEOUT_SECS`, default: 10): RabbitMQ connection timeout.
- `--sqs-wait-time-seconds` (`SQS_WAIT_TIME_SECONDS`, default: 2): SQS receive message long polling wait time (0-20s).
- `--sqs-max-messages` (`SQS_MAX_MESSAGES`, default: 1): SQS receive message maximum batch count (1-10).
- `--wasm-fuel-limit` (`WASM_FUEL_LIMIT`, default: 1,000,000): Fuel instruction budget per WASM event execution.
- `--wasm-max-memory-mb` (`WASM_MAX_MEMORY_MB`, default: 128): WASM linear memory upper limit in megabytes.
- `--rpc-timeout-secs` (`RPC_TIMEOUT_SECS`, default: 10): JSON-RPC HTTP client timeout.
- `--rpc-cb-threshold` (`RPC_CB_THRESHOLD`, default: 5): Consecutive RPC failure circuit breaker trip threshold.
- `--rpc-cooldown-secs` (`RPC_COOLDOWN_SECS`, default: 30): Circuit breaker recovery and cooldown duration.
- `--webhook-timeout-secs` (`WEBHOOK_TIMEOUT_SECS`, default: 5): Outgoing Webhook HTTP delivery request timeout.
- `--webhook-max-retries` (`WEBHOOK_MAX_RETRIES`, default: 1): Outgoing Webhook max delivery retry attempts.
- `--graphql-query-timeout-secs` (`GRAPHQL_QUERY_TIMEOUT_SECS`, default: 5): GraphQL query execution timeout.
