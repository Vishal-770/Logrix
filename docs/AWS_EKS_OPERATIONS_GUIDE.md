# AWS EKS & Logrix Production Operations Guide (A to Z)

An operational guide covering the entire lifecycle of Logrix on AWS:
1. Prerequisites & AWS Configuration
2. Provisioning Infrastructure with Terraform (EKS, VPC, Aurora RDS, SQS, S3, IAM)
3. Connecting kubectl to EKS
4. Deploying Logrix via Helm (Docker-based, Zero GitHub pulls)
5. Querying Blockchain Data (GraphQL API & Direct PostgreSQL)
6. Monitoring & Observability (Prometheus Metrics, Logs, Queue Depth)
7. Operational Maintenance & Pod Management
8. Temporary Stopping / Cost Pausing (Scale to 0)
9. Complete Destruction & Teardown (Clean Resource Removal)
10. Comprehensive Troubleshooting Reference

---

## 1. Prerequisites & AWS CLI Configuration

### Required Tools
- aws CLI (v2)
- terraform (v1.5+)
- kubectl (v1.28+)
- helm (v3.12+)

### Step 1.1: Configure AWS CLI Credentials
Ensure your AWS credentials have permissions for VPC, EC2, EKS, RDS, SQS, S3, and IAM:
```bash
aws configure
# AWS Access Key ID [None]: <YOUR_ACCESS_KEY>
# AWS Secret Access Key [None]: <YOUR_SECRET_KEY>
# Default region name [None]: us-east-1
# Default output format [None]: json
```

Verify your active identity:
```bash
aws sts get-caller-identity
```

---

## 2. Provisioning Infrastructure with Terraform

All AWS infrastructure is defined as pure Infrastructure-as-Code in deploy/terraform/aws/.

### Step 2.1: Navigate to the Terraform Directory
```bash
cd deploy/terraform/aws
```

### Step 2.2: Prepare Your Variables File
Create your terraform.tfvars from the provided example:
```bash
cp terraform.tfvars.example terraform.tfvars
```

Inspect or customize terraform.tfvars:
```hcl
aws_region              = "us-east-1"
environment             = "production"
project_name            = "logrix"
vpc_cidr                = "10.0.0.0/16"
eks_cluster_version     = "1.30"
eks_node_instance_types = ["t3.medium"]
eks_node_desired_size   = 2
eks_node_min_size       = 1
eks_node_max_size       = 5
rds_min_acu             = 0.5
rds_max_acu             = 16.0
rds_database_name       = "logrix"
rds_master_username     = "logrix_admin"
s3_bucket_name          = "logrix-production-block-archive-bucket"
sqs_enable_fifo         = false
k8s_namespace           = "logrix"
```

### Step 2.3: Initialize Terraform
Downloads the AWS provider and modules:
```bash
terraform init
```

### Step 2.4: Review Execution Plan
```bash
terraform plan -out=tfplan
```

### Step 2.5: Apply & Provision Infrastructure
Provisions the VPC, subnets, NAT gateways, EKS cluster, managed t3.medium node group, Aurora Serverless v2 PostgreSQL, SQS queues, S3 archive bucket, and IAM IRSA roles:
```bash
terraform apply tfplan
```
*(Provisioning typically takes 12 to 15 minutes primarily for EKS and Aurora).*

### Step 2.6: Capture Terraform Outputs
View and save the generated outputs for Helm configuration:
```bash
terraform output
```

Key outputs you will need:
```bash
EKS_CLUSTER_NAME=$(terraform output -raw eks_cluster_name)
RDS_ENDPOINT=$(terraform output -raw rds_cluster_endpoint)
RDS_SECRET_ARN=$(terraform output -raw rds_secret_arn)
SQS_LIVE_URL=$(terraform output -raw sqs_live_queue_url)
SQS_BACKFILL_URL=$(terraform output -raw sqs_backfill_queue_url)
PROCESSOR_ROLE_ARN=$(terraform output -raw irsa_processor_role_arn)
INGESTER_ROLE_ARN=$(terraform output -raw irsa_ingester_role_arn)
API_ROLE_ARN=$(terraform output -raw irsa_api_role_arn)
```

