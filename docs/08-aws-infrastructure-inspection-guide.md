# Logrix AWS Infrastructure Inspection Guide (CLI Operations)

This document provides a comprehensive, production-grade CLI reference for inspecting, monitoring, and auditing all AWS infrastructure components supporting the Logrix indexer cluster.

Every command is written with dynamic shell expansion (`$(...)`) and JMESPath `--query` expressions to avoid hardcoding resource identifiers, allowing rapid copy-paste execution during live architecture reviews, operations troubleshooting, and infrastructure demos.

---

## 1. Amazon EKS (Kubernetes Control Plane & Node Groups)

### 1.1 List Clusters
List all active EKS clusters in the target region:
```bash
aws eks list-clusters --region us-east-1 --output table
```

### 1.2 Describe EKS Cluster Specification
Inspect the control plane status, Kubernetes API version, private endpoint access, IAM cluster role, and VPC configuration:
```bash
CLUSTER_NAME=$(aws eks list-clusters --region us-east-1 --query "clusters[0]" --output text)

aws eks describe-cluster \
  --name "$CLUSTER_NAME" \
  --region us-east-1 \
  --query "cluster.[name,status,version,endpoint,roleArn,resourcesVpcConfig.vpcId,resourcesVpcConfig.endpointPrivateAccess,resourcesVpcConfig.endpointPublicAccess]" \
  --output table
```

### 1.3 List and Inspect Managed Node Groups
Check worker node pools, auto-scaling configuration (min, max, desired capacity), and instance types:
```bash
# List all node groups attached to the cluster
aws eks list-nodegroups --cluster-name "$CLUSTER_NAME" --region us-east-1 --output table

# Inspect node group details
NODEGROUP_NAME=$(aws eks list-nodegroups --cluster-name "$CLUSTER_NAME" --region us-east-1 --query "nodegroups[0]" --output text)

aws eks describe-nodegroup \
  --cluster-name "$CLUSTER_NAME" \
  --nodegroup-name "$NODEGROUP_NAME" \
  --region us-east-1 \
  --query "nodegroup.[nodegroupName,status,instanceTypes[0],scalingConfig.minSize,scalingConfig.maxSize,scalingConfig.desiredSize,diskSize,capacityType]" \
  --output table
```

### 1.4 Inspect Core EKS Add-ons
Verify managed system components (Amazon VPC CNI, CoreDNS, Kube-Proxy):
```bash
aws eks list-addons --cluster-name "$CLUSTER_NAME" --region us-east-1 --output table
```

---

## 2. Amazon EC2 & EBS (Worker Compute & Storage Disks)

### 2.1 Inspect Active Kubernetes Worker Instances
List all running EC2 virtual machines, their instance types, private IPs, subnet locations, and availability zones:
```bash
aws ec2 describe-instances \
  --filters "Name=instance-state-name,Values=running" \
  --region us-east-1 \
  --query "Reservations[*].Instances[*].[InstanceId,InstanceType,PrivateIpAddress,SubnetId,Placement.AvailabilityZone,State.Name]" \
  --output table
```

### 2.2 Inspect Attached EBS GP3 Root Disks
Audit persistent block storage volumes backing the worker nodes:
```bash
aws ec2 describe-volumes \
  --region us-east-1 \
  --query "Volumes[*].[VolumeId,Size,VolumeType,Iops,Throughput,Attachments[0].InstanceId,State]" \
  --output table
```

### 2.3 Inspect EBS Snapshots & Backups
Verify backup snapshot retention:
```bash
aws ec2 describe-snapshots --owner-ids self --region us-east-1 --output table
```

---

## 3. Amazon RDS (Aurora Serverless v2 PostgreSQL)

### 3.1 List Aurora DB Clusters and Endpoints
Query writer and reader cluster endpoints for the relational entity store:
```bash
aws rds describe-db-clusters \
  --region us-east-1 \
  --query "DBClusters[*].[DBClusterIdentifier,Engine,EngineVersion,Status,Endpoint,ReaderEndpoint]" \
  --output table
```

### 3.2 Inspect Aurora Serverless v2 Auto-Scaling Limits
Audit dynamic Aurora Capacity Unit (ACU) scaling limits (`MinCapacity` to `MaxCapacity`):
```bash
DB_CLUSTER=$(aws rds describe-db-clusters --region us-east-1 --query "DBClusters[0].DBClusterIdentifier" --output text)

aws rds describe-db-clusters \
  --db-cluster-identifier "$DB_CLUSTER" \
  --region us-east-1 \
  --query "DBClusters[0].[DBClusterIdentifier,Status,ServerlessV2ScalingConfiguration.MinCapacity,ServerlessV2ScalingConfiguration.MaxCapacity,AllocatedStorage,StorageEncrypted]" \
  --output table
```

