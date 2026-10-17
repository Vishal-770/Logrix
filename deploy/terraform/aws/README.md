# AWS reference infrastructure

This Terraform root provisions AWS resources. It does **not** deploy Logrix,
install Kubernetes operators, run migrations, or synchronize secrets into pods.
It is reference infrastructure with known incomplete application compatibility;
it is not a verified production deployment recipe. See the
[deployment overview](../../README.md), [Helm guide](../../helm/logrix/README.md),
and [AWS operations guide](../../../docs/AWS_EKS_OPERATIONS_GUIDE.md).

The source defaults to EKS `1.30` and fixes Aurora PostgreSQL to `16.8`. Current
AWS regional support, EKS support lifecycle, provider compatibility, AMI/instance
availability, and Aurora Serverless capacity ranges have not been verified live.
Do not treat these historical settings as current AWS recommendations.

## Prerequisites and costs

- Terraform `>= 1.5.0` (CI uses `1.8.0`). Root provider constraints are AWS
  `>= 5.0.0` and TLS `>= 4.0.0`. `random_password` also introduces the implicit
  `hashicorp/random` dependency; it is not explicitly constrained in versions.tf.
- A reviewed, reproducible provider selection. `.terraform.lock.hcl` is currently
  ignored by the repository; a locally generated lockfile does not establish a
  shared provider pin. Retain the reviewed lockfile through your IaC process.
- For actual provisioning, a short-lived AWS identity with the required
  VPC/EC2/EKS/IAM/RDS/SQS/S3/Secrets Manager/logging permissions and account quotas.
  A region with at least three available AZs is assumed by the VPC source.
- Protected Terraform state/backend and plan storage. No backend is declared,
  so Terraform defaults to local state unless the operator supplies backend
  configuration. State and saved plans include generated DB credentials and
  Secrets Manager secret contents. Encrypt/control access and locking through
  your backend process; never commit them or send them as diagnostics.
- For application deployment after provisioning: AWS CLI, compatible kubectl,
  Helm, registry access, Kubernetes administrator access, and independently
  installed operators as described below.

**This infrastructure incurs ongoing AWS charges.** Defaults include an EKS
control plane, two worker nodes, one NAT gateway/EIP, Aurora Serverless v2 with
minimum 0.5 ACU, S3 storage/versioning, SQS requests, Secrets Manager, and EKS
control-plane logs. Data transfer, NAT processing/cross-AZ traffic, snapshots,
and load balancers added later can add charges. Scaling pods or nodes to zero
does not stop control-plane, NAT, database, storage, or other recurring charges.
No cost estimate or provisioning-time guarantee is supplied.

## Actual module composition

All module sources are local directories under `modules/`.

| Module | Resources and fixed behavior |
| --- | --- |
| `vpc` | Three public and three private subnets in the first three available AZs; DNS enabled; one Internet gateway; **one** NAT gateway/EIP in public subnet 0 shared by private subnets; S3 gateway endpoint on the private route table |
| `eks` | Cluster `${project_name}-${environment}-eks`; private and public API endpoints enabled; all five control-plane log types; OIDC provider using TLS certificate lookup; managed node group `<cluster>-workers`, AMI type `AL2023_x86_64_STANDARD`; worker/CNI/ECR-read policies |
| `rds` | Private DB subnet group, PostgreSQL SG ingress on 5432 from the EKS cluster SG; Aurora PostgreSQL `16.8`, parameter family `aurora-postgresql16`, one `db.serverless` writer; encrypted storage; `rds.force_ssl=0`; work_mem 65536, maintenance_work_mem 524288 |
| `sqs` | Live, backfill, webhook and one shared DLQ, Standard by default; managed SSE; redrive to DLQ |
| `s3` | Archive bucket, generated prefix/suffix name when not supplied; versioning, AES256 default encryption, public-access block, lifecycle transitions |
| `iam` | Ingester, processor and API web-identity roles with queue/object/secret policies; no service accounts are created |

Subnet CIDRs use `cidrsubnet(vpc_cidr, 4, index)` for public indices 0-2 and
private indices 4-6. Private traffic to services without an endpoint uses the
single NAT path; this is not one NAT per AZ or an isolated private-only design.
Public EKS API access has no source CIDR restriction in this module.
`modules/eks` declares `vpc_id` but does not use it in its resources.

There are no Kubernetes/Helm providers, namespace resources, cluster access
entries, explicit EKS add-on resources, CSI driver installation, metrics-server,
KEDA, Prometheus Operator, ingress controller, External Secrets, Cluster
Autoscaler, or Karpenter here. Node-group autoscaler tags do not install a node
autoscaler. Establish EKS user/role access and verify system add-ons separately.

