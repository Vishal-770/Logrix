<p align="center">
  <img src="logo.png" alt="Logrix Banner" width="100%" />
</p>

<p align="center">
  <strong>Ultra-fast, Kubernetes-native EVM indexer in Rust.</strong><br />
  Write custom event indexing logic in TypeScript with <a href="packages/logrix-sdk"><code>logrix-sdk</code></a> and deploy production indexer clusters in seconds.
</p>

---

## Highlights

- **Custom Event Logic with TypeScript**: Write type-safe handlers with [`@logrix/sdk`](packages/logrix-sdk) (compiled to sandboxed WebAssembly) or zero-code declarative YAML.
- **High-Throughput Ingestion**: Vectorized PostgreSQL writes (`UNNEST` arrays) processing **>20,000 events/sec**.
- **Reorg-Safe Atomic Rollbacks**: Rolling parent-hash ring buffer detects reorgs, applies atomic state rollbacks (`is_reverted = TRUE`), and re-indexes canonical blocks with self-healing gap reconciliation.
- **RPC Cost & Rate-Limit Protection**: Multi-provider failover pool with per-node circuit breakers, decorrelated full jitter, and built-in Compute Unit (CU) budget tracking.
- **Dual Storage Tiering**: Hot GraphQL entities in PostgreSQL; raw block envelopes compressed with **zstd** into cold S3/MinIO buckets, reducing database storage costs by up to 85%.
- **Kubernetes-Native Architecture**: Active-passive Ingester leader election via Kubernetes Leases and event-driven worker autoscaling via **KEDA** (1 to 50+ pods).

---

## Quickstart (`@logrix/sdk`)

### 1. Initialize Project

```bash
npx @logrix/sdk init my-indexer
cd my-indexer
npm install
```

### 2. Generate Typed Bindings

```bash
npm run codegen
```

### 3. Write Event Handler (`handlers/mapping.ts`)

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

### 4. Build & Export Values

```bash
npm run build
npm run export-values
```

Outputs `indexer-values.yaml` containing the compiled WASM handler, GraphQL schema, and contract manifest ready for deployment.

---

## Deployment

Logrix provides two deployment paths via official Helm charts:

### Option A: Self-Hosted Kubernetes (Default)

Deploys to any standard Kubernetes cluster (Minikube, Kind, K3s, EKS, GKE, or bare-metal). Includes built-in in-cluster **PostgreSQL 16** and **RabbitMQ 3.13** out of the box:

```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f indexer-values.yaml
```

**Port-forward the GraphQL API server:**
```bash
kubectl port-forward svc/my-indexer-logrix-api 4000:4000
```
Open **[http://localhost:4000/](http://localhost:4000/)** for GraphiQL IDE, or query `http://localhost:4000/graphql`.

*Optional Cold S3 Archiving:*
```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f indexer-values.yaml \
  --set builtIn.minio=true
```

---

### Option B: AWS Production Deployment (EKS + Terraform)

Production architecture utilizing AWS EKS, AWS Aurora Serverless v2 PostgreSQL, Amazon SQS, and Amazon S3.

1. **Provision Infrastructure (Terraform):**
   ```bash
   cd deploy/terraform/aws
   terraform init
   terraform apply
   ```

2. **Deploy to EKS:**
   ```bash
   aws eks update-kubeconfig --name <cluster-name> --region <aws-region>

   helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
     -f deploy/helm/values-aws.yaml \
     -f indexer-values.yaml \
     --set config.existingSecret=logrix-secrets
   ```

---

## Monitoring & Observability

### Terminal Status Dashboard
Monitor sync head, lag, queue depth, and reorg history live in the terminal:
```bash
logrix status --watch
```

### Prometheus Metrics
Exposed by default on the GraphQL API service (`:4000/metrics`):
- `logrix_head_lag_blocks`: Distance between latest chain head and indexed tip.
- `logrix_events_indexed_total`: Total indexed events count.
- `logrix_queue_depth{queue="live|backfill|webhook|dlq"}`: Pending message backlog.
- `logrix_rpc_cu_consumed_total`: Cumulative RPC Compute Units used.
- `logrix_reorg_detected_total`: Total reorgs caught and rolled back.

---

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