### 3.3 List Aurora DB Instances
Inspect individual cluster member instances:
```bash
aws rds describe-db-instances \
  --region us-east-1 \
  --query "DBInstances[*].[DBInstanceIdentifier,DBInstanceClass,DBClusterIdentifier,Status,AvailabilityZone]" \
  --output table
```

### 3.4 Verify Database Subnet Group Isolation
Confirm database nodes are provisioned strictly inside isolated private subnets:
```bash
aws rds describe-db-subnet-groups \
  --region us-east-1 \
  --query "DBSubnetGroups[*].[DBSubnetGroupName,VpcId,Subnets[*].SubnetIdentifier]" \
  --output json
```

---

## 4. Amazon SQS (Distributed Queue Architecture)

### 4.1 List All SQS Queue URLs
List all message queues provisioned for the indexing pipeline:
```bash
aws sqs list-queues --region us-east-1 --output table
```

### 4.2 Audit Queue Depths in Real-Time
Iterate across all queues and retrieve visible, in-flight (not visible), and delayed message counts:
```bash
for QUEUE_URL in $(aws sqs list-queues --region us-east-1 --query "QueueUrls[]" --output text); do
  echo "=== Queue: $(basename $QUEUE_URL) ==="
  aws sqs get-queue-attributes \
    --queue-url "$QUEUE_URL" \
    --region us-east-1 \
    --attribute-names ApproximateNumberOfMessages ApproximateNumberOfMessagesNotVisible ApproximateNumberOfMessagesDelayed \
    --query "Attributes" \
    --output table
done
```

### 4.3 Inspect Dead Letter Queue (DLQ) Redrive Policy
Inspect the maximum receive retry count before failed messages move to the DLQ:
```bash
LIVE_QUEUE=$(aws sqs list-queues --region us-east-1 --query "QueueUrls[?contains(@, 'live')]" --output text)

aws sqs get-queue-attributes \
  --queue-url "$LIVE_QUEUE" \
  --attribute-names RedrivePolicy \
  --region us-east-1 \
  --output json
```

### 4.4 Non-Destructive Message Inspection (Peek)
Inspect message payload schema without deleting it from the queue:
```bash
aws sqs receive-message \
  --queue-url "$LIVE_QUEUE" \
  --max-number-of-messages 1 \
  --visibility-timeout 10 \
  --attribute-names All \
  --region us-east-1 \
  --output json
```

---

## 5. Amazon S3 (Cold Storage Envelope Archive)

### 5.1 List S3 Buckets
List object storage buckets allocated to Logrix:
```bash
aws s3 ls
```

### 5.2 Summarize Total Block Archive Objects and Storage Size
Count raw block envelopes compressed with `zstd` stored in cold storage:
```bash
BUCKET=$(aws s3api list-buckets --query "Buckets[?contains(Name, 'logrix')].Name | [0]" --output text)

aws s3 ls "s3://${BUCKET}/" --recursive --human-readable --summarize
```

### 5.3 Audit Bucket Security Controls
Verify default server-side encryption and public access blocks:
```bash
aws s3api get-bucket-encryption --bucket "$BUCKET" --output json
aws s3api get-public-access-block --bucket "$BUCKET" --output json
```

---

## 6. AWS VPC & Networking (PrivateLink & Security)

### 6.1 Inspect VPC and CIDR Blocks
```bash
VPC_ID=$(aws ec2 describe-vpcs --filters "Name=tag:Name,Values=*logrix*" --region us-east-1 --query "Vpcs[0].VpcId" --output text)

aws ec2 describe-vpcs \
  --vpc-ids "$VPC_ID" \
  --region us-east-1 \
  --query "Vpcs[*].[VpcId,CidrBlock,State]" \
  --output table
```

### 6.2 Audit Public vs Private Subnet Topology
```bash
aws ec2 describe-subnets \
  --filters "Name=vpc-id,Values=$VPC_ID" \
  --region us-east-1 \
  --query "Subnets[*].[SubnetId,AvailabilityZone,CidrBlock,MapPublicIpOnLaunch,Tags[?Key=='Name'].Value|[0]]" \
  --output table
```

### 6.3 Inspect NAT Gateway Status
Check active NAT Gateways enabling outbound internet traffic for worker nodes:
```bash
aws ec2 describe-nat-gateways \
  --filter "Name=vpc-id,Values=$VPC_ID" \
  --region us-east-1 \
  --query "NatGateways[*].[NatGatewayId,State,SubnetId]" \
  --output table
```

### 6.4 Inspect VPC Endpoints (PrivateLink for SQS & S3)
Verify traffic to AWS services remains within AWS private backbone without crossing public gateways:
```bash
aws ec2 describe-vpc-endpoints \
  --filters "Name=vpc-id,Values=$VPC_ID" \
  --region us-east-1 \
  --query "VpcEndpoints[*].[VpcEndpointId,ServiceName,VpcEndpointType,State]" \
  --output table
```