## Root input variables

These are the actual defaults in `variables.tf`. Examples/comments elsewhere do
not override them. The source provides types but no custom validation blocks.

| Variable | Type | Default | Effect |
| --- | --- | --- | --- |
| `aws_region` | string | `us-east-1` | AWS provider and S3 VPC endpoint region |
| `environment` | string | `production` | Names/tags |
| `project_name` | string | `logrix` | Names/tags; cluster name `<project>-<environment>-eks` |
| `vpc_cidr` | string | `10.0.0.0/16` | VPC and computed subnet CIDRs |
| `eks_cluster_version` | string | `1.30` | EKS control-plane version; verify support before provisioning |
| `eks_node_instance_types` | list(string) | `["t3.medium"]` | Node-group instance types; match fixed x86_64 AMI |
| `eks_node_desired_size` | number | 2 | Initial desired workers |
| `eks_node_min_size` | number | 1 | Node-group lower bound |
| `eks_node_max_size` | number | 20 | Node-group upper bound; does not install an autoscaler |
| `rds_min_acu` | number | 0.5 | Aurora minimum capacity; no auto-pause configured |
| `rds_max_acu` | number | 16.0 | Aurora maximum capacity |
| `rds_database_name` | string | `logrix` | Initial database |
| `rds_master_username` | string | `logrix_admin` | Initial master user; password generated in Terraform |
| `sqs_enable_fifo` | bool | false | Adds `.fifo` to all four queues; runtime FIFO compatibility is incomplete |
| `s3_bucket_name` | string | null | Explicit globally unique bucket name, or AWS-generated suffix on `<project>-<environment>-blocks-` |
| `k8s_namespace` | string | `logrix` | IAM trust subjects only; does not create the namespace |

Validate desired/min/max relationships, capacity, names, and region support
before provisioning. EKS/Aurora engine versions, node AMI type, retention/security
settings, and the operator installation are not all exposed as root inputs.

### Child module interfaces and defaults

The root wires required identity/network inputs to each child; these are not
independent root variables. Editing a child default has no effect when the root
passes a value explicitly.

| Module | Required inputs from root | Child defaults / additional controls |
| --- | --- | --- |
| `vpc` | `project_name`, `environment`, `cluster_name` | `cidr_block="10.0.0.0/16"`, `aws_region="us-east-1"` (both passed by root) |
| `eks` | `cluster_name`, `vpc_id` (unused), `subnet_ids`, `environment` | `cluster_version="1.30"`, `instance_types=["t3.medium"]`, `desired_size=2`, `min_size=1`, `max_size=20` (all passed by root) |
| `rds` | `project_name`, `environment`, `vpc_id`, `subnet_ids`, `client_security_group_id` | `min_capacity=0.5`, `max_capacity=16.0`, `database_name="logrix"`, `master_username="logrix_admin"` (all passed by root) |
| `sqs` | `project_name`, `environment` | `enable_fifo=false` (passed by root); `message_retention_seconds=345600`, `dlq_retention_seconds=1209600`, `visibility_timeout_seconds=300`, `max_receive_count=5` (child-only defaults) |
| `s3` | `project_name`, `environment` | `bucket_name=null` (passed by root); `enable_versioning=true`, `standard_ia_transition_days=30`, `glacier_ir_transition_days=90` (child-only defaults); noncurrent versions expire after fixed 30 days |
| `iam` | `project_name`, `environment`, `oidc_provider_arn`, `oidc_provider_url`, `s3_bucket_arn`, `sqs_queue_arns`, `db_secret_arn` | `k8s_namespace="logrix"` (passed by root) |

## Offline checks versus provisioning

From the repository root, safe local source checks are:

```bash
terraform -chdir=deploy/terraform/aws fmt -check -recursive
terraform -chdir=deploy/terraform/aws validate -no-color
```

`fmt -check` reports formatting without editing. `validate` requires already
initialized modules and locally installed provider plugins; it does not prove
AWS API or runtime compatibility. Missing module/provider metadata is a
prerequisite blocker for an offline check. Do not use `plan` to replace validate:
planning can read state, credentials, and cloud data sources.

For an operator's separate, authorized provisioning workflow, first resolve
version/compatibility and security settings, choose a protected backend and
provider lock, then initialize, validate, review a plan, and apply that reviewed
plan. Commands below are lifecycle instructions, **not commands executed during
documentation validation**:

