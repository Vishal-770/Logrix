# Deploying Logrix

Run pre-built, versioned Logrix container images. Prepare charts, manifests, and
configuration on a workstation or in CI; do not clone or compile the project on
production nodes. The TypeScript SDK prepares application configuration and
handlers, not infrastructure or deployments.

The canonical runtime configuration reference is
[`docs/configuration.md`](../docs/configuration.md). Deployment-specific wiring
and limitations are documented here and in the component guides:

| Path | Purpose | Backing services |
| --- | --- | --- |
| [`helm/logrix`](helm/logrix/README.md) | Separate ingester, processor, API, and optional webhook Deployments | Optional ephemeral demo PostgreSQL/RabbitMQ/MinIO, or external services |
| [`k8s`](k8s/README.md) | Kustomize single-replica `all-in-one` Deployment | Base requires external DB/queue; local overlay supplies development services |
| [`terraform/aws`](terraform/aws/README.md) | Reference AWS infrastructure only | VPC, EKS, Aurora, SQS, S3, and IAM |
| [`AWS operations guide`](../docs/AWS_EKS_OPERATIONS_GUIDE.md) | Infrastructure-to-application handoff and operations | Describes current integration gaps |

## Read before deploying

- Helm defaults to **two ingesters with leader election enabled**. The current
  `KubernetesLeaseElector` only reads a Lease; it does not create, renew, expire,
  or atomically acquire it. Missing/error/unparseable Lease responses can make
  multiple pods assume leadership. This is not safe production active/passive
  coordination. Use one ingester, disable this election, and prevent overlapping
  old/new ingesters during updates. A one-replica rolling Deployment alone does
  not prevent rollout overlap. Kustomize `all-in-one` also uses local leadership
  and must not be scaled horizontally as an HA ingestion solution.
- The Helm default processor ScaledObject requires an installed **KEDA operator
  and CRDs**. API HPA requires the **resource metrics API**, usually supplied by
  metrics-server. `serviceMonitor.enabled=true` requires the Prometheus Operator
  ServiceMonitor CRD and an appropriately configured Prometheus installation.
  The AWS values file enables ServiceMonitor; the default chart does not.
- Terraform is reference infrastructure with incomplete application/operator
  compatibility, not a verified production installer. EKS `1.30` and Aurora
  PostgreSQL `16.8` are source settings, not claims of current regional support.
  No live AWS version/support validation is implied by these docs.
- Built-in Helm backing services use fixed demonstration passwords and have no
  persistent volumes. MinIO does not provision a bucket or inject client AWS
  credentials. Use independently operated durable services for persistent data.
- `api.ingress` values do not create an Ingress. All Helm app components share
  one service account and one IRSA annotation; the chart cannot assign the
  Terraform roles separately by component.
- An inline `config.manifestContent` sets `MANIFEST_PATH` in every app, but the
  chart does not mount it into the ingester. The ingester reads that file during
  startup and fails if it is absent. SDK-exported bundles need a consumer-managed
  ingester volume/mount patch or equivalent readable path; they are not deployable
  unchanged with this chart/runtime combination.

## Container contract and artifact provenance

The root Dockerfile builds `logrix` with cargo-chef `0.1.71`, uses a
`rust:1.96.0-bookworm` builder with native build prerequisites, and runs on
`debian:bookworm-slim` as UID `10001`. Its
entrypoint is `/usr/local/bin/logrix`, default command is `all-in-one`, and exposed
port is `4000`. It does not copy your manifest, GraphQL schema, or WASM handler
into the runtime image. Supply those explicitly. The exposed port is metadata,
not a host port mapping.

Build in a controlled build environment when you need your own image, for example:

```bash
# From the repository root, on the build machine.
docker build --tag registry.example.com/team/logrix:reviewed-release .
```

Publish through your registry workflow, verify the image and architecture, and
record its resolved digest before deployment. Helm currently constructs
`repository:tag` and has no dedicated digest value; use a versioned tag with a
registry immutability policy. Kustomize can pin a digest in a consumer overlay.

`Chart.yaml` currently declares chart/app version `0.7.3`, default Helm and
Kustomize images use `latest`, and `deploy/helm/values-aws.yaml` selects `0.7.3`.
Override these defaults with an image you have verified. None of these settings
proves that an image or chart is published or accessible.

`.github/workflows/release.yml` is intended to push images to
`ghcr.io/${github.repository}` and charts below the lowercase owner's
`oci://ghcr.io/<owner>/charts` path. The chart job derives its version from the
Git ref; the workflow's manual `version` input is not used by these steps.
The workflow is source evidence of intent, not evidence of a successful release.
Use the checked-out source chart for the validation and deployment examples here.

