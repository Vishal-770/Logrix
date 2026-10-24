# AWS Security Architecture: IAM, IRSA & Secrets Management

This document provides a comprehensive technical reference for the zero-trust identity and credential architecture implemented in Logrix on AWS EKS, Amazon Aurora, Amazon SQS, Amazon S3, and AWS Secrets Manager.

---

## 1. Architectural Philosophy: Zero Static Credentials

Traditional cloud indexers rely on long-lived, static access keys (`AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY`) or hardcode database connection strings into Git repositories and Helm overlays. This pattern creates severe security vulnerabilities:
- Stale keys that never expire.
- Risk of credential scraping if Git repositories are opened or cloned.
- Broad, unsegmented permissions across different components.

Logrix adheres to strict **Zero Static Credentials** and **Least Privilege Security**:
1. **Zero AWS Keys**: No pod contains static AWS access credentials.
2. **Cryptographic Pod Identity**: All AWS authentication flows through OpenID Connect (OIDC) token projection (IRSA).
3. **Secrets Isolation**: Secrets are stored in hardware-encrypted AWS Secrets Manager vaults and delivered via decoupled Kubernetes secret projections.
4. **Zero-Log Hygiene**: All credentials, database connection strings, and RPC keys are sanitized and masked before reaching stdout, log aggregators, or tracing pipelines.

---

## 2. End-to-End System Topology