```bash
# Networked/operator workflow, after backend and non-secret inputs are prepared.
terraform -chdir=deploy/terraform/aws init
terraform -chdir=deploy/terraform/aws validate
terraform -chdir=deploy/terraform/aws plan -out=reviewed.tfplan
terraform -chdir=deploy/terraform/aws apply reviewed.tfplan
```

Do not put AWS keys, DB passwords, or provider-key-bearing RPC URLs in tfvars.
Use short-lived identity through the normal AWS credential chain. The repository
does not generally ignore `*.tfvars` or saved `*.tfplan`; protect operator files
outside version control. The generated password is written to Terraform state
even though a Secrets Manager resource is also created. Do not print state,
secret JSON, or a plan's sensitive content to chat/CI logs.

CI runs `fmt -check -recursive`, `init -backend=false`, and `validate`.
`-backend=false` avoids backend initialization but still allows dependency
downloads and local metadata writes; it is not the offline check above.

## Outputs and application handoff

These are **root outputs** available after authorized provisioning, not live
values observed for this documentation task:

| Root output | Consumer / handoff |
| --- | --- |
| `vpc_id` | Network operations and connectivity review |
| `eks_cluster_name` | AWS CLI kubeconfig configuration; default name `logrix-production-eks` |
| `eks_cluster_endpoint` | Kubernetes API endpoint, not the Logrix GraphQL URL |
| `rds_cluster_endpoint` | Aurora writer DNS name; not a full database URL |
| `rds_secret_arn` | Secrets Manager secret identifier; synchronize its `url` field to Secret key `DATABASE_URL` |
| `s3_bucket_name` | Helm `config.s3Bucket` or runtime `S3_BUCKET` |
| `sqs_live_queue_url` | Helm `config.sqsLiveUrl` AND separately `processor.keda.sqs.queueUrl` |
| `sqs_backfill_queue_url` | Helm `config.sqsBackfillUrl` |
| `irsa_ingester_role_arn` | Ingester IAM identity if account/policy wiring is provided |
| `irsa_processor_role_arn` | Processor IAM identity; AWS Helm sample uses a placeholder for this shared-account annotation |
| `irsa_api_role_arn` | API IAM identity if account/policy wiring is provided |

The SQS child exposes `webhook_queue_url` and `dlq_url`, but the root does **not**
export them. There is no root `sqs_webhook_queue_url` or `sqs_dlq_url` output.
Arrange an explicit reviewed handoff for those URLs from infrastructure ownership;
do not invent output commands or copy the sample account/queue URLs.

For reference, child output interfaces are:

| Module | Child outputs |
| --- | --- |
| `vpc` | `vpc_id`, `public_subnet_ids`, `private_subnet_ids`, `cidr_block` |
| `eks` | `cluster_name`, `cluster_endpoint`, `cluster_certificate_authority_data`, `cluster_security_group_id`, `oidc_provider_arn`, `oidc_provider_url` |
| `rds` | `cluster_endpoint`, `reader_endpoint`, `database_name`, `secret_arn`, `security_group_id` |
| `sqs` | `live_queue_url`, `live_queue_arn`, `backfill_queue_url`, `backfill_queue_arn`, `webhook_queue_url`, `webhook_queue_arn`, `dlq_url`, `dlq_arn` |
| `s3` | `bucket_id`, `bucket_arn`, `bucket_domain_name` |
| `iam` | `ingester_role_arn`, `processor_role_arn`, `api_role_arn` |

Child-only outputs do not become root CLI outputs automatically.

The RDS secret JSON contains `host`, `port`, `database`, `username`, `password`,
and `url`. Its `url` encodes password characters and adds `?sslmode=require`.
Prefer that URL over manually concatenating a raw password into a URI. The app
does not fetch this secret by ARN; IAM `GetSecretValue` permission alone performs
no synchronization. Configure a separate controller/process to create the
namespace Secret, including refresh/rotation and pod restarts. Do not display
the password/URL in CLI arguments, logs, or Helm values.

## Known integration gaps to resolve before deployment

### IAM is not per-component chart isolation

The Terraform trust policies permit these subjects, with audience
`sts.amazonaws.com`, under the configured EKS OIDC provider:

