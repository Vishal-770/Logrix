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
- **Strict Core Safety Contracts**: Core ports have no HTTP dependencies, queue handles preserve source lineage, and checkpoints require canonical non-zero block hashes.

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

### 3. Write Event Handler (`src/mapping.ts`)

```typescript
import { TransferEvent } from "./generated/events";
import { Transfer } from "./generated/schema";

export function handleTransfer(event: TransferEvent): i32 {
  let entity = new Transfer(event.transactionHash + "-" + event.logIndex.toString());
  entity.blockNumber = event.blockNumber;
  entity.fromAddress = event.params.from.toHexString();
  entity.toAddress = event.params.to.toHexString();
  entity.amount = event.params.value;
  entity.transactionHash = event.transactionHash;
  entity.timestamp = event.blockTimestamp;
  entity.save();
  return 0;
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

Production cloud architecture utilizing AWS EKS, AWS Aurora Serverless v2 PostgreSQL, Amazon SQS, and Amazon S3. Live deployments strictly consume the remote published OCI Helm chart (`oci://ghcr.io/vishal-770/charts/logrix`).

1. **Provision Infrastructure (Terraform):**
   ```bash
   cd deploy/terraform/aws
   terraform init
   terraform apply
   ```

2. **Connect EKS & Install KEDA Autoscaler:**
   ```bash
   aws eks update-kubeconfig --name <cluster-name> --region <aws-region>

   # Install KEDA for SQS queue-driven processor autoscaling
   helm repo add kedacore https://kedacore.github.io/charts
   helm repo update
   helm install keda kedacore/keda --namespace keda --create-namespace
   ```

3. **Configure Kubernetes Secret Vault:**
   ```bash
   RAW_DB_URL=$(aws secretsmanager get-secret-value \
     --region <aws-region> \
     --secret-id "<rds-secret-arn>" \
     --query SecretString --output text | jq -r .url)

   # Strip ?sslmode=require for internal VPC connectivity
   CLEAN_DB_URL="${RAW_DB_URL%\?*}"

   kubectl create namespace logrix
   kubectl create secret generic logrix-secrets -n logrix \
     --from-literal=DATABASE_URL="$CLEAN_DB_URL" \
     --from-literal=RPC_URL="<primary-rpc-url>" \
     --from-literal=RPC_FALLBACK_URLS="<fallback-rpc-urls>"
   ```

4. **Run Initial Database Migrations:**
   ```bash
   kubectl run logrix-migrator --rm -i --restart=Never \
     --image=ghcr.io/vishal-770/logrix:0.8.3 \
     -n logrix \
     --env="DATABASE_URL=$CLEAN_DB_URL" \
     -- command logrix migrate
   ```

5. **Deploy from Remote OCI Helm Chart:**
   ```bash
   helm upgrade --install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
     --version 0.8.3 \
     -n logrix \
     --disable-openapi-validation \
     --set config.existingSecret="logrix-secrets" \
     --set config.chainId=1 \
     --set config.queueDriver=sqs \
     --set config.sqsLiveUrl="<sqs_live_queue_url>" \
     --set config.sqsBackfillUrl="<sqs_backfill_queue_url>" \
     --set config.s3Bucket="<s3_bucket_name>" \
     --set processor.keda.enabled=true \
     --set processor.keda.sqs.queueUrl="<sqs_live_queue_url>" \
     --set serviceAccount.annotations."eks\.amazonaws\.com/role-arn"="<processor_irsa_arn>" \
     --set ingester.serviceAccount.annotations."eks\.amazonaws\.com/role-arn"="<ingester_irsa_arn>" \
     --set processor.serviceAccount.annotations."eks\.amazonaws\.com/role-arn"="<processor_irsa_arn>" \
     --set api.serviceAccount.annotations."eks\.amazonaws\.com/role-arn"="<api_irsa_arn>"
   ```
   > **Note on Versioning**: The remote OCI Helm chart version (`--version 0.8.3`) automatically sets the container image tag (`ghcr.io/vishal-770/logrix:0.8.3`) via `appVersion`. Environment overlays (`values-aws.yaml`) omit `image.tag` so that bumping the chart version cleanly updates workloads without version pinning drift.

6. **Verify Indexing & GraphQL Endpoints:**
   ```bash
   # Verify all pods are 1/1 Running
   kubectl get pods -n logrix

   # Port-forward and query the GraphQL API
   kubectl port-forward svc/my-indexer-logrix-api 4000:4000 -n logrix

   # In another terminal:
   curl -s -X POST http://localhost:4000/graphql \
     -H "Content-Type: application/json" \
     -d '{"query": "{ health { status version } checkpoint(chainId: 1) { chainId lastIndexedBlock lastBlockHash } }"}'
   ```

---

## Configuration & Tuning Reference

All Logrix components are fully configurable through environment variables, CLI flags, and YAML manifests with sensible production defaults.

