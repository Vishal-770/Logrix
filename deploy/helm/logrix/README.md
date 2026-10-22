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

For production cloud environments, use `values-aws.yaml` to connect to managed AWS infrastructure (Amazon Aurora PostgreSQL, Amazon SQS, Amazon S3, and AWS IAM IRSA):

```bash
helm install logrix deploy/helm/logrix \
  -n logrix \
  -f deploy/helm/values-aws.yaml \
  --set config.existingSecret="logrix-secrets"
```

The referenced Kubernetes Secret must already exist (or be synchronized by an
External Secrets controller). At minimum, provide `DATABASE_URL`,
`RABBITMQ_URL` when RabbitMQ is used, and `WEBHOOK_SECRET` when webhook
delivery is enabled. Keep RPC provider tokens in a Secret-backed environment
variable and reference them from `config.rpcUrl`, for example:
`https://provider.example/${RPC_API_TOKEN}`.

---

## 4. Configuration Reference

### Global & Image Parameters

| Parameter | Type | Default | Description |
|---|---|---|---|
| `image.repository` | string | `ghcr.io/vishal-770/logrix` | Docker image repository |
| `image.tag` | string | `0.7.3` | Immutable image version tag |
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
