# Logrix

> **High-performance, self-hosted, Kubernetes-native blockchain indexer built in Rust.**  
> Point it at any EVM RPC, define your smart contracts, and sync a real-time queryable database in seconds.

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Language](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![CI Status](https://img.shields.io/github/actions/workflow/status/Vishal-770/Logrix/ci.yml?branch=main&label=CI)](https://github.com/Vishal-770/Logrix/actions)
[![Docker](https://img.shields.io/badge/Docker-GHCR-blue?logo=docker)](https://github.com/Vishal-770/Logrix/pkgs/container/logrix)
[![Kubernetes](https://img.shields.io/badge/Kubernetes-Helm%20OCI-326ce5?logo=kubernetes)](https://github.com/Vishal-770/Logrix/pkgs/container/charts%2Flogrix)

---

## Why Logrix?

Most blockchain indexers force you into expensive proprietary cloud subscriptions with vendor lock-in, or crash when network reorgs and RPC rate-limits occur.

**Logrix is engineered for 100% data sovereignty, cost control, and high-throughput production:**

- **High-Throughput Ingestion:** Vectorized PostgreSQL writes (`UNNEST` arrays) processing **>20,000 events/second**.
- **Zero Silent Data Loss (Reorg Safe):** In-memory parent-hash rolling buffer detects forks, executes atomic database rollbacks (`is_reverted = TRUE`), indexes the winning block immediately, and self-heals gaps.
- **RPC Billing Protection:** Multi-provider failover pool with per-provider circuit breakers, decorrelated jitter, and a built-in Compute Unit (CU) ledger that pauses backfills before exceeding your monthly budget.
- **Dual Storage Tiering:** Hot GraphQL entities in PostgreSQL; raw full block envelopes compressed with **`zstd`** into cold AWS S3/MinIO buckets, slashing cloud database storage bills by up to 85%.
- **Production Webhooks:** Cryptographically signed (`HMAC-SHA256`) outgoing notifications with timestamp replay defense and dead-letter queue (DLQ) retries.
- **Kubernetes & Cloud Native:** Microservice architecture with active-passive Ingester leader election (Kubernetes Leases) and event-driven auto-scaling via **KEDA** (1 to 50+ worker pods).

---

## Architecture Overview

```text
                      EVM Blockchain (Ethereum, Arbitrum, Base, Polygon)
                                       │
                                       ▼
                       ┌───────────────────────────────┐
                       │      Logrix RPC Gateway       │
                       │  • Multi-Provider Failover    │
                       │  • CU Budget Ledger ($)       │
                       │  • Singleflight Dedup         │
                       └───────────────┬───────────────┘
                                       │
                                       ▼
                         Logrix Ingester (Leader Pod)
                                       │
                                       ▼
                     Message Queue (RabbitMQ / AWS SQS)
                                       │
                      ┌────────────────┼────────────────┐
                      ▼                                 ▼
           Logrix Processor (KEDA)             Webhook Dispatcher
           • Reorg Auto-Rollback               • HMAC-SHA256 Signed
           • TypeScript / WASM Handlers        • Exponential Retries
           • Self-Healing Gap Reconciler       • Dead-Letter Queue
                      │
           ┌──────────┴──────────┐
           ▼                     ▼
     PostgreSQL               AWS S3
     (Hot Query Tier)     (Cold zstd Blobs)
           │
           ▼
     GraphQL API Server
     • Sub-10ms Queries
     • Live WebSockets
     • GraphiQL Studio (:4000)
```

---

## Quickstart (Get Running in 60 Seconds)

### 1. Initialize Your Project

Scaffold a complete, tailored indexer workspace using the interactive CLI wizard:

```bash
logrix init my-indexer
```

The wizard prompts you for:
1. **Infrastructure Profile**:
   - **Local Docker** (PostgreSQL 16, RabbitMQ with Web UI, MinIO S3)
   - **AWS Cloud-Native** (Automated Terraform for Aurora, SQS, S3, EKS)
   - **Custom BYO** (Bring your own existing database and queue)
2. **Target Network**: Arbitrum One, Ethereum, Base, Polygon, Arbitrum Sepolia, or Custom RPC.
3. **Smart Contract Address & Events**: Enter contract address and start block.
4. **Custom Logic**: No-Code Declarative YAML or Full-Code TypeScript/WASM.

---

### 2. Run Locally (Docker)

```bash
cd my-indexer

# 1. Start local infrastructure with persistent disk volumes
docker compose up -d

# 2. Run the indexer
logrix all-in-one
```

That's it. Logrix automatically runs migrations, connects to the chain, starts indexing, and launches the API.

---

### 3. Query Data via GraphQL & Studio

Open **[http://localhost:4000/](http://localhost:4000/)** in your browser to launch the embedded **GraphiQL Studio**:

```graphql
query {
  transfers(first: 10, orderBy: BLOCK_NUMBER_DESC) {
    nodes {
      id
      blockNumber
      fromAddress
      toAddress
      amount
      transactionHash
    }
  }
}
```

#### Real-Time WebSocket Subscriptions
Listen to new on-chain events live in your frontend:

```graphql
subscription {
  newTransfer {
    blockNumber
    fromAddress
    toAddress
    amount
  }
}
```

---

## Live Monitoring & Observability

### Terminal Status Monitor (TUI)
Check real-time sync progress, queue depths, RPC budgets, and reorg history directly in your terminal:

```bash
logrix status --watch
```

```text
┌───────────────────────── LOGRIX STATUS MONITOR ────────────────────────┐
│ Status: HEALTHY                     Version: 0.1.0                     │
├────────────────────────────────────────────────────────────────────────┤
│ [Blockchain Progress]                                                  │
│    Latest Indexed Block: 22150418                                      │
│    Latest Block Hash:    0x7b2f48...3e91a0                             │
│                                                                        │
│ [Stored Data & Events]                                                 │
│    Total Event Logs:     1842910                                       │
│    Dynamic Entities:     1420                                          │
│                                                                        │
│ [Reorg & Self-Healing]                                                 │
│    Reverted Events:      0                                             │
│    Health State:         [CONTINUOUS / ZERO GAPS]                      │
│                                                                        │
│ [Webhook Engine]                                                       │
│    Active Endpoints:     3                                             │
└────────────────────────────────────────────────────────────────────────┘
```

### Prometheus Metrics for Grafana
The GraphQL API server exposes Prometheus metrics at **`http://localhost:4000/metrics`**:
- `logrix_head_lag_blocks`
- `logrix_events_indexed_total`
- `logrix_queue_depth{queue="live|backfill|webhook|dlq"}`
- `logrix_rpc_cu_consumed_total`
- `logrix_reorg_detected_total`

---

## Production Deployment (Kubernetes & AWS)

Logrix packages production-ready container images and Helm charts directly on GitHub Container Registry (GHCR).

### Option A: Deploy via Helm (Kubernetes)

Install directly from the OCI registry in one command:

```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  --set config.chainId=42161 \
  --set config.contractAddress="0xaf88d065e77c8cC2239327C5EDb3A432268e5831" \
  --set config.rpcUrl="https://arb1.arbitrum.io/rpc" \
  --set config.databaseUrl="postgres://user:pass@aurora-pg.rds.amazonaws.com/indexer" \
  --set config.queueDriver="sqs" \
  --set config.enableWebhooks=true
```

### Option B: Deploy to AWS via Terraform

Provision production-grade AWS infrastructure (Aurora Serverless v2 PostgreSQL, Amazon SQS with DLQ, Amazon S3 with lifecycle transitions, and EKS with IRSA keyless IAM authentication):

```bash
cd deploy/terraform/aws
terraform init
terraform apply
```

---

## CLI Reference

| Command | Description |
| :--- | :--- |
| `logrix init [NAME]` | Interactive project scaffolding wizard (Local Docker, AWS, or BYO). |
| `logrix all-in-one` | Runs ingester, processor, and GraphQL API concurrently in a single process. |
| `logrix ingester` | Runs standalone head-block listener with Kubernetes Lease leader election. |
| `logrix processor` | Runs worker processing jobs from queue with reorg auto-healing. |
| `logrix api` | Runs standalone GraphQL API server with GraphiQL IDE and live subscriptions. |
| `logrix webhook-dispatcher`| Runs standalone outgoing HTTP webhook delivery worker. |
| `logrix backfill` | Executes historical range sync with `--dry-run` CU cost estimation. |
| `logrix status` | Live terminal dashboard displaying sync lag, queue depth, and health metrics. |
| `logrix migrate` | Executes PostgreSQL database migrations. |

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
