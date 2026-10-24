# AWS Networking & VPC Architecture

This document provides a comprehensive technical reference for the Virtual Private Cloud (VPC), subnet segmentation, routing topology, and network security implemented in Logrix on AWS.

---

## 1. Architectural Philosophy: Defense-in-Depth & Wire Speed

The Logrix AWS networking architecture adheres to three core production principles:
1. **Zero Public Database Exposure**: Databases and worker nodes reside exclusively within isolated private subnets with no public IPv4 addresses and no direct inbound routes from the internet.
2. **Wire-Speed Pod Networking**: Workloads run on the native AWS VPC CNI, eliminating overlay network encapsulation (VXLAN/IPIP) penalties and assigning real VPC IP addresses directly to Kubernetes pods.
3. **Internal Backbone Cost Optimization**: High-volume data transfers (such as raw block archives streamed to Amazon S3) bypass NAT Gateways via AWS Gateway VPC Endpoints, eliminating data processing charges and maximizing bandwidth.

---

## 2. End-to-End VPC Topology

```text
                               AWS REGION (us-east-1)
+-----------------------------------------------------------------------------------------+
| VPC CIDR: 10.0.0.0/16 (65,536 Total IPs)                                                |
| DNS Hostnames: Enabled | DNS Resolution: Enabled                                        |
|                                                                                         |
|   +---------------------------------------------------------------------------------+   |
|   | PUBLIC SUBNETS (0.0.0.0/0 -> Internet Gateway: igw)                             |   |
|   |                                                                                 |   |
|   |  [us-east-1a] 10.0.0.0/20         [us-east-1b] 10.0.16.0/20     [us-east-1c]    |   |
|   |  +--------------------------+     +-----------------------+     10.0.32.0/20    |   |
|   |  | NAT Gateway (Elastic IP) |     | Public ALBs / NLBs    |     (Spare AZ)      |   |
|   |  +------------^-------------+     +-----------------------+                     |   |
|   +---------------|-----------------------------------------------------------------+   |
|                   | Outbound SNAT                                                       |
|   +---------------v-----------------------------------------------------------------+   |
|   | PRIVATE SUBNETS (0.0.0.0/0 -> NAT Gateway)                                      |   |
|   |                                                                                 |   |
|   |  [us-east-1a] 10.0.64.0/20        [us-east-1b] 10.0.80.0/20     [us-east-1c]    |   |
|   |  +--------------------------+     +-----------------------+     10.0.96.0/20    |   |
|   |  | EKS Node Group (Workers) |     | EKS Worker Nodes      |                     |   |
|   |  | |-- logrix-ingester pod  |     | |-- logrix-processor  |                     |   |
|   |  | |-- logrix-api pod       |     | |-- keda-operator     |                     |   |
|   |  +------------+-------------+     +-----------+-----------+                     |   |
|   |               |                               |                                 |   |
|   |               | Security Group (Port 5432)    | S3 Gateway VPC Endpoint         |   |
|   |               v                               v (No NAT Data Fees)              |   |
|   |  +--------------------------+     +-----------------------+                     |   |
|   |  | Aurora Serverless v2     |     | S3 Cold Block Archive |                     |   |
|   |  | PostgreSQL Cluster       |     | s3://logrix-prod-...  |                     |   |
|   |  +--------------------------+     +-----------------------+                     |   |
|   +---------------------------------------------------------------------------------+   |
+-----------------------------------------------------------------------------------------+
```

---

## 3. CIDR Allocation & Multi-AZ Subnetting

The VPC is defined in `deploy/terraform/aws/modules/vpc/main.tf` with a `/16` network CIDR block (`10.0.0.0/16`), providing 65,536 addresses. Subnets are programmatically calculated across three Availability Zones using `cidrsubnet(var.cidr_block, 4, ...)` to produce `/20` subnets (4,096 IPs each):