| Role name suffix | Trusted account names in `k8s_namespace` | Policy scope |
| --- | --- | --- |
| `ingester-irsa` | `logrix-ingester`, `logrix-sa` | Send/SendBatch/GetQueueAttributes on all four queues; S3 PutObject/AbortMultipartUpload |
| `processor-irsa` | `logrix-processor`, `logrix-sa` | Queue receive/delete/delete-batch/change-visibility/send/send-batch/get-attributes; S3 Get/Put/DeleteObject; GetSecretValue on DB secret |
| `api-irsa` | `logrix-api`, `logrix-sa` | S3 GetObject and GetSecretValue on DB secret |

Full role names are `<project>-<environment>-<suffix>`. There is no distinct
webhook role. S3 policies target bucket objects and do not grant `s3:ListBucket`.

The chart uses **one** service account for all apps (default `logrix-sa`). One
annotation selects one IRSA role; the chart cannot use all three roles by
component. The sample's processor annotation shares that role's permissions
with ingester/API/webhook too. Custom account names/namespaces must match trust
policies. Review a shared policy/account arrangement or provide external
per-component resource customization; Terraform outputs do not wire it for you.

KEDA's SQS trigger uses `identityOwner: operator`; none of these three trusts
includes a KEDA operator service account. Configure its own IAM and queue access
separately. Neither an app IRSA annotation nor Terraform node autoscaling tags
install/configure KEDA.

### Queue, configuration, database, and runtime compatibility

- `sqs_enable_fifo=true` can provision FIFO queues, but current SQS publish and
  batch entry code supplies no MessageGroupId/deduplication ID, and Terraform
  does not enable content-based deduplication. This is not a usable runtime FIFO
  switch. Keep Standard queues until the complete producer/consumer flow is
  validated with FIFO semantics.
- Copy all queue URLs to runtime config, set `queueDriver: sqs`, set the app AWS
  region, and separately configure the scaler queue URL/region. Queue names in
  Terraform (`<project>-<environment>-live`, `-backfill`, `-webhook`, `-dlq`) differ
  from sample `values-aws.yaml` URLs. No placeholder substitution is performed.
- The AWS profile enables ServiceMonitor and KEDA/HPA, but Terraform installs
  none of their operators/metrics services. Its webhook scaler remains
  RabbitMQ-only if enabled. Independently install required operators or disable
  those features in the application's values.
- The AWS profile still requests two ingesters with incomplete lease election.
  Use one ingester and no-overlap updates pending a safe lease implementation.
  It also embeds a manifest with a WASM path but empty `wasmBase64`; provide the
  actual file or a compatible manifest/image. Manifest `start_block` is not an
  ingester start option; use runtime `START_BLOCK`/checkpoints.
- Initialize migrations before starting split services. Confirm private pod-to-DB
  connectivity: RDS allows the cluster security group, not every arbitrary pod
  or node security group. Verify the actual node/pod SG attachment/network path.
- The RDS parameter group disables forced TLS even though the stored URL requests
  TLS. It has deletion protection off, final snapshots skipped, and the Secrets
  Manager secret has zero-day recovery. These are source behaviors, not a backup
  or production data-retention policy.
- The root exposes no Aurora engine-version input, public EKS endpoint CIDR
  control, or per-role service account selector. Source rendering/validation
  cannot establish current AWS compatibility for the fixed choices.

## Application deployment and operations are separate

After provisioning, infrastructure ownership hands off cluster access, endpoints,
URLs, secret synchronization, and IAM decisions. Platform ownership installs
KEDA/metrics-server/Prometheus as needed. Application ownership selects a
verified versioned image, renders the local chart with complete values, runs
migrations, and then deploys the app. See the
[operations guide](../../../docs/AWS_EKS_OPERATIONS_GUIDE.md) for this sequence.
Terraform here neither pulls application source onto nodes nor calls Helm.

## Retention and teardown

Stop ingestion in a controlled manner and preserve needed checkpoints, database
backups, queue work, and object versions before removal. Uninstalling Helm does
not destroy this infrastructure. Conversely, destroying EKS does not constitute
a complete application-data backup.

The S3 bucket has `force_destroy=false` and versioning enabled. Deleting current
objects with `aws s3 rm --recursive` alone leaves old versions/delete markers;
review and remove all versions/markers and incomplete multipart uploads when
intentionally retiring it. Lifecycle transitions are not complete expiration
of current objects. Aurora skips its final snapshot and has no deletion
protection, while the DB secret has no recovery window; plan backups explicitly.

Use your normal reviewed Terraform destroy plan and approval process only when
authorized. Check for out-of-band load balancers, volumes, snapshots, logs, and
other retained resources afterward. No init, plan, apply, destroy, state reads,
or cloud actions are part of this documentation task.