### 6.5 Inspect Security Groups
```bash
aws ec2 describe-security-groups \
  --filters "Name=vpc-id,Values=$VPC_ID" \
  --region us-east-1 \
  --query "SecurityGroups[*].[GroupId,GroupName,Description]" \
  --output table
```

---

## 7. AWS IAM & Secrets Management (IRSA)

### 7.1 Inspect IAM Roles for Service Accounts (IRSA)
```bash
aws iam list-roles \
  --query "Roles[?contains(RoleName, 'logrix')].[RoleName,Arn]" \
  --output table
```

### 7.2 Inspect OIDC Identity Provider
Verify OIDC federation mapping Kubernetes ServiceAccounts to AWS IAM:
```bash
aws iam list-open-id-connect-providers --output table
```

### 7.3 Inspect Secrets Manager
Verify encrypted secrets storage for database and RPC credentials:
```bash
aws secretsmanager list-secrets \
  --region us-east-1 \
  --query "SecretList[?contains(Name, 'logrix')].[Name,ARN,LastChangedDate]" \
  --output table
```

---

## 8. AWS Cost Explorer & Financial Auditing

### 8.1 Total Month-to-Date Spend
Query current unblended charges for the running month:
```bash
aws ce get-cost-and-usage \
  --time-period Start=$(date +%Y-%m-01),End=$(date +%Y-%m-%d) \
  --granularity MONTHLY \
  --metrics "UnblendedCost" \
  --query "ResultsByTime[0].Total.UnblendedCost" \
  --output table
```

### 8.2 Service-by-Service Breakdown
List costs grouped by individual AWS service:
```bash
aws ce get-cost-and-usage \
  --time-period Start=$(date +%Y-%m-01),End=$(date +%Y-%m-%d) \
  --granularity MONTHLY \
  --metrics "UnblendedCost" \
  --group-by Type=DIMENSION,Key=SERVICE \
  --query "ResultsByTime[0].Groups[*].[Keys[0],Metrics.UnblendedCost.Amount]" \
  --output table
```

### 8.3 Daily Cost Trajectory (Last 7 Days)
Audit day-by-day spending trend to verify stability:
```bash
aws ce get-cost-and-usage \
  --time-period Start=$(date -d '7 days ago' +%Y-%m-%d),End=$(date +%Y-%m-%d) \
  --granularity DAILY \
  --metrics "UnblendedCost" \
  --query "ResultsByTime[*].[TimePeriod.Start,Total.UnblendedCost.Amount]" \
  --output table
```

### 8.4 Month-End Forecast
Query AWS Cost Explorer projected month-end closing bill:
```bash
FIRST_NEXT_MONTH=$(date -d "$(date +%Y-%m-01) +1 month" +%Y-%m-01)

aws ce get-cost-forecast \
  --time-period Start=$(date +%Y-%m-%d),End="$FIRST_NEXT_MONTH" \
  --metric "UNBLENDED_COST" \
  --granularity MONTHLY \
  --query "Total" \
  --output table
```

---

## 9. Amazon CloudWatch Telemetry

### 9.1 Query EC2 Worker Node CPU Utilization
Fetch average and maximum CPU utilization over the past hour:
```bash
NODE_ID=$(aws ec2 describe-instances --filters "Name=instance-state-name,Values=running" --region us-east-1 --query "Reservations[0].Instances[0].InstanceId" --output text)

aws cloudwatch get-metric-statistics \
  --namespace AWS/EC2 \
  --metric-name CPUUtilization \
  --dimensions Name=InstanceId,Value="$NODE_ID" \
  --start-time $(date -u -d '1 hour ago' +%Y-%m-%dT%H:%M:%SZ) \
  --end-time $(date -u +%Y-%m-%dT%H:%M:%SZ) \
  --period 300 \
  --statistics Average Maximum \
  --output table
```

---

## 10. Executive 1-Liner Health Dashboard

Execute this single composite command in bash for an instant live status check:
```bash
echo "=== LOGRIX POD STATUS ===" && \
kubectl get pods -n logrix -o wide && \
echo "" && echo "=== SQS QUEUE DEPTHS ===" && \
for q in live backfill dlq; do \
  echo -n "$q: " && aws sqs get-queue-attributes --queue-url "https://sqs.us-east-1.amazonaws.com/206388234927/logrix-production-$q" --attribute-names ApproximateNumberOfMessages --query "Attributes.ApproximateNumberOfMessages" --output text; \
done && \
echo "" && echo "=== KEDA AUTOSCALER ===" && \
kubectl get scaledobject -n logrix
```