```hcl
locals {
  azs = slice(data.aws_availability_zones.available.names, 0, 3)
}

resource "aws_subnet" "public" {
  count                   = 3
  vpc_id                  = aws_vpc.this.id
  cidr_block              = cidrsubnet(var.cidr_block, 4, count.index)
  availability_zone       = local.azs[count.index]
  map_public_ip_on_launch = true

  tags = {
    Name                                        = "${var.project_name}-${var.environment}-pub-${local.azs[count.index]}"
    "kubernetes.io/role/elb"                    = "1"
    "kubernetes.io/cluster/${var.cluster_name}" = "shared"
  }
}

resource "aws_subnet" "private" {
  count             = 3
  vpc_id            = aws_vpc.this.id
  cidr_block        = cidrsubnet(var.cidr_block, 4, count.index + 4)
  availability_zone = local.azs[count.index]

  tags = {
    Name                                        = "${var.project_name}-${var.environment}-priv-${local.azs[count.index]}"
    "kubernetes.io/role/internal-elb"           = "1"
    "kubernetes.io/cluster/${var.cluster_name}" = "shared"
  }
}
```

### Subnet Allocation Matrix

| Subnet Identifier | Availability Zone | CIDR Block | Usable IPs | Role / Placement |
|---|---|---|---|---|
| `pub-us-east-1a` | `us-east-1a` | `10.0.0.0/20` | 4,091 | NAT Gateway, Public Load Balancers |
| `pub-us-east-1b` | `us-east-1b` | `10.0.16.0/20` | 4,091 | Redundant Public Load Balancers |
| `pub-us-east-1c` | `us-east-1c` | `10.0.32.0/20` | 4,091 | Multi-AZ Expansion / Public Ingress |
| `priv-us-east-1a` | `us-east-1a` | `10.0.64.0/20` | 4,091 | EKS Worker Nodes, Aurora PostgreSQL |
| `priv-us-east-1b` | `us-east-1b` | `10.0.80.0/20` | 4,091 | EKS Worker Nodes, Aurora PostgreSQL |
| `priv-us-east-1c` | `us-east-1c` | `10.0.96.0/20` | 4,091 | EKS Worker Nodes, Aurora PostgreSQL |

---

## 4. Routing & Gateway Architecture

### Public Route Table (`aws_route_table.public`)
- **Default Route**: `0.0.0.0/0 -> aws_internet_gateway.this`
- **Association**: Associated with all three public subnets.
- **Function**: Handles ingress traffic for external callers and serves as the egress path for the NAT Gateway Elastic IP.

### Private Route Table (`aws_route_table.private`)
- **Default Route**: `0.0.0.0/0 -> aws_nat_gateway.this`
- **Association**: Associated with all three private subnets.
- **Function**: EKS worker nodes and containers reach the external internet (pulling container images from GHCR, connecting to external EVM RPC providers like Alchemy or Infura, and sending AWS STS requests) via outbound Source Network Address Translation (SNAT).

---

## 5. Amazon S3 Gateway VPC Endpoint

Raw blockchain blocks and receipts are archived in Amazon S3 (`crates/logrix-blob-s3`). Archiving high-throughput chain data through a NAT Gateway incurs AWS NAT data processing charges ($0.045 per GB).

To eliminate this cost and maximize transfer speeds, a Gateway VPC Endpoint is associated with the private route table:

```hcl
resource "aws_vpc_endpoint" "s3" {
  vpc_id            = aws_vpc.this.id
  service_name      = "com.amazonaws.${var.aws_region}.s3"
  vpc_endpoint_type = "Gateway"
  route_table_ids   = [aws_route_table.private.id]

  tags = {
    Name = "${var.project_name}-${var.environment}-s3-ep"
  }
}
```

### Architectural Benefits:
1. **Zero Egress / Processing Cost**: Data transfer between `logrix-processor` pods and the S3 bucket is routed internally over the AWS network fabric at $0.00/GB.
2. **Reduced NAT Contention**: Keeps high-bandwidth blob archiving traffic away from the NAT Gateway, leaving NAT bandwidth fully available for EVM JSON-RPC polling.
3. **Private Transport**: S3 requests never exit to the public internet.

---

## 6. EKS Pod Networking via AWS VPC CNI

Logrix runs on Amazon EKS using the official **AWS VPC CNI** plugin (`AmazonEKS_CNI_Policy` attached in `deploy/terraform/aws/modules/eks/main.tf`).

### Native IP Allocation
- Unlike overlay networks (such as Flannel or Calico VXLAN) that wrap packets with extra headers, the AWS VPC CNI attaches Elastic Network Interfaces (ENIs) directly to EC2 worker nodes.
- Every Pod receives a **native, routable private IP address** directly from the `/20` private subnet pool (e.g. `10.0.64.145`).
- Packets between pods and Aurora PostgreSQL travel with zero encapsulation overhead at native VPC line speed.