---

## 3. Connecting kubectl to the EKS Cluster

### Step 3.1: Update Local Kubeconfig
Point your local kubectl to the newly provisioned AWS EKS cluster:
```bash
aws eks update-kubeconfig \
  --region us-east-1 \
  --name logrix-production-eks
```

### Step 3.2: Verify Cluster Connectivity & Nodes
Verify that the t3.medium worker nodes are registered and Ready:
```bash
kubectl get nodes -o wide
```

Verify system pods (CoreDNS, kube-proxy, AWS VPC CNI):
```bash
kubectl get pods -n kube-system
```

---

## 4. Deploying Logrix via Helm (Docker-Only)

> Deployments strictly run pre-built, versioned Docker container images from the container registry (ghcr.io/vishal-770/logrix:0.6.0). Never pull code from GitHub or build binaries on AWS nodes.

### Step 4.1: Create the Kubernetes Namespace
```bash
kubectl create namespace logrix
```

### Step 4.2: Retrieve Database Password from AWS Secrets Manager
Extract the auto-generated Aurora master password from AWS Secrets Manager:
```bash
DB_PASSWORD=$(aws secretsmanager get-secret-value \
  --secret-id "$RDS_SECRET_ARN" \
  --query SecretString \
  --output text | jq -r .password)
```

Create the Kubernetes Secret containing database connection details:
```bash
kubectl create secret generic logrix-secrets \
  --namespace logrix \
  --from-literal=database-url="postgres://logrix_admin:${DB_PASSWORD}@${RDS_ENDPOINT}:5432/logrix"
```

### Step 4.3: Configure values-aws.yaml
Inspect deploy/helm/values-aws.yaml. Ensure:
- image.repository: ghcr.io/vishal-770/logrix
- image.tag: "0.6.0"
- config.queueDriver: "sqs"
- config.sqsLiveUrl: matches your sqs_live_queue_url
- serviceAccount.annotations."eks.amazonaws.com/role-arn": matches your irsa_processor_role_arn

### Step 4.4: Deploy or Upgrade via Helm
```bash
cd ../../.. # Back to repository root
helm upgrade --install logrix ./deploy/helm/logrix \
  --namespace logrix \
  -f ./deploy/helm/values-aws.yaml
```

### Step 4.5: Verify Pod Rollout
Check that all microservice components are running:
```bash
kubectl get pods -n logrix -w
```
Expected output:
```text
NAME                                READY   STATUS    RESTARTS   AGE
logrix-api-7c9b8f6d5-a1b2c          1/1     Running   0          45s
logrix-api-7c9b8f6d5-d3e4f          1/1     Running   0          45s
logrix-ingester-58d7c49b6-g5h6i     1/1     Running   0          45s
logrix-ingester-58d7c49b6-j7k8l     1/1     Running   0          45s
logrix-processor-6df9c74bf-m9n0p    1/1     Running   0          45s
logrix-processor-6df9c74bf-q1r2s    1/1     Running   0          45s
logrix-webhook-849c7b6d5-t3u4v      1/1     Running   0          45s
```

---

## 5. Querying & Inspecting Indexed Blockchain Data

### Method 1: Interactive GraphQL Playground & API (Recommended)
Forward the logrix-api service port to your local browser:
```bash
kubectl port-forward svc/logrix-api 4000:4000 -n logrix
```

1. Open http://localhost:4000/ in your browser to access the GraphiQL IDE.
2. Run GraphQL queries:
```graphql
query GetTransfers {
  transfers(first: 10, orderBy: blockNumber, orderDirection: desc) {
    id
    blockNumber
    fromAddress
    toAddress
    amount
    transactionHash
  }
}
```

