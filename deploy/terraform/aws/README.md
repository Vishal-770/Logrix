# AWS Infrastructure Provisioning for Logrix (Terraform)

Production Terraform configuration provisioning the AWS infrastructure required by Logrix on Amazon Web Services (AWS).

---

## Architecture Components

- **Amazon VPC**: Multi-AZ network across 3 Availability Zones with Public, Private Worker, and Private Database subnets. Includes high-availability NAT Gateway and S3 Gateway VPC Endpoint.
- **Amazon EKS**: Managed Kubernetes cluster (v1.30) with AL2023 Managed Node Groups, OIDC Provider, and IAM Roles for Service Accounts (IRSA).
- **Amazon Aurora Serverless v2 PostgreSQL**: Multi-AZ PostgreSQL 16 cluster scaling from 0.5 to 16.0 ACU with tuned connection pooling parameters.
- **Amazon SQS**: 4 distributed queues: Live jobs, Backfill jobs, Webhook jobs, and Dead-Letter Queue (DLQ).
- **Amazon S3**: Block archive bucket with server-side encryption, Block Public Access, and automated lifecycle tiering (Standard -> Standard-IA -> Glacier Instant Retrieval).
- **AWS Secrets Manager**: Encrypted storage for Aurora PostgreSQL master credentials.

---

## Provisioning Guide

### 1. Configure Variables
Copy the template and adjust region or capacity:
```bash
cp terraform.tfvars.example terraform.tfvars
```

### 2. Plan and Apply
```bash
terraform init
terraform plan -out=tfplan
terraform apply tfplan
```

### 3. Retrieve Outputs
Print all generated resource identifiers:
```bash
terraform output
```

Key outputs required for deployment:
- `eks_cluster_name`: Name of the EKS cluster (e.g., `logrix-production-eks`)
- `rds_cluster_endpoint`: Primary writer endpoint for Aurora PostgreSQL
- `rds_secret_arn`: ARN of the secret in AWS Secrets Manager containing database credentials
- `sqs_live_queue_url`: Live ingestion SQS queue URL
- `sqs_backfill_queue_url`: Historical backfill SQS queue URL
- `s3_bucket_name`: Name of the S3 archive bucket
- `irsa_*_role_arn`: Discrete IAM IRSA roles for Ingester, Processor, and API

---

## Connecting EKS and Kubernetes

### 1. Update Kubeconfig
```bash
aws eks update-kubeconfig --region <aws-region> --name <eks_cluster_name>
kubectl get nodes -o wide
```

### 2. Fetch Database Connection String
Extract the pre-formatted PostgreSQL connection URL directly from Secrets Manager:
```bash
DB_URL=$(aws secretsmanager get-secret-value \
  --region <aws-region> \
  --secret-id "<rds_secret_arn>" \
  --query SecretString --output text | jq -r .url)
```

### 3. Create Namespace and Secrets Vault
Inject credentials into the cluster using standard uppercase environment variable keys:
```bash
kubectl create namespace logrix

kubectl create secret generic logrix-secrets -n logrix \
  --from-literal=DATABASE_URL="$DB_URL" \
  --from-literal=RPC_URL="https://eth-mainnet.g.alchemy.com/v2/<API_KEY>" \
  --from-literal=RPC_FALLBACK_URLS="https://rpc.payload.de,https://eth.llamarpc.com"
```

---

## Destroying Infrastructure
To tear down all provisioned resources:
```bash
terraform destroy
```
