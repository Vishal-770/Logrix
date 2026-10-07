# logrix-cli

Command-line interface and orchestrator for running Logrix indexer roles.

## Overview
Compiles the unified binary (`logrix`) supporting standalone workers and all-in-one local development modes.

## Available Subcommands
- `logrix init`: Scaffolds a new indexer project with TypeScript/AssemblyScript handlers.
- `logrix ingester`: Runs the chain head poller and block dispatcher.
- `logrix processor`: Runs the event decoding and WASM logic execution engine.
- `logrix backfill`: Triggers historical block range extraction.
- `logrix webhook`: Runs the dedicated asynchronous webhook dispatcher worker.
- `logrix api`: Runs the GraphQL server, WebSocket subscriptions, and Webhook management REST API.
- `logrix all-in-one`: Starts ingester, processor, api, and webhook runner concurrently in a single process.
- `logrix status`: Displays live indexing sync status, block heights, and health.