```
                                    AWS CLOUD ACCOUNT BOUNDARY
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                                  │
│   ┌───────────────────────────────────┐               ┌──────────────────────────────────────┐   │
│   │        AWS Secrets Manager        │               │       EKS OIDC Identity Provider     │   │
│   │                                   │               │                                      │   │
│   │  Secret: logrix-production-rds-*  │               │  Issuer URL:                         │   │
│   │  Encrypted at rest: AWS KMS       │               │  oidc.eks.us-east-1.amazonaws.com    │   │
│   │  (Aurora master username/pass)    │               └──────────────────┬───────────────────┘   │
│   └─────────────────┬─────────────────┘                                  │                       │
│                     │                                                    │ OIDC Trust Handshake  │
│                     │                                                    ▼                       │
│                     │                                 ┌──────────────────────────────────────┐   │
│                     │                                 │           IAM Roles (IRSA)           │   │
│                     │                                 │                                      │   │
│                     │                                 │ 1. logrix-production-ingester-irsa   │   │
│                     │                                 │ 2. logrix-production-processor-irsa  │   │
│                     │                                 │ 3. logrix-production-api-irsa        │   │
│                     │                                 │ 4. logrix-production-webhook-irsa    │   │
│                     │                                 └──────────────────┬───────────────────┘   │
│                     │                                                    │                       │
└─────────────────────┼────────────────────────────────────────────────────┼───────────────────────┘
                      │                                                    │
                      │ Controlled Secret Sync                             │ Projected WebIdentity
                      ▼                                                    ▼ Tokens (RS256 JWT)
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                       AMAZON EKS CLUSTER                                         │
│                                                                                                  │
│   Kubernetes Secret: logrix-secrets (Namespace: logrix)                                          │
│   ├── DATABASE_URL: postgres://logrix_admin:<masked>@aurora...:5432/logrix                       │
│   ├── RPC_URL: Primary Alchemy / Infura endpoint                                                 │
│   └── RPC_FALLBACK_URLS: Redundant multi-tier fallback pool                                      │
│                                                                                                  │
│   ┌──────────────────────────────────────────────────────────────────────────────────────────┐   │
│   │ 1. INGESTER WORKLOAD (Deployment: logrix-ingester)                                       │   │
│   │    - ServiceAccount: logrix-ingester                                                     │   │
│   │    - Bound IAM Role: logrix-production-ingester-irsa                                     │   │
│   │    - Scope: EVM RPC Header Ingestion -> SQS SendMessage (Live Queue)                     │   │
│   └──────────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                                  │
│   ┌──────────────────────────────────────────────────────────────────────────────────────────┐   │
│   │ 2. PROCESSOR WORKLOAD (Deployment: logrix-processor)                                      │   │
│   │    - ServiceAccount: logrix-processor                                                    │   │
│   │    - Bound IAM Role: logrix-production-processor-irsa                                    │   │
│   │    - Scope: SQS Receive/Delete -> WASM Handler -> Aurora Postgres Upsert -> S3 Upload     │   │
│   │    - KEDA Scaler: Uses projected token to inspect queue backlog and autoscale workers    │   │
│   └──────────────────────────────────────────────────────────────────────────────────────────┘   │
│                                                                                                  │
│   ┌──────────────────────────────────────────────────────────────────────────────────────────┐   │
│   │ 3. GRAPHQL API WORKLOAD (Deployment: logrix-api)                                         │   │
│   │    - ServiceAccount: logrix-api                                                          │   │
│   │    - Bound IAM Role: logrix-production-api-irsa                                          │   │
│   │    - Scope: Read-only GraphQL queries against Aurora PostgreSQL via VPC internal network │   │
│   └──────────────────────────────────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. IAM Roles for Service Accounts (IRSA) Deep-Dive

### Step 1: The OIDC Federation Handshake
When an Amazon EKS cluster is provisioned (`deploy/terraform/aws/modules/eks/main.tf`), AWS enables an IAM OpenID Connect (OIDC) identity provider associated with the cluster API server:

```text
arn:aws:iam::<ACCOUNT_ID>:oidc-provider/oidc.eks.<REGION>.amazonaws.com/id/<OIDC_ID>
```

### Step 2: Fine-Grained Trust Policies (`deploy/terraform/aws/modules/iam/main.tf`)
Each microservice is bound strictly to its designated Kubernetes ServiceAccount and namespace:

```hcl
resource "aws_iam_role" "processor" {
  name = "logrix-production-processor-irsa"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Action = "sts:AssumeRoleWithWebIdentity"
      Effect = "Allow"
      Principal = {
        Federated = var.oidc_provider_arn
      }
      Condition = {
        StringLike = {
          # Restricts role assumption strictly to this service account in this namespace
          "${var.oidc_provider_url}:sub" = [
            "system:serviceaccount:logrix:logrix-processor",
            "system:serviceaccount:logrix:logrix-sa",
            "system:serviceaccount:keda:keda-operator"
          ]
        }
        StringEquals = {
          # Ensures token was generated specifically for AWS STS
          "${var.oidc_provider_url}:aud" = "sts.amazonaws.com"
        }
      }
    }]
  })
}
```

### Step 3: Kubernetes Mutating Admission Webhook Execution
When a pod specifying `serviceAccountName: logrix-processor` is scheduled:
1. The EKS Pod Identity Webhook intercepts the Pod definition before writing to etcd.
2. It projects a signed RS256 JWT token directly into the container at:
   `/var/run/secrets/eks.amazonaws.com/serviceaccount/token`
3. It injects two standard environment variables into container PID 1:
   ```bash
   AWS_ROLE_ARN="arn:aws:iam::206388234927:role/logrix-production-processor-irsa"
   AWS_WEB_IDENTITY_TOKEN_FILE="/var/run/secrets/eks.amazonaws.com/serviceaccount/token"
   ```

### Step 4: Runtime Authentication Inside the Rust Binary
Logrix uses the official AWS SDK for Rust (`aws-config`, `aws-sdk-sqs`, `aws-sdk-s3`). At runtime, the client resolves credentials automatically:
1. `aws_config::load_defaults()` detects `$AWS_WEB_IDENTITY_TOKEN_FILE`.
2. Reads the short-lived JWT from disk.
3. Calls AWS STS:
   ```text
   POST https://sts.us-east-1.amazonaws.com
   Action=AssumeRoleWithWebIdentity
   &RoleArn=arn:aws:iam::...:role/logrix-production-processor-irsa
   &WebIdentityToken=<JWT>
   ```
4. AWS STS verifies the token signature against the EKS OIDC public keys and returns temporary credentials (valid for 1 hour).
5. The Rust binary caches the credentials in memory and refreshes them asynchronously before expiration without restarting the container.

---

## 4. Workload Segmentation & Least Privilege Matrix

| Workload Pod | Kubernetes ServiceAccount | Bound IAM Role | Allowed AWS Operations |
|---|---|---|---|
| **Ingester** | `logrix-ingester` | `logrix-production-ingester-irsa` | `sqs:SendMessage`, `sqs:SendMessageBatch`, `sqs:GetQueueAttributes` on SQS Live Queue. Cannot read database or touch S3. |
| **Processor** | `logrix-processor` | `logrix-production-processor-irsa` | `sqs:ReceiveMessage`, `sqs:DeleteMessage`, `sqs:SendMessage` on Live/Backfill queues.<br>`s3:GetObject`, `s3:PutObject` on `chain_1/blocks/*`.<br>`secretsmanager:GetSecretValue` on RDS secret. |
| **GraphQL API** | `logrix-api` | `logrix-production-api-irsa` | Zero external AWS write permissions. Read-only queries served directly from Aurora PostgreSQL via private VPC subnets. |
| **KEDA Operator** | `keda-operator` (in `keda` ns) | `logrix-production-processor-irsa` | `sqs:GetQueueAttributes` to calculate queue backlog depth and autoscale processor pods. |

---

## 5. AWS Secrets Manager Integration

### 1. Secret Generation in Terraform
When Aurora Serverless v2 PostgreSQL is created (`deploy/terraform/aws/modules/rds/main.tf`):
```hcl
resource "random_password" "db_password" {
  length  = 32
  special = false
}

resource "aws_secretsmanager_secret" "rds_credentials" {
  name                    = "${var.project_name}-${var.environment}-rds-creds-${random_id.suffix.hex}"
  kms_key_id              = "aws/secretsmanager"
  recovery_window_in_days = 0
}

resource "aws_secretsmanager_secret_version" "rds_credentials" {
  secret_id = aws_secretsmanager_secret.rds_credentials.id
  secret_string = jsonencode({
    username = var.db_admin_username
    password = random_password.db_password.result
    host     = aws_rds_cluster.postgresql.endpoint
    port     = 5432
    database = var.db_name
  })
}
```
**Key Advantage**: The database master password is never stored in plaintext, never touched by human operators, and never committed to Git.

### 2. Delivery to Kubernetes
The secret is mounted into Kubernetes via the decoupled secret object `logrix-secrets`:
```yaml
apiVersion: v1
kind: Secret
metadata:
  name: logrix-secrets
  namespace: logrix
type: Opaque
data:
  DATABASE_URL: <base64-encoded postgres connection string>
  RPC_URL: <base64-encoded primary provider URL>
  RPC_FALLBACK_URLS: <base64-encoded comma-separated fallback URLs>
```

In the Helm chart templates (`deployment-processor.yaml`, `deployment-api.yaml`):
```yaml
envFrom:
  - secretRef:
      name: {{ .Values.config.existingSecret | default "logrix-secrets" }}
```

---

## 6. Secret Hygiene & Redaction in Code

Even with encrypted storage, credentials risk leaking if printed to logs during network failures or panic traces. Logrix enforces strict secret redaction rules across all crates:

### Redaction Implementation (`crates/logrix-rpc-gateway/src/provider.rs`):
```rust
impl RpcProvider {
    /// Mask sensitive authentication keys and credentials before logging
    pub fn masked_url(&self) -> String {
        let mut parsed = match url::Url::parse(&self.url) {
            Ok(u) => u,
            Err(_) => return "***".to_string(),
        };

        // Redact basic auth username and password
        if !parsed.username().is_empty() {
            let _ = parsed.set_username("***");
        }
        if parsed.password().is_some() {
            let _ = parsed.set_password(Some("***"));
        }

        // Mask path tokens (e.g. Alchemy / Infura API keys in /v2/<key>)
        let path = parsed.path().to_string();
        if path.starts_with("/v2/") && path.len() > 4 {
            parsed.set_path("/v2/***");
        }

        parsed.to_string()
    }
}
```

### Result in Production Pod Logs:
```text
2026-10-24T04:21:28Z  WARN logrix_rpc_gateway::gateway: Provider call failed, tripping breaker provider="primary" url="https://eth-mainnet.g.alchemy.com/v2/***" elapsed_ms=1053 error=[ChainRpc::RateLimited] HTTP 429 Too Many Requests
```
Sensitive API tokens and database passwords never enter stdout, CloudWatch Logs, Datadog, or Grafana dashboards.

---

## 7. Operational Troubleshooting

### Verifying ServiceAccount Token Projection:
```bash
kubectl exec -it deployment/logrix-processor -n logrix -- printenv | grep AWS
# Output:
# AWS_ROLE_ARN=arn:aws:iam::206388234927:role/logrix-production-processor-irsa
# AWS_WEB_IDENTITY_TOKEN_FILE=/var/run/secrets/eks.amazonaws.com/serviceaccount/token
```

### Verifying IAM AssumeRoleWithWebIdentity via AWS CLI:
```bash
JWT_TOKEN=$(kubectl exec deployment/logrix-processor -n logrix -- cat /var/run/secrets/eks.amazonaws.com/serviceaccount/token)

aws sts assume-role-with-web-identity \
  --role-arn "arn:aws:iam::206388234927:role/logrix-production-processor-irsa" \
  --role-session-name "test-session" \
  --web-identity-token "$JWT_TOKEN"
```