| Subsystem | Key Environment Variables | Default Value | Crate Documentation |
|---|---|---|---|
| **PostgreSQL Database** | `DATABASE_URL`<br>`DATABASE_MAX_CONNECTIONS`<br>`DATABASE_MIN_CONNECTIONS` | `postgres://logrix:logrix@localhost:5432/logrix`<br>`10`<br>`2` | [`logrix-store-postgres`](crates/logrix-store-postgres) |
| **RabbitMQ Queue** | `RABBITMQ_URL`<br>`RABBITMQ_PREFETCH`<br>`RABBITMQ_TIMEOUT_SECS` | `amqp://logrix:logrix@localhost:5672/%2f`<br>`100`<br>`10s` | [`logrix-queue-rabbitmq`](crates/logrix-queue-rabbitmq) |
| **AWS SQS Queue** | `QUEUE_DRIVER=sqs`<br>`SQS_LIVE_URL`<br>`SQS_WAIT_TIME_SECONDS`<br>`SQS_MAX_MESSAGES` | `rabbitmq`<br>`""` (Required if SQS)<br>`2s`<br>`1` | [`logrix-queue-sqs`](crates/logrix-queue-sqs) |
| **JSON-RPC Gateway** | `RPC_URL`<br>`RPC_FALLBACK_URLS`<br>`RPC_TIMEOUT_SECS`<br>`RPC_CB_THRESHOLD`<br>`RPC_COOLDOWN_SECS` | `https://sepolia-rollup.arbitrum.io/rpc`<br>None<br>`10s`<br>`5`<br>`30s` | [`logrix-rpc-gateway`](crates/logrix-rpc-gateway) |
| **EVM Log Extraction** | `CHAIN_ID`<br>`CHUNK_SIZE`<br>`CHAIN_HEAD_POLL_MS`<br>`MAX_QUEUE_DEPTH` | `421614`<br>`10`<br>`1500ms`<br>`1000` | [`logrix-chain-evm`](crates/logrix-chain-evm) |
| **Gap Reconciler & Reorg** | `RING_BUFFER_DEPTH`<br>`RECONCILER_INTERVAL_SECS`<br>`RECONCILER_CHUNK_SIZE`<br>`RECONCILER_LAG_THRESHOLD` | `128`<br>`30s`<br>`10`<br>`50` | [`logrix-reconciler`](crates/logrix-reconciler) |
| **WASM Handlers** | `WASM_FUEL_LIMIT`<br>`WASM_MAX_MEMORY_MB`<br>`MANIFEST_PATH` | `1,000,000`<br>`128 MB`<br>None | [`logrix-handlers`](crates/logrix-handlers) |
| **GraphQL API Server** | `GRAPHQL_MAX_DEPTH`<br>`GRAPHQL_MAX_COMPLEXITY`<br>`GRAPHQL_DEFAULT_LIMIT`<br>`GRAPHQL_MAX_LIMIT`<br>`GRAPHQL_QUERY_TIMEOUT_SECS` | `7`<br>`200`<br>`100`<br>`1000`<br>`5s` | [`logrix-api`](crates/logrix-api) |
| **Webhook Dispatcher** | `ENABLE_WEBHOOKS`<br>`WEBHOOK_URL`<br>`WEBHOOK_SECRET`<br>`WEBHOOK_TIMEOUT_SECS`<br>`WEBHOOK_MAX_RETRIES` | `false`<br>None<br>`""`<br>`5s`<br>`1` | [`logrix-webhook`](crates/logrix-webhook) |
| **Cloud Object Storage** | `S3_BUCKET`<br>`S3_ENDPOINT`<br>`S3_PREFIX`<br>`AWS_REGION` | None<br>None<br>None<br>`us-east-1` | [`logrix-blob-s3`](crates/logrix-blob-s3) |
| **Fault Tolerance Primitives** | Circuit breakers, Token bucket rate limiting, Jitter backoff | Tuned per subsystem | [`logrix-resilience`](crates/logrix-resilience) |
| **Unified CLI Runner** | Full flag & subcommand reference | See CLI guide | [`logrix-cli`](crates/logrix-cli) |

For complete per-crate options, custom programmatic builders, and architecture details, refer to each individual crate README linked above.

### Core safety and adapter boundaries

`logrix-core` contains domain models, typed errors, and pure port traits only. Runtime block ranges use `BlockRangeJob::try_new`, checkpoints use `Checkpoint::try_new`, and every `MessageHandle` carries the concrete source `QueueType`. Kubernetes Lease coordination is implemented by the CLI infrastructure adapter with valid TLS, bounded requests, fail-closed errors, and optimistic `resourceVersion` renewal.

---

## Monitoring & Dashboards

### 1. Logrix Studio (Web Dashboard & GraphQL Explorer)
Interactive web dashboard served directly by the API container at **`http://localhost:4000/`** (or `/graphiql`):
- Schema explorer, interactive query builder, and entity inspection.
- Real-time event streaming and live mutations over WebSockets (`/ws`).
- Access via port-forward:
  ```bash
  kubectl port-forward svc/my-indexer-logrix-api 4000:4000
  ```

### 2. Pre-Built Grafana Dashboard
A production-ready Grafana dashboard is included at [`deploy/dashboards/logrix-overview.json`](deploy/dashboards/logrix-overview.json). Import it directly into Grafana to monitor:
- **Indexed Head Height**: Live block height vs chain head.
- **Ingestion Throughput**: Rate of indexed events per second (`rate(logrix_events_indexed_total[1m])`).
- **Database Connection Pool**: Active vs idle connections in PostgreSQL.
- **Reorg & Rollback Activity**: Live alert counts on chain reorganizations (`logrix_reorg_events_total`).
- **Dynamic Entity Volume**: Cumulative custom entity counts (`logrix_entities_total`).

### 3. Kubernetes Cluster Inspection
Monitor pods and streaming processing logs with standard `kubectl`:
```bash
# Check pod status and scaling
kubectl get pods -l app.kubernetes.io/instance=my-indexer

# Stream processor ingestion logs
kubectl logs -f deployment/my-indexer-logrix-processor

# View KEDA autoscaling status
kubectl get scaledobject
```

### 4. Prometheus Metrics & Operator Integration
The API service exposes Prometheus metrics at `http://localhost:4000/metrics`.

To enable automatic scraping via Prometheus Operator in your Helm values:
```yaml
serviceMonitor:
  enabled: true
  interval: 15s
```

---

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
