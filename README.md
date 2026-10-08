# Logrix

> **High-performance, self-hosted, Kubernetes-native blockchain indexer built in Rust.**  
> Write custom event indexing logic in TypeScript with [`logrix-sdk`](packages/logrix-sdk) and deploy production indexer clusters to Local Kubernetes or AWS EKS in seconds.

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Language](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![CI Status](https://img.shields.io/github/actions/workflow/status/Vishal-770/Logrix/ci.yml?branch=main&label=CI)](https://github.com/Vishal-770/Logrix/actions)
[![Docker](https://img.shields.io/badge/Docker-GHCR-blue?logo=docker)](https://github.com/Vishal-770/Logrix/pkgs/container/logrix)
[![Kubernetes](https://img.shields.io/badge/Kubernetes-Helm%20OCI-326ce5?logo=kubernetes)](https://github.com/Vishal-770/Logrix/pkgs/container/charts%2Flogrix)
[![npm](https://img.shields.io/npm/v/@logrix/sdk?color=red&logo=npm)](https://www.npmjs.com/package/@logrix/sdk)

---

## Why Logrix?

Most blockchain indexers force you into expensive proprietary cloud subscriptions with vendor lock-in, or crash when network reorgs and RPC rate-limits occur.

**Logrix is engineered for 100% data sovereignty, cost control, and high-throughput production:**

- **Custom Event Logic with TypeScript:** Write custom business logic and state transformations using [`@logrix/sdk`](packages/logrix-sdk) (compiled to sandboxed WebAssembly) or zero-code Declarative YAML.
- **Zero-Dependency Kubernetes Deployment:** Mount custom WASM binaries and GraphQL schemas directly into standard pods via Kubernetes Secrets. No custom Docker builds or S3 buckets required.
- **High-Throughput Ingestion:** Vectorized PostgreSQL writes (`UNNEST` arrays) processing **>20,000 events/second**.
- **Zero Silent Data Loss (Reorg Safe):** In-memory parent-hash rolling buffer detects forks, executes atomic database rollbacks (`is_reverted = TRUE`), indexes the canonical winning block immediately, and self-heals gaps.
- **RPC Billing Protection:** Multi-provider failover pool with per-provider circuit breakers, decorrelated jitter, and a built-in Compute Unit (CU) ledger that pauses backfills before exceeding your monthly budget.
- **Dual Storage Tiering:** Hot GraphQL entities in PostgreSQL; raw full block envelopes compressed with **`zstd`** into cold AWS S3/MinIO buckets, slashing cloud database storage bills by up to 85%.
- **Kubernetes Native (Local & AWS):** Microservice architecture with active-passive Ingester leader election (Kubernetes Leases) and event-driven worker autoscaling via **KEDA** (1 to 50+ pods).

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
           • @logrix/sdk WASM Handlers         • Exponential Retries
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

## 1. Writing Custom Event Logic (`@logrix/sdk`)

### Scaffold a New Indexer Project
```bash
npx @logrix/sdk init my-indexer
cd my-indexer
npm install
```

### Generate Strongly Typed Bindings
```bash
npm run codegen
```

### Write Handler (`handlers/mapping.ts`)
```typescript
import { TransferEvent } from "./generated/events";
import { TransferEntity } from "./generated/schema";

export function handleTransfer(event: TransferEvent): void {
  let entity = new TransferEntity(event.transactionHash + "-" + event.logIndex.toString());
  entity.blockNumber = event.blockNumber;
  entity.fromAddress = event.params.from;
  entity.toAddress = event.params.to;
  entity.amount = event.params.value;
  entity.transactionHash = event.transactionHash;
  entity.timestamp = event.blockTimestamp;
  entity.save();
}
```

### Compile to WebAssembly
```bash
npm run build
```

### Export Helm Deployment Bundle
```bash
npm run export-values
```
Generates `indexer-values.yaml` bundling your compiled WASM handler (`config.wasmBase64`), GraphQL schema, and contract manifest.

---

## 2. Kubernetes Deployments

Logrix publishes pre-built container images and Helm charts directly on GitHub Container Registry (GHCR). No container builds or repository cloning are required.

### Option A: Local Kubernetes (Minikube / Kind / K3s)

Deploy a complete, self-contained indexer cluster with built-in in-cluster PostgreSQL 16 and RabbitMQ:

```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f deploy/values-local.yaml \
  -f indexer-values.yaml
```

Port-forward and open the GraphQL playground:
```bash
kubectl port-forward svc/my-indexer-logrix-api 4000:4000
```
Visit **[http://localhost:4000/](http://localhost:4000/)** in your browser.

---

### Option B: Production AWS EKS Deployment

Deploy to AWS EKS with Aurora Serverless v2 PostgreSQL, Amazon SQS, Amazon S3, and KEDA autoscaling.

#### Step 1: Provision or Configure AWS Backing Services

- **Using Existing AWS Resources:** If you already have AWS RDS/Aurora PostgreSQL, Amazon SQS, and an S3 bucket, configure your endpoints in `deploy/values-aws.yaml` and credentials in `deploy/secrets.example.yaml`.
- **Provisioning with Terraform:** If starting from scratch, run the Terraform modules scaffolded by Logrix:
  ```bash
  cd infra/terraform
  terraform init
  terraform apply
  ```
  *(If developing directly from the source repository, the Terraform modules are located in `deploy/terraform/aws`.)*
  
  Copy the Terraform outputs (`database_url`, `sqs_queue_url`, `s3_bucket_name`, and `iam_role_arn`) into `deploy/values-aws.yaml`.

#### Step 2: Deploy Helm Chart to AWS EKS

```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f deploy/values-aws.yaml \
  -f indexer-values.yaml \
  --set config.existingSecret=logrix-aws-secrets
```

---

### Option C: Bring Your Own Infrastructure (Custom BYO)

Supply your custom cluster infrastructure values along with the generated indexer bundle:

```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f my-cluster-infra.yaml \
  -f indexer-values.yaml
```

---

## 3. Live Monitoring & Observability

### Terminal Status Monitor (TUI)
Check sync progress, queue depth, and reorg history directly in your terminal:
```bash
logrix status --watch
```

### Prometheus Metrics for Grafana
The GraphQL API server exposes Prometheus metrics at **`http://localhost:4000/metrics`**:
- `logrix_head_lag_blocks`
- `logrix_events_indexed_total`
- `logrix_queue_depth{queue="live|backfill|webhook|dlq"}`
- `logrix_rpc_cu_consumed_total`
- `logrix_reorg_detected_total`

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
