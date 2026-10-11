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
- `logrix ingester`: Runs the chain head poller and block dispatcher.
- `logrix processor`: Runs the event decoding and WASM logic execution engine.
- `logrix backfill`: Triggers historical block range extraction.
- `logrix webhook`: Runs the dedicated asynchronous webhook dispatcher worker.
- `logrix api`: Runs the GraphQL server, WebSocket subscriptions, and Webhook management REST API.
- `logrix all-in-one`: Starts ingester, processor, api, and webhook runner concurrently in a single process.
- `logrix status`: Displays live indexing sync status, block heights, and health.