### Cluster API Access Configuration
```hcl
vpc_config {
  subnet_ids              = var.subnet_ids
  endpoint_private_access = true
  endpoint_public_access  = true
}
```
- **`endpoint_private_access = true`**: Kubernetes control-plane-to-node communication occurs strictly inside the private subnets without leaving the VPC.
- **`endpoint_public_access = true`**: Operators can execute `kubectl` commands remotely using IAM-authenticated TLS sessions without requiring an active bastion host or VPN.

---

## 7. Aurora PostgreSQL Network Isolation & Security Groups

The Aurora Serverless v2 PostgreSQL database is created in `deploy/terraform/aws/modules/rds/main.tf` with multi-layered network isolation:

### Subnet Group Binding
```hcl
resource "aws_db_subnet_group" "this" {
  name       = "${var.project_name}-${var.environment}-rds-subnet-grp"
  subnet_ids = var.subnet_ids # Strictly private subnets
}
```
The database cluster is assigned exclusively to the private subnets across all 3 Availability Zones. It has no public DNS endpoint and cannot be addressed over the public internet.

### Security Group Chaining
Instead of allowing broad CIDR blocks (e.g. `10.0.0.0/16`), access is strictly controlled by referencing the EKS worker node security group ID:

```hcl
resource "aws_security_group" "rds" {
  name   = "${var.project_name}-${var.environment}-rds-sg"
  vpc_id = var.vpc_id

  ingress {
    description     = "PostgreSQL access from EKS workers"
    from_port       = 5432
    to_port         = 5432
    protocol        = "tcp"
    security_groups = [var.client_security_group_id] # module.eks.cluster_security_group_id
  }

  egress {
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}
```
Only network packets carrying the EKS cluster security group tag can complete a TCP handshake on port 5432. Any rogue host or untagged instance inside the VPC is blocked at the hypervisor level.

---

## 8. End-to-End Packet Traversal Flows

### Ingester -> External EVM RPC Node
```text
logrix-ingester (10.0.64.12)
  -> Private Route Table (priv-rt)
  -> NAT Gateway (10.0.0.45) [SNAT to Elastic IP]
  -> Internet Gateway (igw)
  -> Alchemy / Infura HTTPS RPC Endpoint
```

### Processor -> Aurora PostgreSQL
```text
logrix-processor (10.0.80.34)
  -> CoreDNS resolves Aurora endpoint to 10.0.64.88
  -> Direct VPC routing across AZ private subnets
  -> RDS Security Group validates client_security_group_id
  -> Aurora PostgreSQL Writer Instance (port 5432)
```

### Processor -> S3 Cold Archive
```text
logrix-processor (10.0.80.34)
  -> Private Route Table matches S3 Prefix List (pl-63a5400a)
  -> aws_vpc_endpoint.s3 (Gateway Endpoint)
  -> AWS Internal Backbone Network
  -> S3 Bucket (s3://logrix-production-archive-*)
```

### Operator -> GraphQL API
```text
Operator Workstation
  -> kubectl port-forward svc/logrix-api 4000:4000
  -> EKS Public API Endpoint (IAM / TLS Auth)
  -> EKS Control Plane Private ENI
  -> logrix-api Pod (10.0.64.55:4000)
```

---

## 9. Verification & Network Diagnostics

### Verify Pod IP Allocation (AWS VPC CNI)
```bash
kubectl get pods -n logrix -o wide
# Check that all pod IPs belong to the 10.0.64.0/20, 10.0.80.0/20, or 10.0.96.0/20 CIDR ranges
```

### Verify Internal PostgreSQL DNS Resolution and Port Reachability
```bash
kubectl exec -it deployment/logrix-processor -n logrix -- \
  nc -zv <aurora_cluster_endpoint> 5432
# Output: Connection to <aurora_cluster_endpoint> 5432 port [tcp/postgresql] succeeded!
```

### Verify S3 Gateway Endpoint Routing from Inside Cluster
```bash
# Verify route table has S3 endpoint target
aws ec2 describe-route-tables \
  --filters "Name=tag:Name,Values=*priv-rt*" \
  --query "RouteTables[0].Routes[?VpcEndpointId != null]"
```
