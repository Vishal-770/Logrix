# AWS EKS operations guide

This guide describes the handoff from the repository's reference AWS Terraform
to a Logrix Helm deployment and its day-two operations. It is not a claim of a
verified production deployment. Source semantics take precedence over comments
or the label "Production AWS EKS Profile" in the sample values.

Related references:

- [Deployment overview](../deploy/README.md)
- [Helm values, conditions, and limitations](../deploy/helm/logrix/README.md)
- [Terraform variables, modules, and outputs](../deploy/terraform/aws/README.md)
- [Canonical runtime configuration](configuration.md)
- [Kustomize all-in-one deployment](../deploy/k8s/README.md)

## 1. Current deployment boundaries

There are three independent phases:

1. **Infrastructure provisioning:** Terraform defines VPC/subnets/NAT, EKS,
   Aurora PostgreSQL, four SQS queues, S3, and IAM roles.
2. **Platform/operator preparation:** establish cluster access, system add-ons,
   metrics-server, KEDA and its AWS identity, and optional Prometheus/secret/ingress
   controllers. The Terraform and application chart do not install them.
3. **Application deployment:** prepare secrets and files, run DB migrations, and
   use Helm to deploy verified pre-built, versioned images. SDK tooling may
   prepare schemas/handlers; it does not deploy the application.

The default EKS version `1.30`, node AMI type `AL2023_x86_64_STANDARD`, and fixed
Aurora PostgreSQL `16.8` must be checked against the target region and current AWS
support lifecycle before provisioning. No current supported-version claim or
live cloud verification is made here. Review Terraform's broad provider
constraints and ignored lockfile before relying on reproducible provisioning.

**Important runtime limitations:**

- Both chart defaults and the AWS profile request two ingesters with election
  enabled. The current elector only reads a Lease, never creates/renews/expires
  it, and can assume leadership on missing/error/unparseable responses. It also
  skips API certificate validation. This is not safe production lease election.
  Use one ingester, disable election, and prevent old/new pod overlap on updates.
- All Helm components use one service account and IRSA annotation. Terraform's
  three IAM roles are not selected per Deployment. The sample processor role
  grants its privileges to every app using the shared account. There is no
  separately provisioned webhook role or KEDA operator trust.
- Split commands do not run migrations. No Helm migration hook or dependency
  readiness orchestration is provided. The all-in-one command's automatic
  migrations do not apply to the split Helm deployment.

## 2. Operator prerequisites and secret handling

For authorized provisioning use Terraform `>= 1.5.0` (CI `1.8.0`) and reviewed
provider versions. For cluster/application operations use AWS CLI v2, kubectl
compatible with the selected cluster, and Helm (CI `3.14.0`). Choose a region with
three available AZs and sufficient EKS/EC2/network/database quotas. Use
short-lived AWS identity through your organization's credential process, not
long-lived keys embedded in files or deployment values.

State/backend and saved plans require encryption, restricted access, and locking.
The source has no backend block, so local state is the default. Generated DB
passwords and secret JSON exist in Terraform state. Never commit or attach state,
real tfvars, local credentials, secret dumps, or plans containing sensitive data.
`*.tfvars`/`*.tfplan` are not generally ignored by this repository.

The chart expects a namespace Secret, selected by `config.existingSecret`, with
exact uppercase runtime environment keys such as `DATABASE_URL`, not
`database-url`. ConfigMap envFrom is imported first, Secret envFrom second, so
the Secret wins duplicate keys. Explicit pod env and CLI options take precedence
over envFrom. This Secret is shared by all app components.

The Terraform RDS secret has fields `host`, `port`, `database`, `username`,
`password`, and `url`. Use a reviewed secret-sync process to map its `url` field
to `DATABASE_URL`; that URL encodes password characters and requests TLS with
`sslmode=require`. A secret ARN or IAM GetSecretValue permission does not populate
pod env. A Kubernetes Secret controller, refresh process, and restarts are
separate responsibilities. Do not interpolate raw passwords into shell commands
or put credential-bearing URLs in Helm values/ConfigMaps/`--set`.