The root `docker-compose.yml` starts **backing services only**, not Logrix:
PostgreSQL 16, RabbitMQ 3.13 management, and MinIO; profile `aws` adds LocalStack
3.2 for SQS/S3. It persists the first three services using named volumes and
publishes their ports on the host. It creates no S3 bucket or SQS queues. Its
default DB/queue password is `logrix`, unlike Helm's `logrixpassword`. Do not
transfer one deployment's connection strings to another unchanged.

## Secrets and configuration precedence

Use a namespace-scoped Secret supplied by your secret-management process.
`config.existingSecret` in Helm is a **Secret name**, not a credential or secret
ARN. Kustomize base expects a Secret named `logrix-secrets`.

Both paths import the ConfigMap first and the Secret second with `envFrom`.
Duplicate keys in the Secret win. Keys are exact environment names, such as
`DATABASE_URL`, `RABBITMQ_URL`, `RPC_URL`, `WEBHOOK_SECRET`, `AWS_REGION`, or
`S3_PREFIX`; `database-url` does not set `DATABASE_URL`. Explicit pod `env` entries
(Helm's ingester identity/election entries) override `envFrom`, and explicit CLI
arguments override the associated environment option.

Do not put real credentials or provider-key-bearing URLs in Helm values,
ConfigMaps, `--set`, shell history, or committed YAML/tfvars. Helm release records
and rendered output can contain values and inline WASM. Base64 is not encryption.
For manual bootstrap, a protected env file may be used on the operator machine:

```bash
# Supply this private file using your secret-management process; never commit it.
kubectl create secret generic logrix-secrets --namespace logrix \
  --from-env-file=/secure/operator/logrix-runtime.env
```

This is an operator command, not something Terraform or Helm runs automatically.
Use short-lived AWS identity/IRSA rather than long-lived AWS access keys for AWS.
Set the SDK region explicitly when not supplied by your platform. The Helm
`processor.keda.sqs.awsRegion` value configures the scaler, not the application
SDK region. IAM permission to read Secrets Manager does not populate pod env.

## Database initialization for split services

`all-in-one` runs embedded DB migrations at startup. The separate `ingester`,
`processor`, and `api` commands do not. The chart has no migration Job/hook or
dependency-readiness init containers. Initialize a fresh DB before starting the
split services, and plan migrations and backups before upgrades.

For a release named `logrix` using the default service account, prepare an
operator-owned Job like this, substituting the verified image and names:

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: logrix-migrate-reviewed-release
  namespace: logrix
spec:
  backoffLimit: 1
  template:
    spec:
      restartPolicy: Never
      serviceAccountName: logrix-sa
      containers:
        - name: migrate
          image: registry.example.com/team/logrix:reviewed-release
          command: ["logrix", "migrate"]
          envFrom:
            - configMapRef:
                name: logrix-config
            - secretRef:
                name: logrix-secrets
```

Ensure the referenced ConfigMap, Secret, account, image pull credentials, network
access, and database are ready. For Helm demo backing services, first install with
app components disabled, then run the Job, wait for success, and enable the apps.
Use a new Job name for each migration run. For example, after saving the reviewed
manifest as `migration-job.yaml`:

```bash
kubectl apply -f migration-job.yaml
kubectl wait --namespace logrix --for=condition=complete \
  job/logrix-migrate-reviewed-release --timeout=120s
```

## Offline source validation

From the repository root, these commands render/check local source only:

```bash
helm lint deploy/helm/logrix
helm template logrix deploy/helm/logrix --namespace logrix > /dev/null
helm lint deploy/helm/logrix -f deploy/helm/values-aws.yaml
helm template logrix deploy/helm/logrix --namespace logrix \
  -f deploy/helm/values-aws.yaml > /dev/null
kustomize build deploy/k8s/base > /dev/null
kustomize build deploy/k8s/local > /dev/null
terraform -chdir=deploy/terraform/aws fmt -check -recursive
terraform -chdir=deploy/terraform/aws validate -no-color
```

Without standalone Kustomize, use `kubectl kustomize deploy/k8s/base` and
`kubectl kustomize deploy/k8s/local` (no cluster connection). Terraform validation
requires already initialized module/provider metadata and locally available
plugins. If absent, report the prerequisite failure; do not initialize or fetch
providers as part of an offline check. `fmt -check` does not fix source files.
Neither command requires reading tfvars or Terraform state.

CI uses Helm `3.14.0` and Terraform `1.8.0`, renders the default chart and both
Kustomize directories, and runs `terraform init -backend=false` before validate.
That init can download dependencies and is not an offline step. Rendering and
static validation do not verify CRDs, IAM, DB migrations, registry access,
regional engine availability, or a successful deployment.

The commands in these guides are operator instructions. Documentation validation
does not execute apply/install/upgrade, Terraform init/plan/apply/destroy, or any
cloud operation.
