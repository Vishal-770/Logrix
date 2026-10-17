# Logrix Kustomize manifests

These manifests deploy one `logrix all-in-one` process: ingester, processor, and
API share a pod. This is distinct from the split-service
[Helm chart](../helm/logrix/README.md). See the [deployment overview](../README.md)
and canonical [runtime configuration](../../docs/configuration.md).

Use Kustomize (CI builds both directories) or kubectl's embedded Kustomize.
Rendering needs no cluster or cloud credentials. Applying requires access to a
Kubernetes cluster, a verified pre-built image, and reachable PostgreSQL,
RabbitMQ or SQS, and EVM RPC. No SDK deployment step is involved.

## What is actually included

| Directory/resource | Behavior |
| --- | --- |
| `base/configmap.yaml` | `logrix-config`: `CHAIN_ID=421614`, Arbitrum Sepolia public `RPC_URL`, USDC `CONTRACT_ADDRESS=0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d`, `PORT=4000`, `QUEUE_DRIVER=rabbitmq` |
| `base/logrix.yaml` | Deployment `logrix`, replicas 1, image `ghcr.io/vishal-770/logrix:latest`, pull policy `IfNotPresent`, command `["logrix", "all-in-one"]` |
| Base pod environment | Imports ConfigMap `logrix-config`, then required Secret `logrix-secrets`. Secret duplicate keys win |
| Base resources | Requests `250m` CPU / `256Mi`; limits `1000m` CPU / `1Gi` |
| Base probes | `/healthz` on port 4000 (delay 5s/period 10s), `/readyz` (delay 3s/period 5s) |
| Base Service | `logrix-api`, ClusterIP port 4000 -> targetPort 4000, selecting the all-in-one pod |
| `local/kustomization.yaml` | Adds namespace `logrix-dev`, PostgreSQL and RabbitMQ, and a generated Secret with hashed name; references are rewritten by Kustomize |
| Local PostgreSQL | `postgres:16-alpine`, one-replica StatefulSet, `postgres-data` claim requesting 2Gi ReadWriteOnce, default StorageClass |
| Local RabbitMQ | `rabbitmq:3.13-management-alpine`, one-replica Deployment, no persistent volume; Service ports 5672 and 15672 |
| Local service credentials | Demo user/password `logrix` / `logrix` for both, DB `logrix`; Secret uses `postgres://logrix:logrix@postgres:5432/logrix` and `amqp://logrix:logrix@rabbitmq:5672` |
| Local backing-service resources | Each requests `250m` / `256Mi`, limits `1000m` / `1Gi` |

Base creates no namespace, Secret, database, broker, service account, or volumes.
Local adds only the resources above: no MinIO, S3 bucket, SQS queues, LocalStack,
Ingress, HPA, KEDA, ServiceMonitor, operators, or handler files. It does not need
KEDA/Prometheus CRDs or metrics-server as supplied. If you add autoscaling or
monitoring resources yourself, install their prerequisites separately.

The local storage claim needs dynamic provisioning/default StorageClass, or a
matching pre-provisioned PV. The manifests install no CSI driver. DB persistence
does not make the ephemeral RabbitMQ queue durable across pod replacement.
The fixed demo credentials are not appropriate for a production environment.

## Offline validation

From the repository root:

```bash
kustomize build deploy/k8s/base > /dev/null
kustomize build deploy/k8s/local > /dev/null
```

If standalone Kustomize is absent, use:

```bash
kubectl kustomize deploy/k8s/base > /dev/null
kubectl kustomize deploy/k8s/local > /dev/null
```

These are local builds, not server-side validation. They do not prove registry
availability, sufficient capacity, DB/RPC connectivity, or a successful rollout.

## Run the local overlay with a versioned image

The checked-in default `latest` is not a production image selection. Prepare a
consumer-owned overlay at `operator-k8s/kustomization.yaml` relative to the
repository root with the following content:

```yaml
apiVersion: kustomize.config.k8s.io/v1beta1
kind: Kustomization
resources:
  - ../deploy/k8s/local
images:
  - name: ghcr.io/vishal-770/logrix
    newName: registry.example.com/team/logrix
    newTag: reviewed-release
patches:
  - target:
      kind: Deployment
      name: logrix
    patch: |-
      apiVersion: apps/v1
      kind: Deployment
      metadata:
        name: logrix
      spec:
        strategy:
          type: Recreate
```

Replace repository/tag with an image already verified in your registry. For an
immutable digest, use `digest: sha256:<verified-digest>` instead of `newTag`.
The root Dockerfile is a build-machine/CI input, not a build step on a node.