Or query via curl:
```bash
curl -X POST http://localhost:4000/graphql \
  -H "Content-Type: application/json" \
  -d '{"query": "{ transfers(first: 5) { id blockNumber amount transactionHash } }"}' | jq .
```

### Method 2: Direct SQL Query on Aurora Serverless v2 PostgreSQL
Because Aurora is deployed inside private subnets, connect using a port-forwarding proxy pod:

```bash
# Start an ephemeral lightweight TCP proxy pod in the cluster
kubectl run pg-proxy --namespace logrix \
  --image=alpine/socat \
  --restart=Never \
  -- -d -d tcp-listen:5432,fork tcp-connect:${RDS_ENDPOINT}:5432

# Forward port 5432 to localhost
kubectl port-forward pod/pg-proxy 5432:5432 -n logrix
```

In a separate terminal, connect via psql:
```bash
psql -h localhost -p 5432 -U logrix_admin -d logrix
```

Run administrative SQL queries:
```sql
-- View latest indexed transfer entities
SELECT * FROM entities_transfer ORDER BY block_number DESC LIMIT 10;

-- Check latest committed block height checkpoint
SELECT * FROM _logrix_checkpoints;

-- Inspect reorg rollback audit logs
SELECT * FROM _logrix_reorg_history ORDER BY detected_at DESC LIMIT 5;
```

When done, terminate the proxy pod:
```bash
kubectl delete pod pg-proxy -n logrix
```

---

## 6. Observability, Metrics & Log Inspection

### 6.1 Prometheus Metrics Endpoint
Every Logrix service exposes real-time Prometheus metrics at /metrics on port 4000:
```bash
# In Terminal 1 (or background with '&'):
kubectl port-forward svc/logrix-api 4000:4000 -n logrix

# In Terminal 2 (scrape Prometheus metrics):
curl -s http://localhost:4000/metrics | grep logrix_
```

Key operational metrics:
- logrix_head_block_number: Current blockchain block height reached by the Ingester.
- logrix_blocks_ingested_total: Cumulative count of ingested blocks.
- logrix_reorg_depth: Depth of detected chain reorgs handled.
- logrix_queue_lag: Number of blocks queued waiting for processor consumption.
- logrix_handler_execution_duration_seconds: Latency of WebAssembly execution per event.

### 6.2 Health & Readiness Endpoints
```bash
# Liveness probe
curl -s http://localhost:4000/health

# Readiness probe (verifies database connectivity)
curl -s http://localhost:4000/ready
```

### 6.3 Real-Time Pod Log Streaming
Stream structured logs by component:

```bash
# Ingester logs (RPC polling, block batching, reorg detection)
kubectl logs -n logrix -l app.kubernetes.io/component=ingester -f --tail=100

# Processor logs (Queue message receipt, WebAssembly execution, DB persistence)
kubectl logs -n logrix -l app.kubernetes.io/component=processor -f --tail=100

# API logs (GraphQL query execution, latency, HTTP errors)
kubectl logs -n logrix -l app.kubernetes.io/component=api -f --tail=100

# Webhook logs (Event dispatching, HMAC signatures, delivery retries)
kubectl logs -n logrix -l app.kubernetes.io/component=webhook -f --tail=100
```

### 6.4 AWS SQS Queue Depth & Lag
Monitor queue backlog directly from AWS:
```bash
aws sqs get-queue-attributes \
  --queue-url "$SQS_LIVE_URL" \
  --attribute-names ApproximateNumberOfMessages ApproximateNumberOfMessagesNotVisible
```

---

## 7. Operational Maintenance & Pod Management

### Restarting a Microservice Component
Perform a zero-downtime rolling restart:
```bash
kubectl rollout restart deployment logrix-ingester -n logrix
kubectl rollout restart deployment logrix-processor -n logrix
kubectl rollout restart deployment logrix-api -n logrix
```

Check rollout status:
```bash
kubectl rollout status deployment logrix-processor -n logrix
```

