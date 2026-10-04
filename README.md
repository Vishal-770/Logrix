# Logrix

> **High-performance, self-hosted, Kubernetes-native blockchain indexer built in Rust.**  
> Point it at any EVM RPC, write custom event logic in TypeScript (`logrix-sdk`) or declarative YAML, and run production Kubernetes clusters in seconds.

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Language](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![CI Status](https://img.shields.io/github/actions/workflow/status/Vishal-770/Logrix/ci.yml?branch=main&label=CI)](https://github.com/Vishal-770/Logrix/actions)
[![Docker](https://img.shields.io/badge/Docker-GHCR-blue?logo=docker)](https://github.com/Vishal-770/Logrix/pkgs/container/logrix)
[![Kubernetes](https://img.shields.io/badge/Kubernetes-Helm%20OCI-326ce5?logo=kubernetes)](https://github.com/Vishal-770/Logrix/pkgs/container/charts%2Flogrix)
[![npm](https://img.shields.io/npm/v/logrix-sdk?color=red&logo=npm)](https://www.npmjs.com/package/logrix-sdk)

---

## Why Logrix?

Most blockchain indexers force you into expensive proprietary cloud subscriptions with vendor lock-in, or crash when network reorgs and RPC rate-limits occur.

**Logrix is engineered for 100% data sovereignty, cost control, and high-throughput production:**

- **High-Throughput Ingestion:** Vectorized PostgreSQL writes (`UNNEST` arrays) processing **>20,000 events/second**.
- **Zero Silent Data Loss (Reorg Safe):** In-memory parent-hash rolling buffer detects forks, executes atomic database rollbacks (`is_reverted = TRUE`), indexes the winning block immediately, and self-heals gaps.
- **RPC Billing Protection:** Multi-provider failover pool with per-provider circuit breakers, decorrelated jitter, and a built-in Compute Unit (CU) ledger that pauses backfills before exceeding your monthly budget.
- **Custom Event Logic:** Write business logic in TypeScript using [`logrix-sdk`](packages/logrix-sdk) (compiled to WASM) or pure zero-code Declarative YAML.
- **Zero-Dependency Kubernetes Deployment:** Deploy custom WebAssembly handlers and schemas directly via native Kubernetes Secrets without requiring cloud S3 buckets.
- **Production Webhooks:** Cryptographically signed (`HMAC-SHA256`) outgoing notifications with timestamp replay defense and dead-letter queue (DLQ) retries.
- **Kubernetes Native (Local & AWS):** Active-passive Ingester leader election (Kubernetes Leases) and event-driven worker autoscaling via **KEDA** (1 to 50+ pods).

---

## Architecture

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
           • logrix-sdk WASM Handlers          • Exponential Retries
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

## Custom Event Logic with `logrix-sdk`

Create your custom event mapping logic using the official TypeScript/AssemblyScript SDK:

```bash
npm install logrix-sdk
```

```typescript
// handlers/mapping.ts
import { EventLog, logrix_db_get, logrix_db_set, logrix_emit } from "logrix-sdk";

export function handleTransfer(event: EventLog): void {
  let sender = event.topics[1];
  let recipient = event.topics[2];
  let amount = event.data;

  // 1. Maintain cross-block entity state
  let currentBalance = logrix_db_get(sender);
  let newBalance = updateBalance(currentBalance, amount);
  logrix_db_set(sender, newBalance);

  // 2. Emit entity for dynamic GraphQL queries
  logrix_emit("Transfer", JSON.stringify({
    id: `${event.transaction_hash}-${event.log_index}`,
    blockNumber: event.block_number,
    fromAddress: sender,
    toAddress: recipient,
    amount: amount,
    timestamp: event.block_timestamp
  }));
}
```

Compile handler to WebAssembly:
```bash
npx asc handlers/mapping.ts -o handlers/mapping.wasm --optimize --exportRuntime
```

---

## Production Kubernetes Deployments

Logrix publishes pre-built container images and Helm charts directly on GitHub Container Registry (GHCR).

### 1. Local Kubernetes Deployment (Minikube / Kind / K3s)

Run a complete, self-contained indexer cluster with built-in PostgreSQL 16 and RabbitMQ:

```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f deploy/helm/values-local.yaml \
  --set-file config.manifestContent=manifest.yaml \
  --set-file config.schemaContent=schema.graphql \
  --set-file config.wasmBinary=handlers/mapping.wasm
```

Port-forward and open the GraphQL playground:
```bash
kubectl port-forward svc/my-indexer-logrix-api 4000:4000
```
Visit **[http://localhost:4000/](http://localhost:4000/)** in your browser.

---

### 2. Production AWS EKS Deployment

Deploy to AWS EKS with Aurora Serverless v2 PostgreSQL, Amazon SQS, and KEDA autoscaling:

#### Step 1: Provision Infrastructure with Terraform
```bash
cd deploy/terraform/aws
terraform init
terraform apply
```

#### Step 2: Deploy Helm Chart
```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f deploy/helm/values-aws.yaml \
  --set-file config.manifestContent=manifest.yaml \
  --set-file config.schemaContent=schema.graphql \
  --set-file config.wasmBinary=handlers/mapping.wasm
```

---

### 3. Bring Your Own Infrastructure (Custom BYO)

Supply your existing database and queue credentials directly in `deploy/helm/values.yaml` or via an existing Kubernetes Secret:

```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  --set config.databaseUrl="postgres://user:pass@your-db-host:5432/dbname" \
  --set config.queueDriver="rabbitmq" \
  --set config.rabbitmqUrl="amqp://user:pass@your-queue-host:5672/%2f" \
  --set config.rpcUrl="https://arb1.arbitrum.io/rpc"
```

---

## Live Monitoring & Observability

### Terminal Status Monitor (TUI)
```bash
logrix status --watch
```

### Prometheus Metrics
Prometheus endpoint available at **`http://localhost:4000/metrics`**:
- `logrix_head_lag_blocks`
- `logrix_events_indexed_total`
- `logrix_queue_depth{queue="live|backfill|webhook|dlq"}`
- `logrix_rpc_cu_consumed_total`
- `logrix_reorg_detected_total`

---

## CLI Reference

| Command | Description |
| :--- | :--- |
| `logrix init [NAME]` | Interactive project scaffolding wizard (Local K8s, AWS, or BYO). |
| `logrix deploy` | 1-Click build & deployment orchestrator for Local and AWS environments. |
| `logrix all-in-one` | Runs ingester, processor, and GraphQL API concurrently in a single process. |
| `logrix ingester` | Runs standalone head-block listener with Kubernetes Lease leader election. |
| `logrix processor` | Runs worker processing jobs from queue with reorg auto-healing. |
| `logrix api` | Runs standalone GraphQL API server with GraphiQL IDE and live subscriptions. |
| `logrix webhook-dispatcher` | Runs standalone outgoing HTTP webhook delivery worker. |
| `logrix backfill` | Executes historical range sync with `--dry-run` CU cost estimation. |
| `logrix status` | Live terminal dashboard displaying sync lag, queue depth, and health metrics. |
| `logrix migrate` | Executes PostgreSQL database migrations. |

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