Operator commands after reviewing the overlay:

```bash
kubectl kustomize operator-k8s > /dev/null
kubectl apply -k operator-k8s
kubectl rollout status statefulset/postgres --namespace logrix-dev
kubectl rollout status deployment/rabbitmq --namespace logrix-dev
kubectl rollout status deployment/logrix --namespace logrix-dev
kubectl port-forward --namespace logrix-dev svc/logrix-api 4000:4000
```

Open `http://localhost:4000/` for GraphiQL. The API routes are `/graphql`,
`/healthz`, `/readyz`, and `/metrics`; `/health` and `/ready` are not these HTTP
probe routes. PostgreSQL migrations run at `all-in-one` startup, unlike the Helm
split commands. Dependencies can become ready after the app's first attempt;
inspect pod events/restarts and logs rather than assuming apply order ensures
readiness. Only the API has HTTP probes. Check worker progress independently.

`all-in-one` uses a local leader elector regardless of the Kubernetes election
env setting. Keep one replica and avoid overlapping old/new pods during
updates. The `Recreate` patch deliberately stops the old pod before replacing
it, accepting downtime. The supplied base's default rolling strategy can overlap
pods even with `replicas: 1`; horizontal scaling is not an HA ingestion design.

## Use the base with external services

Create an operator-owned overlay using `../deploy/k8s/base` instead of the local
overlay. Include your namespace resource or precreate the namespace; set
`namespace: logrix` in that overlay. Retain the reviewed image and no-overlap
strategy. Create `logrix-secrets` in that namespace before applying the base.
Supply uppercase env keys via your secret-management mechanism, for example
`DATABASE_URL`, `RABBITMQ_URL` for RabbitMQ, and any sensitive `RPC_URL`.
The [secret bootstrap example](../README.md#secrets-and-configuration-precedence)
uses a private env file without exposing values in CLI arguments.

Override non-secret defaults with patches to `logrix-config`, or a
`configMapGenerator` of the same name with `behavior: merge`. For SQS, set
`QUEUE_DRIVER=sqs` and all relevant `SQS_LIVE_URL`, `SQS_BACKFILL_URL`,
`SQS_WEBHOOK_URL`, `SQS_DLQ_URL`; supply AWS region/identity and optional endpoint
explicitly. The manifests create none of these AWS resources. For S3 archiving,
provide `S3_BUCKET`, optional `S3_ENDPOINT`/`S3_PREFIX`, AWS region/identity, and a
precreated bucket. No Helm-style `config.*` values are interpreted by Kustomize.

There is no custom service account or IRSA annotation in the base. An AWS
consumer overlay must create/select the desired account and attach its reviewed
IAM policy; the split-service Terraform IAM role layout is not automatically
wired here. Private-registry pull credentials also need an account/pod patch.

For custom mappings, generate ConfigMaps/Secrets and patch volume/volumeMounts,
or supply a versioned image containing the files. Set `MANIFEST_PATH` and
`SCHEMA_PATH` to actual in-container paths and make the manifest's handler path
match the mounted WASM. Env paths alone do not distribute files. Avoid placing
secrets in the manifest/ConfigMap. Keep payloads within Kubernetes object limits.

`PORT` is an all-in-one subcommand env option, so it can be overridden via the
Secret. If changed, also patch container port, probe ports, Service port and
targetPort: the supplied manifests fix those at 4000. Manual syntax is
`logrix all-in-one --port 4000`; root options go before `all-in-one`.

## Operations and removal

```bash
kubectl logs --namespace logrix-dev deployment/logrix --tail=100
kubectl get pods,pvc --namespace logrix-dev
kubectl rollout restart deployment/logrix --namespace logrix-dev
```

ConfigMap/Secret env changes require pod replacement; there is no application
hot reload. A Kustomize secretGenerator changes its hash/name when content
changes and rewrites references on the next apply. The fixed base ConfigMap
has no hash/checksum rollout trigger. Do not print real Secret contents or
include them in diagnostics.

Remove using the same operator overlay with `kubectl delete -k operator-k8s`.
Because the local overlay contains a Namespace, deleting it removes namespaced
objects too. StatefulSet PVCs can be retained when deleting just the StatefulSet;
namespace deletion removes PVC objects. PV data retention/deletion depends on
the volume's reclaim policy. Back up required data and check storage independently;
do not assume either automatic retention or complete cleanup. This affects no
external database, SQS, S3, or other cloud infrastructure.
