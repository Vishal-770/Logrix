# Logrix source Helm chart

This application chart deploys separate Logrix processes. It has no chart
dependencies and installs no operators. See [deployment overview](../../README.md),
[runtime configuration](../../../docs/configuration.md), and the
[AWS operations guide](../../../docs/AWS_EKS_OPERATIONS_GUIDE.md).

`Chart.yaml` declares version/appVersion `0.7.3`. `image.tag` defaults to `latest`,
so appVersion is only an image-tag fallback when the tag is empty. These are
source settings; published images/OCI charts and their accessibility have not
been verified. Examples use the local source chart and an operator-verified image.

## Prerequisites and default resources

Use a Kubernetes cluster supporting `apps/v1`, `policy/v1`, and `autoscaling/v2`,
Helm (CI uses `3.14.0`), kubectl, sufficient CPU/memory, and registry access.
Install compatible operators independently, or disable the corresponding values:

| Feature | Render condition | External requirement |
| --- | --- | --- |
| Processor ScaledObject | `processor.enabled && processor.keda.enabled` (both true by default) | KEDA CRDs, operator, metrics adapter, queue connectivity and scaler credentials |
| Webhook ScaledObject | `webhook.enabled && webhook.autoscaling.enabled` | KEDA; this template only supports RabbitMQ |
| API HPA | `api.enabled && api.hpa.enabled` (both true) | Resource metrics API, usually metrics-server, and container resource requests |
| ServiceMonitor | `serviceMonitor.enabled` (false by default, true in AWS profile) | Prometheus Operator CRD and Prometheus selectors matching its labels |

No CRD-presence test suppresses these resources. A successful `helm template`
does not establish that the target cluster can accept or operate them. The
ServiceMonitor can render even when the API is disabled; it scrapes only the API
Service's `graphql` port at `/metrics`, not each worker.

By default the chart renders ingester (2 replicas), processor (1 initial replica),
API (replica count controlled by HPA, min 2/max 10), one demo PostgreSQL, one demo
RabbitMQ, a shared service account, lease RBAC, and API/ingester PDBs. Webhook and
MinIO are disabled. Workers have no HTTP probes; only API has `/healthz` and
`/readyz` probes. No migration hook, Ingress, TLS, NetworkPolicy, PVC, or operator
installation is included.

### Ingester leadership is not production HA

The default two replicas plus `leaderElection.enabled=true` are unsafe as an HA
guarantee. In `crates/logrix-core/src/lease.rs`, the elector only GETs a Lease:
it never creates/updates it or checks expiry. A non-successful or unparseable
response falls through to assuming leadership; transport errors return standby.
It also disables API certificate verification. The supplied Role granting lease
permissions does not make the implementation safe.

Use `ingester.replicas=1` and `ingester.leaderElection.enabled=false` pending a
proper coordination implementation. Prevent old/new pod overlap during upgrades
with a controlled stop/start or an operator-managed deployment patch; this chart
has no deployment-strategy value. Use distinct lease names for independent
releases if evaluating election; the default name is shared within a namespace.

## Validate and deploy a small demo

All paths/commands below start at the repository root. Choose an image that you
have built/published or otherwise verified; `reviewed-release` is a placeholder.

```bash
export LOGRIX_IMAGE_REPOSITORY=registry.example.com/team/logrix
export LOGRIX_IMAGE_TAG=reviewed-release
helm lint deploy/helm/logrix
helm template logrix deploy/helm/logrix --namespace logrix > /dev/null
helm lint deploy/helm/logrix -f deploy/helm/values-aws.yaml
helm template logrix deploy/helm/logrix --namespace logrix \
  -f deploy/helm/values-aws.yaml > /dev/null
```

Prepare an operator-owned `demo-values.yaml` (only non-secret settings):

```yaml
ingester:
  replicas: 1
  leaderElection:
    enabled: false
processor:
  replicas: 1
  keda:
    enabled: false
api:
  replicas: 1
  hpa:
    enabled: false
podDisruptionBudget:
  enabled: false
```

For a fresh split-service deployment, provision backing services/config first:

```bash
helm upgrade --install logrix ./deploy/helm/logrix \
  --namespace logrix --create-namespace -f demo-values.yaml \
  --set-string image.repository="$LOGRIX_IMAGE_REPOSITORY" \
  --set-string image.tag="$LOGRIX_IMAGE_TAG" \
  --set ingester.enabled=false --set processor.enabled=false --set api.enabled=false
kubectl rollout status deployment/logrix-postgres --namespace logrix
kubectl rollout status deployment/logrix-rabbitmq --namespace logrix
```

