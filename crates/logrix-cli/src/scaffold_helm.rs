use crate::scaffold::ScaffoldConfig;

pub fn values_local_template(cfg: &ScaffoldConfig) -> String {
    format!(
        r#"# Local Kubernetes Profile (Minikube / Kind / K3s)
# 1-click deployment with built-in PostgreSQL 16 and RabbitMQ

image:
  repository: ghcr.io/vishal-770/logrix
  tag: "0.1.0"
  pullPolicy: IfNotPresent

localDev:
  enabled: true

config:
  chainId: {chain_id}
  contractAddress: "{addr}"
  rpcUrl: "{rpc_url}"
  queueDriver: "rabbitmq"
  rabbitmqUrl: "amqp://logrix:logrixpassword@logrix-rabbitmq:5672/%2f"
  databaseUrl: "postgres://logrix:logrixpassword@logrix-postgres:5432/logrix"
  reconcilerIntervalSecs: 15
  ringBufferDepth: 64

ingester:
  replicas: 1
  leaderElection:
    enabled: false

processor:
  replicas: 1
  keda:
    enabled: false

api:
  replicas: 1
  hpa:
    enabled: false
  service:
    type: NodePort
    port: 4000
"#,
        chain_id = cfg.chain_id,
        addr = cfg.contract_address,
        rpc_url = cfg.rpc_url,
    )
}

pub fn values_aws_template(cfg: &ScaffoldConfig) -> String {
    format!(
        r#"# Production AWS EKS Profile
# Provisions on AWS Aurora Serverless v2 PostgreSQL, Amazon SQS, and Amazon S3

image:
  repository: ghcr.io/vishal-770/logrix
  tag: "0.1.0"
  pullPolicy: IfNotPresent

localDev:
  enabled: false

config:
  chainId: {chain_id}
  contractAddress: "{addr}"
  rpcUrl: "{rpc_url}"
  queueDriver: "sqs"
  reconcilerIntervalSecs: 30
  ringBufferDepth: 256
  # AWS RDS / Aurora Connection
  databaseUrl: "postgres://admin:YOUR_PASSWORD@your-aurora-cluster.rds.amazonaws.com:5432/logrix?sslmode=require"
  sqsQueueUrl: "https://sqs.us-east-1.amazonaws.com/123456789012/{name}-queue"
  s3Bucket: "{name}-cold-blobs"

serviceAccount:
  create: true
  name: "logrix-sa"
  annotations:
    eks.amazonaws.com/role-arn: "arn:aws:iam::123456789012:role/{name}-eks-role"

ingester:
  replicas: 2
  leaderElection:
    enabled: true
    leaseName: "logrix-ingester-lease"

processor:
  replicas: 2
  keda:
    enabled: true
    minReplicaCount: 2
    maxReplicaCount: 50
    queueType: sqs
    targetQueueDepth: 100

api:
  replicas: 2
  hpa:
    enabled: true
    minReplicas: 2
    maxReplicas: 20
    targetCPUUtilizationPercentage: 75
  service:
    type: ClusterIP
    port: 4000
"#,
        chain_id = cfg.chain_id,
        addr = cfg.contract_address,
        rpc_url = cfg.rpc_url,
        name = cfg.name,
    )
}