### Manual Replica Scaling
```bash
# Scale processor pods to handle large backfill bursts
kubectl scale deployment logrix-processor --replicas=8 -n logrix

# Scale API pods for high GraphQL query traffic
kubectl scale deployment logrix-api --replicas=4 -n logrix
```

---

## 8. Stopping / Pausing the Stack (Cost Optimization)

To pause operations without destroying database records or Terraform infrastructure:

### Step 8.1: Scale Kubernetes Deployments to 0
```bash
kubectl scale deployment --all --replicas=0 -n logrix
```

### Step 8.2: Scale EKS Worker Node Group to 0
Using AWS CLI:
```bash
aws eks update-nodegroup-config \
  --cluster-name logrix-production-eks \
  --nodegroup-name logrix-production-eks-managed-nodes \
  --scaling-config minSize=0,desiredSize=0,maxSize=5
```
*(Stops all EC2 t3.medium instances; compute cost drops to $0.00/hr).*

### Step 8.3: Resuming Operations
When ready to resume indexing:
```bash
# 1. Scale node group back to 2 instances
aws eks update-nodegroup-config \
  --cluster-name logrix-production-eks \
  --nodegroup-name logrix-production-eks-managed-nodes \
  --scaling-config minSize=1,desiredSize=2,maxSize=5

# 2. Wait for nodes to become ready
kubectl get nodes -w

# 3. Scale Kubernetes deployments back up
kubectl scale deployment logrix-ingester --replicas=2 -n logrix
kubectl scale deployment logrix-processor --replicas=2 -n logrix
kubectl scale deployment logrix-api --replicas=2 -n logrix
kubectl scale deployment logrix-webhook --replicas=1 -n logrix
```

---

## 9. Complete Destruction & Teardown (Clean Resource Removal)

To completely remove all AWS resources and eliminate all recurring charges:

### Step 9.1: Uninstall Helm Release
Deletes all Kubernetes Deployments, Services, ConfigMaps, and Pods:
```bash
helm uninstall logrix -n logrix
kubectl delete namespace logrix
```

### Step 9.2: Empty the S3 Archive Bucket
AWS S3 will reject deletion of non-empty buckets. Empty the bucket first:
```bash
BUCKET_NAME=$(cd deploy/terraform/aws && terraform output -raw s3_bucket_name)
aws s3 rm "s3://${BUCKET_NAME}" --recursive
```

### Step 9.3: Destroy Terraform Infrastructure
Destroys EKS cluster, node groups, Aurora RDS, SQS queues, S3 bucket, IAM roles, and VPC:
```bash
cd deploy/terraform/aws
terraform destroy -auto-approve
```

Verify all resources are cleaned:
```bash
aws eks list-clusters --region us-east-1
aws rds describe-db-clusters --region us-east-1
```

---

## 10. Troubleshooting Reference

| Symptom | Cause | Solution |
|---|---|---|
| Pod Pending (Node affinity / Insufficient CPU) | Cluster nodes scaling up or pods exceed CPU/RAM limits | Run kubectl describe pod <pod_name> -n logrix and check events. Scale node group if desired size is 0. |
| CrashLoopBackOff on logrix-ingester | Database unreachable or invalid RPC URL | Check kubectl logs <pod_name> -n logrix. Verify logrix-secrets contains reachable database-url. |
| SQS AccessDeniedException | IAM Role for Service Accounts (IRSA) misconfigured | Verify eks.amazonaws.com/role-arn annotation on service account matches Terraform irsa_processor_role_arn. |
| GraphQL queries return stale blocks | Processor falling behind Ingester queue | Check SQS queue depth via aws sqs get-queue-attributes. Scale processors with kubectl scale deployment logrix-processor --replicas=5 -n logrix. |
| Reorg detected loop | Reorg deeper than buffer depth | Adjust ringBufferDepth: 512 in values-aws.yaml and re-run helm upgrade. |