Run the [migration Job](../../README.md#database-initialization-for-split-services)
with the same image and `logrix-config`. For this demo only, omit that example's
`secretRef` unless you created `logrix-secrets`; the ConfigMap supplies the known
demo database URL. Then enable apps using the complete desired values:

```bash
helm upgrade --install logrix ./deploy/helm/logrix \
  --namespace logrix -f demo-values.yaml \
  --set-string image.repository="$LOGRIX_IMAGE_REPOSITORY" \
  --set-string image.tag="$LOGRIX_IMAGE_TAG"
kubectl rollout status deployment/logrix-api --namespace logrix
kubectl get pods --namespace logrix
kubectl port-forward --namespace logrix svc/logrix-api 4000:4000
```

The demo still uses public Arbitrum Sepolia RPC/chain/contract defaults and
ephemeral backing services. Inspect worker logs and DB checkpoints to verify
indexing; API readiness alone does not demonstrate worker progress. Remove with
`helm uninstall logrix --namespace logrix`; demo DB/queue/object data is not durable.

For external services, use `builtIn.enabled=false`, keep `localDev.enabled=false`,
set `config.existingSecret` to a precreated Secret, and provide the correct queue
driver and non-secret endpoints. Run migrations before enabling the apps. The
[AWS profile](../values-aws.yaml) is an example with placeholder account IDs,
queue URLs, IAM ARN, and a manifest referencing a WASM file it does not supply;
do not deploy it unchanged.

## Supported chart controls

Names below are relative to values.yaml. Defaults are chart defaults, not a
complete statement of CLI defaults. Helm accepts arbitrary values, so an
unrecognized value can be silently unused.

| Values | Default | Actual template behavior |
| --- | --- | --- |
| `global.nameOverride`, `global.fullnameOverride` | empty | Affect resource names/labels; examples assume release `logrix` and no overrides |
| `image.repository`, `tag`, `pullPolicy` | `ghcr.io/vishal-770/logrix`, `latest`, `IfNotPresent` | Shared by all app Deployments; no independent worker image or digest control |
| `config.existingSecret` | empty | Appends the same Secret via envFrom to all app Deployments |
| `ingester.enabled`, `replicas` | true, 2 | Render toggle and literal replica count |
| `ingester.leaderElection.enabled`, `leaseName` | true, `logrix-ingester-lease` | Explicit ingester env; see leadership limitation above |
| `ingester.affinity` | preferred pod anti-affinity by component/hostname | Only the ingester exposes an affinity value |
| `processor.enabled`, `replicas` | true, 1 | Literal initial replicas even when KEDA is enabled |
| `processor.terminationGracePeriodSeconds` | 60 | Defaults back to 60 for numeric zero; fixed preStop `sleep 5` |
| `api.enabled`, `replicas`, `port` | true, 2, 4000 | Replicas omitted when HPA is on; port drives command, container port, probes, scrape annotation |
| `api.service.type`, `port` | `ClusterIP`, 4000 | Service port is independently configurable; target is named container port `graphql` |
| `webhook.enabled`, `replicaCount` | false, 1 | Renders `logrix webhook-dispatcher`; also emits `ENABLE_WEBHOOKS=true` for all apps |
| `ingester.resources`, `processor.resources`, `api.resources`, `webhook.resources` | See table below | Forwarded to each component unchanged |
| `serviceAccount.create`, `name`, `annotations` | true, `logrix-sa`, `{}` | One account used by all apps; when not created, it must already exist |
| `rbac.create` | true | Namespace Role/RoleBinding for leases independent of election being enabled |
| `podDisruptionBudget.enabled`, `minAvailable` | true, 1 | PDBs for API and ingester even if those apps are disabled; numeric zero becomes 1 |
| `serviceMonitor.enabled`, `interval`, `scrapeTimeout`, `labels` | false, `15s`, `10s`, `{}` | API-only monitor, no Prometheus/operator install |

| Component | Requests CPU/memory | Limits CPU/memory |
| --- | --- | --- |
| Ingester | `100m` / `128Mi` | `500m` / `512Mi` |
| Processor | `250m` / `256Mi` | `1000m` / `1Gi` |
| API | `200m` / `256Mi` | `1000m` / `1Gi` |
| Webhook | `100m` / `128Mi` | `500m` / `512Mi` |

The API command is **`logrix api --port <api.port>`**, matching the subcommand's
port flag. Root options such as `--database-max-connections` precede `api` when
running manually: `logrix --database-max-connections 12 api --port 4000`.
Changing `api.service.port` does not change the listening port. A Secret `PORT`
does not override the explicit Helm `--port` argument.

### Runtime values forwarded by the ConfigMap

All entries below are under `config`. Non-empty endpoint values can expose
credentials in a ConfigMap; put sensitive replacements in `existingSecret`.
See the canonical runtime reference for option validation and process consumers.

| Value(s) | Chart default(s) | Environment mapping / condition |
| --- | --- | --- |
| `chainId`, `contractAddress`, `rpcUrl` | `421614`, Arbitrum Sepolia USDC address, `https://sepolia-rollup.arbitrum.io/rpc` | `CHAIN_ID`, `CONTRACT_ADDRESS`, `RPC_URL` |
| `rpcFallbackUrls` | empty string | `RPC_FALLBACK_URLS`, comma-separated |
| `queueDriver` | `rabbitmq` | `QUEUE_DRIVER`; template comparisons to `sqs` are case-sensitive even though the runtime accepts case-insensitive SQS |
| `databaseUrl` | empty | `DATABASE_URL`; empty produces `postgres://logrix:logrixpassword@<fullname>-postgres:5432/logrix` even when no built-in DB is rendered |
| `rabbitmqUrl` | empty | `RABBITMQ_URL`; empty produces `amqp://logrix:logrixpassword@<fullname>-rabbitmq:5672/%2f` even with built-ins off |
| `databaseMaxConnections`, `databaseMinConnections`, `maxQueueDepth` | 10, 2, 1000 | `DATABASE_MAX_CONNECTIONS`, `DATABASE_MIN_CONNECTIONS`, `MAX_QUEUE_DEPTH` |
| `sqsLiveUrl`, `sqsBackfillUrl`, `sqsWebhookUrl`, `sqsDlqUrl` | all empty | `SQS_LIVE_URL`, `SQS_BACKFILL_URL`, `SQS_WEBHOOK_URL`, `SQS_DLQ_URL`; group emitted only if driver is exactly `sqs` or live URL is truthy |
| `sqsEndpoint` | empty | `SQS_ENDPOINT` only if non-empty and the SQS group above is emitted |
| `s3Bucket` | absent from defaults; AWS profile adds empty | `S3_BUCKET` when non-empty or built-in MinIO is enabled; fallback bucket `logrix-blocks` |
| `s3Endpoint` | empty | `S3_ENDPOINT` only within the S3 bucket/MinIO condition; otherwise omitted even if an endpoint alone is set |
| `startBlock`, `cuBudget`, `bulkStreamUrl`, `webhookUrl` | all empty | Optional `START_BLOCK`, `CU_BUDGET`, `BULK_STREAM_URL`, `WEBHOOK_URL`, each only if truthy |
| `chunkSize` | 10 | `CHUNK_SIZE` |
| `reconcilerIntervalSecs`, `ringBufferDepth` | 30, 128 | `RECONCILER_INTERVAL_SECS`, `RING_BUFFER_DEPTH` (directly quoted, no template fallback) |
| `reconcilerChunkSize` | empty | `RECONCILER_CHUNK_SIZE` only if truthy |
| `reconcilerLagThreshold`, `chainHeadPollMs` | 50, 1500 | `RECONCILER_LAG_THRESHOLD`, `CHAIN_HEAD_POLL_MS` |
| `rabbitmqPrefetch`, `rabbitmqTimeoutSecs` | 100, 10 | `RABBITMQ_PREFETCH`, `RABBITMQ_TIMEOUT_SECS` |
| `sqsWaitTimeSeconds`, `sqsMaxMessages` | 2, 1 | `SQS_WAIT_TIME_SECONDS`, `SQS_MAX_MESSAGES` |
| `wasmFuelLimit`, `wasmMaxMemoryMb` | 1000000, 128 | `WASM_FUEL_LIMIT`, `WASM_MAX_MEMORY_MB` |
| `rpcTimeoutSecs`, `rpcCbThreshold`, `rpcCooldownSecs` | 10, 5, 30 | `RPC_TIMEOUT_SECS`, `RPC_CB_THRESHOLD`, `RPC_COOLDOWN_SECS` |
| `webhookTimeoutSecs`, `webhookMaxRetries` | 5, 1 | `WEBHOOK_TIMEOUT_SECS`, `WEBHOOK_MAX_RETRIES` |
| `graphqlMaxDepth`, `graphqlMaxComplexity` | 7, 200 | `GRAPHQL_MAX_DEPTH`, `GRAPHQL_MAX_COMPLEXITY` |
| `graphqlDefaultLimit`, `graphqlMaxLimit`, `graphqlQueryTimeoutSecs` | 100, 1000, 5 | `GRAPHQL_DEFAULT_LIMIT`, `GRAPHQL_MAX_LIMIT`, `GRAPHQL_QUERY_TIMEOUT_SECS` |
| `manifestPath`, `schemaPath` | empty | `MANIFEST_PATH` / `SCHEMA_PATH` only if path or corresponding inline content is supplied |
| `manifestContent`, `schemaContent` | empty | Stored in ConfigMap and mounted as files as described below |
| `wasmBinary`, `wasmBase64` | empty | Generate a separate WASM Secret, not runtime env |

Helm's `default` treats numeric `0`, false, empty strings, and null as empty.
Thus numeric `config.sqsWaitTimeSeconds: 0`, `config.databaseMinConnections: 0`,
or `config.webhookMaxRetries: 0` is replaced with 2, 2, or 1 respectively.
Truthiness gates omit numeric `startBlock: 0`, `cuBudget: 0`, and
`reconcilerChunkSize: 0`. A quoted string `"0"` is non-empty and survives these
gates/defaults, but must still be valid for the runtime or Kubernetes field.
Inspect rendered output rather than assuming YAML zero always disables a feature.

There is **no `config.s3Prefix` forwarding** and no generic extra-env value.
Supply exact `S3_PREFIX` through `existingSecret` if needed; likewise use that
Secret or a consumer-managed pod patch for unsupported runtime environment
settings such as `WEBHOOK_SECRET`, `AWS_REGION`, and webhook worker timing.
Secret entries override duplicate ConfigMap envFrom keys; the ingester's explicit
`POD_NAME`, `K8S_NAMESPACE`, `ENABLE_LEADER_ELECTION`, and `LEASE_NAME` take precedence
over both. Secrets do not affect Helm's resource-rendering conditions: a DB URL
in a Secret does not automatically suppress the built-in DB.

### Manifest, schema, and handler files

Use non-secret local files with Helm `--set-file`, for example:

```bash
# Add to the reviewed helm template/upgrade invocation.
--set-file config.manifestContent=manifest.yaml \
--set-file config.schemaContent=schema.graphql \
--set-file config.wasmBinary=handlers/mapping.wasm
```

Inline manifest/schema files mount at `/etc/logrix/manifest.yaml` and
`/etc/logrix/schema.graphql` on processor, API, and enabled webhook pods, not the
ingester. Their default env paths point there. Explicit `manifestPath`/`schemaPath`
change only env; they do not relocate the mounted files or create a new mount.
**This is a startup blocker for inline manifests:** the ingester receives
`MANIFEST_PATH` and reads it before running, so a default SDK-exported bundle
fails there without a consumer-managed volume/mount patch (or another readable
manifest at that path). This chart has no ingester extra-volume/mount value.
Path-only values require files already present in your chosen image or a
consumer-managed mount. Ingestion start position is controlled by `START_BLOCK`
and DB checkpoints, not the manifest's contract `start_block` field.

The WASM Secret's `mapping.wasm` mounts only on processors at
`/etc/logrix/handlers/mapping.wasm`. `wasmBinary` is base64-encoded by the template
and wins over `wasmBase64` when both are set; `wasmBase64` must already be encoded.
Match the manifest's handler path to this mount. Keep payloads below Kubernetes
ConfigMap/Secret size limits. A path in a manifest alone does not ship the file.

### Autoscaling controls and gaps

| Values | Defaults / behavior |
| --- | --- |
| `processor.keda.enabled` | true; independent of operator installation |
| `processor.keda.minReplicaCount`, `maxReplicaCount` | 1 / 50; numeric min 0 becomes 1 through `default` |
| `processor.keda.pollingInterval`, `cooldownPeriod`, `targetQueueDepth` | 15s / 300s / 30; zero falls back to these values |
| `processor.keda.queueType` | `rabbitmq`; SQS is chosen if this is `sqs` OR `config.queueDriver` is `sqs` |
| `processor.keda.rabbitmq.queueName` | `logrix.live.jobs`; runtime default live queue is **`logrix.blocks.live`**, so override it |
| `processor.keda.rabbitmq.host` | `amqp://logrix:logrix@rabbitmq:5672/%2f`; this non-empty default does not match built-in host/password. Empty uses the built-in URL fallback |
| `processor.keda.sqs.queueUrl`, `awsRegion` | empty / `us-east-1`; set independently of `config.sqsLiveUrl` and the app SDK region |
| `api.hpa.minReplicas`, `maxReplicas`, `targetCPUUtilizationPercentage` | 2 / 10 / 80; numeric zero triggers template defaults |
| `api.hpa.targetMemoryUtilizationPercentage` | Not in values.yaml; emits an additional memory target when truthy |
| `webhook.autoscaling.enabled`, `minReplicas`, `maxReplicas`, `targetQueueDepth` | true / 0 / 10 / 50; active only if webhook is enabled; min 0 is emitted literally |

The processor scaler watches one queue, not aggregate live/backfill backlog.
SQS uses `identityOwner: operator` with no TriggerAuthentication resource: the
**KEDA operator** needs appropriate AWS identity and queue permissions. The app
service account's IRSA annotation does not configure the operator.

Webhook scaling is fixed to RabbitMQ queue `logrix.blocks.webhook`, polling 15s,
cooldown 300s, and the literal `config.rabbitmqUrl` (empty by default; no built-in
fallback). It does not switch to SQS with the AWS profile. Disable
`webhook.autoscaling.enabled` or manage a suitable scaler separately for SQS.
Setting `webhook.enabled=true` also enables embedded webhook dispatch in each
processor; a separate webhook Deployment is not the only consumer created.
Manual scaling can be overwritten by HPA/KEDA or subsequent Helm upgrades.

### Built-in backing services and unused controls

| Value | Default | Actual condition |
| --- | --- | --- |
| `builtIn.enabled` | true | Master switch for normal built-in conditions |
| `builtIn.postgres` | true | Render if enabled and `config.databaseUrl` is empty |
| `builtIn.rabbitmq` | true | Render if enabled, `config.rabbitmqUrl` empty, and driver is not exactly `sqs` |
| `builtIn.minio` | false | Render if enabled and `config.s3Endpoint` empty; setting `s3Bucket` does not suppress it |
| `localDev.enabled` | false | Backward-compatible OR branch that forces PostgreSQL and RabbitMQ even with built-ins disabled/external endpoints; does not enable MinIO |

All three are one-replica Deployments and ClusterIP Services with fixed resource
requests `100m`/`128Mi`, limits `500m`/`512Mi`. PostgreSQL uses `postgres:16-alpine`;
RabbitMQ uses `rabbitmq:3.13-management-alpine`; MinIO uses
`minio/minio:RELEASE.2024-03-30T09-41-56Z`. App image settings do not change them.
The fixed demo user is `logrix`, password `logrixpassword`, and DB is `logrix`.
No volumes or initialization Jobs are rendered. In particular, enabling MinIO
emits bucket `logrix-blocks` and its endpoint env but **does not create that
bucket**, set a region, or inject `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` into
clients. Provision the bucket and client configuration separately for a demo.

These declared values are currently **unused**:

- `global.imagePullSecrets`: not forwarded to any pod. For private registries,
  arrange pull credentials on the shared existing service account (for example
  with `serviceAccount.create=false`) or manage pod patches externally.
- `builtIn.postgresStorage: 1Gi`, `rabbitmqStorage: 1Gi`, `minioStorage: 5Gi`: no PVC
  or mount uses them; they do not enable persistence.
- `api.ingress.*`: enabled, className, annotations, hosts, and paths are declared
  but there is no ingress template. Supply an independent Ingress/controller/TLS
  configuration if exposing the API.

All app pods share `serviceAccount.name`. One IRSA role can be annotated there;
per-component Terraform roles are not automatically selected. Choosing the
processor role for that shared account grants its permissions to API, ingester,
and webhook pods too. See the [Terraform handoff](../../terraform/aws/README.md)
before treating that example as least-privilege isolation.

Changes to envFrom ConfigMaps/Secrets do not update running process env. There
are no checksum-triggered pod rollout annotations; plan an explicit restart.
Manifest/Secret volume projection alone does not make the application reload
configuration. Retain backups and coordinate ingester shutdown during upgrades;
PDBs do not guarantee ingestion exclusivity or zero downtime.
