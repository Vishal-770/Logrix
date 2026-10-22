# Production Deployment, Security & Helm/K8s Architecture Audit Report

This audit documents findings across `deploy/helm/logrix`, `deploy/helm/values-aws.yaml`, `deploy/k8s`, Rust configuration loaders in `logrix-cli`, and supporting AWS Terraform modules.

---

## Part 1: Critical Findings

### 1. Plaintext database and RabbitMQ credentials in ConfigMap
- **Location**: `deploy/helm/logrix/templates/configmap.yaml`
- **Issue**: Workload templates use `envFrom` with ConfigMap before optional Secret. Default `DATABASE_URL` and `RABBITMQ_URL` fallbacks with `logrix:logrixpassword` are published in ConfigMap even when external secrets are configured.
- **Remediation**:
  - Do not emit `DATABASE_URL` or `RABBITMQ_URL` into ConfigMap when `existingSecret` is set.
  - Require external Secret for production AWS profiles.

### 2. Built-in backing services hard-code credentials in Pod specifications
- **Location**: `templates/local-postgres.yaml`, `templates/local-rabbitmq.yaml`, `templates/local-minio.yaml`
- **Issue**: Pod specifications contain hardcoded plaintext credentials (`POSTGRES_PASSWORD: logrixpassword`, etc.) in container env arrays.
- **Remediation**:
  - Use Secret-backed environment variables via `secretKeyRef` or generate a local-dev Secret.
  - Disable built-in services in production profiles.

### 3. WEBHOOK_SECRET is not represented by Helm values or ConfigMap
- **Location**: `crates/logrix-cli/src/args.rs` expects `WEBHOOK_SECRET`.
- **Issue**: The chart emits neither `WEBHOOK_SECRET` nor sets it from Secret. When absent, the processor generates a random secret on startup. If pods restart or scale out, multiple replicas have different secrets, invalidating signatures.
- **Remediation**:
  - Standardize on `WEBHOOK_SECRET` matching the Rust CLI loader.
  - Inject with `secretKeyRef` or via `existingSecret`.

### 4. Built-in persistence settings are completely unused
- **Location**: `values.yaml` declares `postgresStorage: 1Gi`, `rabbitmqStorage: 1Gi`, `minioStorage: 5Gi`.
- **Issue**: `local-postgres.yaml`, `local-rabbitmq.yaml`, and `local-minio.yaml` use Deployments with zero `volumeMounts`, resulting in ephemeral storage where restarts wipe out checkpoints, queues, and blocks.
- **Remediation**:
  - Convert built-in services to StatefulSets with `volumeClaimTemplates` or configure PVC mounts.

### 5. KEDA SQS scaling uses operator identity instead of workload IRSA
- **Location**: `templates/scaledobject-processor.yaml`, `templates/scaledobject-webhook.yaml`
- **Issue**: Both SQS triggers specify `identityOwner: operator` without `TriggerAuthentication` or IRSA references.
- **Remediation**:
  - Add `TriggerAuthentication` referencing the processor IRSA ServiceAccount or configure workload identity.

### 6. KEDA processor queue URL has no fallback to `config.sqsLiveUrl`
- **Location**: `templates/scaledobject-processor.yaml`
- **Issue**: If `processor.keda.sqs.queueUrl` is empty but `config.sqsLiveUrl` is set, `queueURL` renders empty.
- **Remediation**:
  - `queueURL: {{ default .Values.config.sqsLiveUrl .Values.processor.keda.sqs.queueUrl | quote }}`

### 7. AWS IRSA ServiceAccount names mismatch Terraform trust policies
- **Location**:
  - Helm: `logrix-ingester-sa`, `logrix-processor-sa`
  - Terraform trust policy: `logrix-ingester`, `logrix-processor` (without `-sa`)
- **Remediation**:
  - Align naming across Helm ServiceAccounts and Terraform trust policies.

### 8. Helm RBAC binds lease Role only to default ServiceAccount
- **Location**: `templates/rbac.yaml`
- **Issue**: RoleBinding uses `logrix.serviceAccountName` instead of `logrix.ingesterServiceAccountName`. Ingest running with `logrix-ingester` cannot access the Lease.
- **Remediation**:
  - Bind the lease Role to `logrix.ingesterServiceAccountName`.

---

## Part 2: Configuration & Operational Findings

### 9. Variables parsed by Rust missing from Helm
- `WEBHOOK_SECRET`
- `S3_PREFIX`
- `S3_BUCKET`
- `WEBHOOK_WORKER_IDLE_MS`
- `WEBHOOK_WORKER_BACKOFF_MS`
- `AWS_REGION`

### 10. Missing Ingress template
- `values.yaml` defines `api.ingress.enabled`, but `templates/ingress.yaml` does not exist.

### 11. Unused global.imagePullSecrets
- `global.imagePullSecrets` is defined but not injected into Pod specifications.

### 12. Pod Security Contexts missing
- Workload Pods do not declare `runAsNonRoot: true`, `readOnlyRootFilesystem: true`, or drop capabilities.

### 13. RabbitMQ KEDA Scaler URL
- Static URL in values uses wrong service name (`rabbitmq` instead of `<release>-logrix-rabbitmq`) and password.

### 14. deploy/k8s base and local manifests
- `deploy/k8s/base/logrix.yaml` references missing secret, uses `:latest` mutable tag.
- `deploy/k8s/local/rabbitmq.yaml` is ephemeral deployment.
