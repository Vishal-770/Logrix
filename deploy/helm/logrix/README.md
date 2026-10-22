# Logrix Helm Chart

Production-grade, highly-configurable Helm chart for deploying the Logrix modular EVM blockchain indexing engine on Kubernetes.

---

## 1. Architectural Model & Workloads

Logrix uses containerized, role-separated deployments following hexagonal boundaries:

| Workload | Role | Scaling & Redundancy | Default Resources |
|---|---|---|---|
| **`logrix-ingester`** | Connects to EVM RPC nodes, tracks head, detects reorgs, publishes block jobs to queues. | Active/Standby with Lease leader election. | 100m - 500m CPU, 128Mi - 512Mi RAM |
| **`logrix-processor`** | Consumes queue jobs, runs custom WASM mapping handlers, atomically commits events and checkpoints to Postgres. | Elastic event-driven autoscaling via KEDA (`ScaledObject`). | 250m - 1000m CPU, 256Mi - 1Gi RAM |
| **`logrix-api`** | Serves GraphQL queries, subscriptions, and metrics. | Horizontal Pod Autoscaling (HPA) on CPU load. | 200m - 1000m CPU, 256Mi - 1Gi RAM |
| **`logrix-webhook`** | Consumes webhook queue jobs and delivers signed HMAC-SHA256 HTTP notifications. | Optional standalone deployment or KEDA scaling. | 100m - 500m CPU, 128Mi - 512Mi RAM |

---

## 2. Quickstart: Local / Staging Deployment (Zero External Dependencies)

Out of the box, `builtIn.enabled: true` automatically provisions in-cluster PostgreSQL (16), RabbitMQ (3.13), and MinIO (S3 compatible) pods.

```bash
# Add namespace
kubectl create namespace logrix

# Install Logrix with default in-cluster backing infrastructure
helm install logrix deploy/helm/logrix --namespace logrix
```

To forward and test the GraphQL API locally:
```bash
kubectl port-forward svc/logrix-api 4000:4000 -n logrix
# Query interface available at http://localhost:4000
```

---

## 3. Production Deployment: AWS EKS with Aurora, SQS, and S3

In production cloud environments, Logrix runs exclusively on managed AWS infrastructure (Amazon Aurora Serverless v2 PostgreSQL, Amazon SQS, and Amazon S3) with zero local container dependencies. Live deployments strictly consume the remote published OCI Helm chart (`oci://ghcr.io/vishal-770/charts/logrix`).

### Step 1: Connect `kubectl` to EKS Cluster
```bash
aws eks update-kubeconfig --region <aws-region> --name <cluster-name>
kubectl get nodes -o wide
```

### Step 2: Install KEDA Autoscaler (Required for SQS queue-driven scaling)
If queue-driven dynamic worker scaling (`processor.keda.enabled: true`) is enabled, ensure KEDA CRDs are present on the cluster:
```bash
helm repo add kedacore https://kedacore.github.io/charts
helm repo update
helm install keda kedacore/keda --namespace keda --create-namespace
```

### Step 3: Create Kubernetes Secrets
Kubernetes injects credentials as environment variables (`DATABASE_URL`, `RPC_URL`, `RPC_FALLBACK_URLS`) into pods. Keys must use uppercase and underscores so standard POSIX shells and Rust `clap` parse them accurately.

When using Aurora Serverless v2 PostgreSQL inside a private VPC subnet, strip any trailing `?sslmode=require` query parameters:

```bash
# 1. Fetch Aurora connection string from AWS Secrets Manager
RAW_DB_URL=$(aws secretsmanager get-secret-value \
  --region <aws-region> \
  --secret-id "<rds-secret-arn>" \
  --query SecretString --output text | jq -r .url)

# 2. Strip ?sslmode=require query parameter for internal VPC connectivity
CLEAN_DB_URL="${RAW_DB_URL%\?*}"

# 3. Create namespace and secret vault
kubectl create namespace logrix

kubectl create secret generic logrix-secrets -n logrix \
  --from-literal=DATABASE_URL="$CLEAN_DB_URL" \
  --from-literal=RPC_URL="https://eth-mainnet.g.alchemy.com/v2/<API_KEY>" \
  --from-literal=RPC_FALLBACK_URLS="https://rpc.payload.de,https://eth.llamarpc.com"
```

### Step 4: Run Initial Database Migrations
Before deploying the long-running ingester and processor workloads, run the Logrix database schema migration using a one-off ephemeral pod:

```bash
kubectl run logrix-migrator --rm -i --restart=Never \
  --image=ghcr.io/vishal-770/logrix:0.8.0 \
  -n logrix \
  --env="DATABASE_URL=$CLEAN_DB_URL" \
  -- command logrix migrate
```
This idempotently provisions `logrix_checkpoints`, `raw_events`, `token_transfers`, and required database indices.

### Step 5: Deploy Using Published Remote OCI Helm Chart
Deploy from the published OCI registry, passing dynamic cloud resource endpoints without baking infrastructure identifiers into git:

