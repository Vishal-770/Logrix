# Production Deployment & Security Remediation Plan

This plan details the prioritized fixes for the audited deployment defects across Helm, K8s, and AWS configuration.

---

## Phase 1: Security & Secret Hardening (Immediate Priority)

1. **ConfigMap Credential Leakage Fix**:
   - In `templates/configmap.yaml`, suppress `DATABASE_URL` and `RABBITMQ_URL` when `config.existingSecret` is provided or when external database/queue configurations are present.
   - For built-in services, generate a dedicated internal `Secret` (`templates/secret-internal.yaml`) rather than hardcoding passwords in ConfigMaps or Pod specs.

2. **WEBHOOK_SECRET Representation**:
   - Add `config.webhookSecret` in `values.yaml`.
   - Wire `WEBHOOK_SECRET` in `templates/configmap.yaml` and deployment templates via `secretKeyRef` or `existingSecret`.
   - Prevent random secret generation on worker restart.

3. **Pod Security Contexts**:
   - Add `securityContext` to all workload templates (`deployment-api.yaml`, `deployment-ingester.yaml`, `deployment-processor.yaml`, `deployment-webhook.yaml`):
     ```yaml
     securityContext:
       runAsNonRoot: true
       runAsUser: 10001
       runAsGroup: 10001
       readOnlyRootFilesystem: true
       allowPrivilegeEscalation: false
       capabilities:
         drop: ["ALL"]
     ```

---

## Phase 2: Identity, RBAC & Autoscaling Alignment

4. **IRSA Naming & Trust Policy Alignment**:
   - Align ServiceAccount names between `values-aws.yaml` and Terraform IAM module trust policy:
     - Helm: `logrix-ingester`, `logrix-processor`, `logrix-api` (matching Terraform).
     - Fix default role ARN annotation in `values-aws.yaml` to `logrix-production-processor-irsa`.

5. **Lease RBAC Binding Fix**:
   - In `templates/rbac.yaml`, bind `lease-role` to `logrix.ingesterServiceAccountName` so the ingester pod can manage the Kubernetes Lease.

6. **KEDA Autoscaling Hardening**:
   - In `templates/scaledobject-processor.yaml`, add fallback for queue URL:
     `default .Values.config.sqsLiveUrl .Values.processor.keda.sqs.queueUrl`
   - Fix default RabbitMQ host URL to point to `<release>-logrix-rabbitmq:5672/%2f`.
   - Add `TriggerAuthentication` resource template for KEDA SQS IRSA integration.

---

## Phase 3: Storage, Networking & Configuration Gaps

7. **Built-in Service Persistence**:
   - Convert `templates/local-postgres.yaml`, `templates/local-rabbitmq.yaml`, and `templates/local-minio.yaml` to `StatefulSet` with `volumeClaimTemplates` honoring `builtIn.postgresStorage`, etc.

8. **Missing Ingress Resource**:
   - Create `templates/ingress.yaml` supporting `api.ingress.enabled`, ingressClassName, annotations, hosts, paths, and TLS.

9. **Wire Unwired Variables**:
   - Wire `global.imagePullSecrets` across all workload pod templates.
   - Wire `S3_PREFIX`, `S3_BUCKET`, `AWS_REGION`, and worker tuning variables.

10. **deploy/k8s Clean-up**:
    - Update `deploy/k8s/base/logrix.yaml` to use version-pinned image tag `0.7.3` instead of mutable `:latest`.