For an isolated manual bootstrap, use a protected env file as described in the
[deployment overview](../deploy/README.md#secrets-and-configuration-precedence).
Supply `AWS_REGION` when not provided by the platform, and `WEBHOOK_SECRET` if
webhooks require a stable signing secret. Prefer IRSA to static AWS credentials.
The chart has no direct `S3_PREFIX` value forwarding; use an exact `S3_PREFIX`
Secret entry or an externally managed env patch when needed.

## 3. Provision reference infrastructure and hand off outputs

Use the [Terraform README](../deploy/terraform/aws/README.md) for all actual
inputs/defaults, module settings, offline checks, and the separate authorized
init/plan/apply workflow. No infrastructure apply is part of documentation
validation. Do not substitute the sample values for verified operator inputs.

Defaults are `us-east-1`, project `logrix`, environment `production`, cluster
`logrix-production-eks`, two desired `t3.medium` workers (min 1/max **20**), and
Aurora 0.5-16 ACU. The VPC has three public/private subnet pairs but only **one**
NAT gateway, not a NAT gateway per AZ. Both private and public cluster API access
are enabled. A max node count and autoscaler tags do not install Cluster
Autoscaler or Karpenter.

Required application handoff:

| Terraform root output | Use |
| --- | --- |
| `eks_cluster_name`, `eks_cluster_endpoint` | Cluster connection; endpoint is not Logrix API |
| `rds_cluster_endpoint`, `rds_secret_arn` | DB connectivity and secret synchronization |
| `s3_bucket_name` | `config.s3Bucket` if archiving is wanted |
| `sqs_live_queue_url` | Both `config.sqsLiveUrl` and `processor.keda.sqs.queueUrl` |
| `sqs_backfill_queue_url` | `config.sqsBackfillUrl` |
| `irsa_ingester_role_arn`, `irsa_processor_role_arn`, `irsa_api_role_arn` | Inputs to the reviewed identity/account arrangement, not automatic chart wiring |
| `vpc_id` | Network operations |

The webhook/DLQ URLs exist as SQS child outputs but are not root outputs. Obtain
an explicit infrastructure handoff for them; do not use nonexistent root
`sqs_webhook_queue_url`/`sqs_dlq_url` commands. The real Terraform queue names
are `<project>-<environment>-live`, `-backfill`, `-webhook`, and `-dlq`, unlike the
placeholders in the AWS profile. Keep `sqs_enable_fifo=false`: current publishers
do not send FIFO group/deduplication IDs and Terraform does not enable
content-based deduplication.

After an operator has provisioned and authorized access, connect using its actual
cluster name and region (the commands below are operator instructions):

```bash
export AWS_REGION=your-reviewed-region
export EKS_CLUSTER_NAME=your-provisioned-cluster
aws eks update-kubeconfig --region "$AWS_REGION" --name "$EKS_CLUSTER_NAME"
kubectl get nodes -o wide
kubectl get pods --namespace kube-system
```

Do not assume a successful infrastructure apply establishes user access or
working add-ons. The modules have no explicit access-entry or EKS add-on
resources. Confirm pod-to-RDS connectivity on 5432; the RDS ingress rule permits
the EKS cluster SG, so actual node/pod SG attachments and networking matter.

## 4. Prepare operators and application values

The checked-in AWS profile enables KEDA processor scaling, API HPA, and
ServiceMonitor. Independently install compatible versions and verify:

- **KEDA CRDs, operator and metrics adapter** if processor KEDA or enabled webhook
  autoscaling is used. Configure KEDA's own AWS role/queue access: SQS scaler
  `identityOwner: operator` does not use the app's IRSA annotation.
- **Resource metrics API**, usually metrics-server, for API CPU/memory HPA.
- **Prometheus Operator ServiceMonitor CRD** and a Prometheus instance whose
  ServiceMonitor label selectors match `serviceMonitor.labels`, if enabled.

Otherwise disable the corresponding values. Operators and app autoscalers do not
provide node autoscaling. The chart creates no Ingress despite declared
`api.ingress` values; supply ingress controller, routing, TLS, and access policy
independently if exposing the ClusterIP API.

`values-aws.yaml` is sample configuration, not provisioning. Prepare a complete
operator-owned values file from it and replace all placeholders. In particular:

- Pin `image.repository`/`image.tag` to a verified versioned image. The source
  profile says `0.7.3`; default Helm uses `latest`. Neither proves a published
  artifact exists. Validate/use the local chart, not an assumed OCI release.
- Keep `builtIn.enabled=false`, individual built-ins false, and
  `localDev.enabled=false`. A URL supplied only in a Secret does not affect Helm's
  built-in rendering conditions.
- Use `config.queueDriver: sqs`, all required real queue URLs, app SDK region,
  and independently the processor scaler queue URL/region. Blank endpoint values
  mean normal AWS endpoints; custom endpoints are for compatible test services.
- Match namespace, service account name and role trust. Trust policies allow
  component-specific accounts and `logrix-sa`; a single annotated shared account
  cannot assume three roles by Deployment. Choose/review the permission model
  before deploying.
- Set `ingester.replicas: 1` and `ingester.leaderElection.enabled: false`, with a
  no-overlap update procedure. PDBs are not an exclusivity guarantee.
- Supply actual manifest/schema/WASM files if using custom mapping. The sample
  manifest references `/etc/logrix/handlers/mapping.wasm`, but its WASM value is
  empty. Use `--set-file config.wasmBinary=...` or `config.wasmBase64`; a path alone
  does not supply a file. Use `config.startBlock`/`START_BLOCK` for ingestion start;
  the manifest contract `start_block` does not control the ingester.
- Patch the ingester to mount the inline manifest too, or provide an equivalent
  readable path. The current chart sets `MANIFEST_PATH` there but mounts the
  file only in other components; ingestion fails at startup without this fix.
  There is no ingester extra-volume/mount value, so this is a consumer-managed
  patch rather than a supported Helm toggle.
- If enabling webhook Deployment, disable its RabbitMQ-only autoscaler for SQS
  and manage a suitable scaler separately. `webhook.enabled=true` also enables
  embedded dispatch loops in processors.

Review the [Helm table](../deploy/helm/logrix/README.md) for fixed/unused values and
truthiness: numeric zero may be replaced by a template default or omitted;
quoted `"0"` behaves differently and must still be valid for the destination.

## 5. Validate, migrate, then deploy the app

From the repository root, run local source checks before an operator deployment:

```bash
helm lint deploy/helm/logrix
helm template logrix deploy/helm/logrix --namespace logrix > /dev/null
helm lint deploy/helm/logrix -f deploy/helm/values-aws.yaml
helm template logrix deploy/helm/logrix --namespace logrix \
  -f deploy/helm/values-aws.yaml > /dev/null
helm lint deploy/helm/logrix -f operator-aws-values.yaml
helm template logrix deploy/helm/logrix --namespace logrix \
  -f operator-aws-values.yaml > /dev/null
```

The operator file should contain complete non-secret values, including image
selection. Do not rely on Helm's saved/reused values to retain an image pin.
Rendered output can contain inline WASM and credential-bearing configuration;
do not publish it to logs. Lint/template do not verify cluster CRDs, IAM,
registry artifacts, or runtime health.

For a **fresh** installation, create the chosen namespace and arrange its Secret
and service account/IRSA. For example, using namespace/release `logrix`:

```bash
kubectl create namespace logrix
# Synchronize logrix-secrets using the reviewed external secret process first.
helm upgrade --install logrix ./deploy/helm/logrix --namespace logrix \
  -f operator-aws-values.yaml \
  --set ingester.enabled=false --set processor.enabled=false --set api.enabled=false \
  --set webhook.enabled=false --set serviceMonitor.enabled=false \
  --set podDisruptionBudget.enabled=false
```

This first phase establishes ConfigMap/account wiring without starting apps;
it does not provision AWS resources. Run the
[migration Job](../deploy/README.md#database-initialization-for-split-services)
with the exact same image, ConfigMap, Secret, service account, and required image
pull configuration. Wait for migration completion before enabling split apps.
For an existing deployment, coordinate schema changes and backups instead of
blindly repeating the fresh-install sequence.

Then deploy using complete desired values (with the ingester safeguards above):

```bash
helm upgrade --install logrix ./deploy/helm/logrix --namespace logrix \
  -f operator-aws-values.yaml
kubectl rollout status deployment/logrix-api --namespace logrix
kubectl rollout status deployment/logrix-processor --namespace logrix
kubectl rollout status deployment/logrix-ingester --namespace logrix
kubectl get pods,hpa --namespace logrix
```

Names assume no fullname override. Only expect `logrix-webhook` if enabled; it is
disabled in both the default chart and the current AWS profile. A Running pod
does not prove ingestion correctness, active leadership, or successful mapping.

Helm's API command is `logrix api --port <api.port>`: `--port` belongs after the
`api` subcommand. Shared root options go before the subcommand, for example
`logrix --database-max-connections 12 api --port 4000`. `api.service.port` changes
the Service port only; it does not change the listener. API probes use
`/healthz` and `/readyz`.

## 6. Query, observe, and diagnose

For a default-port release, use a local port-forward:

```bash
kubectl port-forward --namespace logrix svc/logrix-api 4000:4000
```

In another terminal:

```bash
curl --fail http://localhost:4000/healthz
curl --fail http://localhost:4000/readyz
curl --fail http://localhost:4000/metrics
curl --fail http://localhost:4000/graphql \
  -H 'Content-Type: application/json' -d '{"query":"{ __typename }"}'
```

GraphiQL is at `/` and `/graphiql`. Query entity fields from the deployed schema;
the sample Transfer schema is not universal. `/healthz` reports HTTP process
liveness; `/readyz` checks a DB `SELECT 1` when the server has its store attached,
not queue/RPC/indexing progress. `/metrics` is exposed by the API, not every
standalone worker. Current API metric families include `logrix_api_health`,
`logrix_api_version`, `logrix_db_pool_connections`, `logrix_last_indexed_block`,
`logrix_events_indexed_total`, `logrix_reorg_events_total`, and
`logrix_entities_total`. Some DB metric queries fall back to zero on failure;
zero is not proof of an empty or healthy pipeline.

Example Kubernetes diagnostics (review/redact output before sharing):

```bash
kubectl get pods --namespace logrix
kubectl logs --namespace logrix deployment/logrix-ingester --tail=100
kubectl logs --namespace logrix deployment/logrix-processor --tail=100
kubectl logs --namespace logrix deployment/logrix-api --tail=100
kubectl get hpa --namespace logrix
kubectl get scaledobjects --namespace logrix
kubectl get events --namespace logrix
```

The ScaledObject command requires installed KEDA CRDs. Inspect queue metrics for
both live and backfill queues; the processor scaler watches only one queue.
For an operator with queue read permission and an actual URL:

```bash
aws sqs get-queue-attributes --queue-url "$SQS_LIVE_URL" \
  --attribute-names ApproximateNumberOfMessages ApproximateNumberOfMessagesNotVisible
```

Use a protected authorized DB connection for deeper checks. Actual storage uses
tables such as `logrix_checkpoints`, `logrix_events`, and `logrix_entities`; do not
assume `_logrix_checkpoints` or an `entities_transfer` table exists. Avoid exposing
the private database publicly or placing passwords in SQL command arguments.

| Symptom | Source-grounded checks |
| --- | --- |
| `no matches for kind ScaledObject` | Install KEDA CRDs/operator first or disable the relevant KEDA values |
| `no matches for kind ServiceMonitor` | Install Prometheus Operator CRD or disable ServiceMonitor (enabled by AWS profile) |
| API HPA reports unknown metrics | Confirm resource metrics API, resource requests, and metrics-server operation |
| SQS scaler AccessDenied but app works | Scaler uses KEDA operator identity; app IRSA alone does not authorize it |
| SQS app AccessDenied | Check shared account annotation, namespace/name/audience trust, region, and actual queue policy/URLs |
| Split workers fail with missing tables | Run reviewed migrations; these commands do not automatically migrate |
| DB connection errors | Check Secret key `DATABASE_URL`, encoded URL/TLS, private networking and SGs |
| Processor cannot load mapping | Supply WASM bytes and manifest path matching the processor mount; AWS sample does not supply the binary |
| Multiple ingesters publish the same work | Current election is incomplete; reduce to one and prevent update overlap |
| Changed env seems ignored | Existing pod env is not refreshed; inspect key spelling and precedence, then coordinate restart |
| API ingress never appears | `api.ingress.*` is unused; manage an Ingress independently |

## 7. Upgrades and scaling

Back up state, record the verified old/new image identity and values, review DB
migration compatibility, and render the new chart before upgrading. Chart changes
do not guarantee binary/config/schema compatibility. A Helm rollback does not
undo database migrations or restore queue/object state.

The chart has no ConfigMap/Secret checksum rollout annotations. Env changes need
pod replacement; file projection does not imply application hot reload. Coordinate
restarts after secret rotation. Restarting an ingester via the default rolling
strategy can overlap instances; do not advertise it as a safe zero-downtime step.
For single-ingester updates, stop it and wait for old pods to exit before applying
the new image/configuration and restoring one replica. For release `logrix`:

```bash
kubectl scale deployment/logrix-ingester --namespace logrix --replicas=0
kubectl wait --namespace logrix --for=delete pod \
  -l 'app.kubernetes.io/instance=logrix,app.kubernetes.io/component=ingester' \
  --timeout=120s
# Now run the reviewed upgrade restoring one ingester with election disabled.
```

Processor grace period defaults to 60s with a fixed five-second preStop sleep;
other worker lifecycle settings are not exposed equivalently. Plan queue
visibility timeout and shutdown/processing time together. Do not promise that
PDBs or rolling restarts make every operation zero-downtime.

Use Helm values to control lasting replica/limit changes. API HPA and processor
KEDA can overwrite manual `kubectl scale`; Helm upgrades can reset manually
changed replicas. Defaults impose minimums, and processor numeric
`minReplicaCount: 0` becomes 1 via template `default`. At high replica counts,
budget database pools per process as well as CPU/memory and node capacity.

## 8. Pause and cost management

Scaling all Deployments to zero is not a durable pause when controllers restore
minimum replicas. Record desired configuration, disable relevant HPA/KEDA
controllers through reviewed app values, stop ingestion, drain or preserve
queued work, then scale workloads down deliberately. Resume one ingester only
after dependencies are ready and checkpoints are verified. There is no chart
deployment-strategy setting, so preserve no-overlap safeguards when resuming.

The managed node group's actual Terraform name is `<cluster-name>-workers`, not
`<cluster-name>-managed-nodes`. Reducing its min/desired capacity requires a
reviewed infrastructure change and may conflict with node autoscalers or later
Terraform applies. Pods/operators cannot run without capacity. Do not assume
manually changing AWS sizing updates Terraform's desired configuration.

Even with no app pods or worker nodes, charges can remain for EKS control plane
(including support tier), NAT gateway/EIP, Aurora capacity/storage, S3 versions,
Secrets Manager, SQS, logs, load balancers, and snapshots. The source Aurora
minimum is 0.5 ACU with no auto-pause configured. This procedure is not a
zero-cost shutdown and provides no dollar-per-hour guarantee.

## 9. Retirement and data retention

Plan backups, retention, pending queue work, and rollback requirements before
uninstalling. An operator can remove application resources with:

```bash
helm uninstall logrix --namespace logrix
```

This does not destroy Terraform infrastructure, independently created Secrets,
operators, ingress resources, or external data. Remove the namespace only after
reviewing everything it contains; namespace deletion is not selective cleanup.

For authorized infrastructure retirement, review a Terraform destroy plan and
apply it through the normal IaC approval process. Do not use an unreviewed
auto-approved destroy recipe. Source behavior is especially important:

- Aurora has `deletion_protection=false` and `skip_final_snapshot=true`. Take
  independently verified backups if data is required.
- The DB Secrets Manager resource has `recovery_window_in_days=0`.
- S3 is versioned with `force_destroy=false`. Removing current keys with
  `aws s3 rm --recursive` does not remove all versions/delete markers. Plan their
  removal and any multipart cleanup before intentional bucket destruction.
- Check for retained/out-of-band volumes, snapshots, logs, load balancers, and
  other billable resources. No command here guarantees every charge is removed.

Documentation verification runs local render/format/validate checks only. It
does not read Terraform state, real tfvars, or local credentials, initialize
providers, execute apply commands, or contact live cloud services.