```bash
helm upgrade --install logrix oci://ghcr.io/vishal-770/charts/logrix \
  --version 0.8.2 \
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

### Step 6: Verify Deployment and Indexing

1. Check all pods are `1/1 Ready` and `Running`:
   ```bash
   kubectl get pods -n logrix -o wide
   ```

2. Tail live block ingestion logs:
   ```bash
   kubectl logs -n logrix -l app.kubernetes.io/component=ingester --tail=30 -f
   ```

3. Tail processor database commits and S3 uploads:
   ```bash
   kubectl logs -n logrix -l app.kubernetes.io/component=processor --tail=30 -f
   ```

4. Port-forward the GraphQL API:
   ```bash
   kubectl port-forward svc/logrix-api 4000:4000 -n logrix
   ```
   Query health and checkpoint in another terminal:
   ```bash
   curl -s -X POST http://localhost:4000/graphql \
     -H "Content-Type: application/json" \
     -d '{"query": "{ health { status version } checkpoint(chainId: 1) { chainId lastIndexedBlock lastBlockHash } }"}'
   ```

### Troubleshooting & Common Gotchas
- **Liveness & Readiness Probes in Minimal Images**: Ingester, Processor, and Webhook deployments use `pidof logrix || exit 1` probes which operate reliably inside Debian-slim without needing `procps` (`pgrep`).
- **`invalid value '1e+06' for '--wasm-fuel-limit'`**: Ensure `WASM_FUEL_LIMIT` is formatted as an integer string (`"1000000"`). Chart 0.8.1+ enforces integer float formatting in `configmap.yaml`.
- **EKS Control Plane Client Timeout**: If Helm hangs or emits `context deadline exceeded` while reading discovery bodies, pass `--disable-openapi-validation`.
- **Pod CrashLoopBackOff on Database URL**: Verify `logrix-secrets` contains `DATABASE_URL` (uppercase) pointing to Aurora, without `?sslmode=require`.


---

## 4. Configuration Reference

### Global & Image Parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| `image.repository` | string | `ghcr.io/vishal-770/logrix` | Docker image repository |
| `image.tag` | string | `0.8.0` | Immutable image version tag |
| `image.pullPolicy` | string | `IfNotPresent` | Image pull policy |

### Core Runtime Parameters (`config`)

| Parameter | Type | Default | Description |
|---|---|---|---|
| `config.chainId` | integer | `421614` | Target EVM chain ID |
| `config.contractAddress` | string | `0x75faf114...` | Target smart contract address |
| `config.rpcUrl` | string | `https://sepolia-rollup...` | Primary EVM JSON-RPC endpoint |
| `config.rpcFallbackUrls` | string | `""` | Comma-delimited list of fallback RPC URLs |
| `config.queueDriver` | string | `rabbitmq` | Queue backend driver (`rabbitmq` or `sqs`) |
| `config.databaseMaxConnections` | integer | `10` | Maximum connection pool size for PostgreSQL |
| `config.databaseMinConnections` | integer | `2` | Minimum idle connection pool size |
| `config.databaseStatementTimeoutSecs` | integer | `30` | PostgreSQL statement timeout |
| `config.chunkSize` | integer | `10` | Number of blocks fetched per RPC log query |
| `config.reconcilerIntervalSecs` | integer | `30` | Gap recovery reconciler scan interval |
| `config.ringBufferDepth` | integer | `128` | Depth of in-memory reorg tracking buffer |
| `config.wasmFuelLimit` | integer | `1000000` | WebAssembly fuel limit per transaction |
| `config.wasmMaxMemoryMb` | integer | `128` | WebAssembly maximum runtime memory in MB |
| `config.existingSecret` | string | `""` | External Secret name for runtime credentials; expected keys include `DATABASE_URL`, `RABBITMQ_URL`, and `WEBHOOK_SECRET` when applicable |
| `config.manifestContent` | string | `""` | Inline `manifest.yaml` content mounted to `/etc/logrix/manifest.yaml` |
| `config.schemaContent` | string | `""` | Inline `schema.graphql` content mounted to `/etc/logrix/schema.graphql` |
| `config.wasmBase64` | string | `""` | Pre-encoded base64 WASM binary injected to `/etc/logrix/handlers/mapping.wasm` |

### Security & IAM (`serviceAccount`)

| Parameter | Type | Default | Description |
|---|---|---|---|
| `serviceAccount.create` | boolean | `true` | Create default service account |
| `serviceAccount.name` | string | `logrix-sa` | Default service account name |
| `ingester.serviceAccount.create` | boolean | `false` | Create dedicated ServiceAccount for Ingester |
| `ingester.serviceAccount.annotations` | object | `{}` | AWS IRSA role ARN for Ingester |
| `processor.serviceAccount.create` | boolean | `false` | Create dedicated ServiceAccount for Processor |
| `processor.serviceAccount.annotations` | object | `{}` | AWS IRSA role ARN for Processor |
| `api.serviceAccount.create` | boolean | `false` | Create dedicated ServiceAccount for API |
| `webhook.serviceAccount.create` | boolean | `false` | Create dedicated ServiceAccount for Webhook |
| `networkPolicy.enabled` | boolean | `false` | Create default ingress/egress network policies |

---

## 5. Automated Rollouts & Resilience

- **Configuration Hash Tracking**: Pod templates include `checksum/config` and `checksum/wasm` metadata annotations. Upgrading values via `helm upgrade` triggers an automated rolling restart across all affected workloads.
- **Node Drain Safety**: PodDisruptionBudgets (`pdb.yaml`) ensure that at least one API and one Ingester pod remain online during Kubernetes node upgrades and drains.
- **Graceful Termination**: `terminationGracePeriodSeconds` (default 60s on processor, 30s on others) ensures all in-flight blocks and database transactions flush to disk before pods terminate.

---

## 6. Container & Helm Chart Releases

Container images (`ghcr.io/vishal-770/logrix`) and OCI Helm charts (`oci://ghcr.io/vishal-770/charts/logrix`) are published via the dedicated `Release Container & Helm Chart` CI/CD workflow, completely independent of the npm SDK release cycle.

To trigger an automated release:
```bash
git tag v0.7.4
git push origin v0.7.4
```
This builds and pushes multi-architecture images to GitHub Container Registry, packages the Helm chart with the matching version, and publishes the OCI artifact.
