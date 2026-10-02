# Logrix: Kubernetes & Autoscaling Architecture Plan

**Document Version:** 1.0  
**Status:** Approved Architecture Blueprint  
**Primary Focus:** Production-Grade Kubernetes Orchestration, Targeted Autoscaling (KEDA + HPA + Karpenter), and Workload Separation.

---

## Executive Summary & Core Principle

> **Core Philosophy:** *Use each autoscaler for what it's best at, instead of one tool everywhere.*
>
> - **KEDA:** For queue-driven workers (Decoders, Webhook workers), unlocking scale-to-zero and event-driven agility.
> - **HPA (Built-in):** For the API tier, scaling on CPU, memory, and request latency.
> - **Node Autoscaler (Karpenter / Cluster Autoscaler):** Provisioning compute nodes underneath when pods queue up.
> - **Zero Autoscaling:** For singleton roles (Listener, Controller) using leader election and hot standbys instead.
> - **VPA:** Restricted to **recommendation-only mode** to right-size resource requests without fighting HPAs.

### System Boundary: Kubernetes vs Cloud Managed Services

Logrix leverages Kubernetes for what container orchestration excels at—stateless scale-out, self-healing, rolling deployments, and granular isolation—while delegating stateful infrastructure to battle-tested cloud-managed services:

```text
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                                 KUBERNETES CLUSTER                                     │
│                                                                                        │
│   ┌─────────────────────┐  ┌─────────────────────┐   ┌─────────────────────────────┐   │
│   │   logrix-listener   │  │  logrix-controller  │   │         logrix-api          │   │
│   │ (Active + Standby)  │  │ (Active + Standby)  │   │  (HPA: CPU / Latency)       │   │
│   │ [On-Demand Nodes]   │  │ [On-Demand Nodes]   │   │  [On-Demand Nodes]          │   │
│   └──────────┬──────────┘  └──────────┬──────────┘   └──────────────┬──────────────┘   │
│              │                        │                             │                  │
│              │                        │                             │                  │
│   ┌──────────▼──────────┐  ┌──────────▼──────────┐   ┌──────────────▼──────────────┐   │
│   │ logrix-decoder-live │  │logrix-decoder-backfill  │    logrix-webhook-worker    │   │
│   │ (Min: 2, Max: 6)    │  │(KEDA: Scale 0 to 50)│   │  (KEDA: Webhook queue)      │   │
│   │ [On-Demand Nodes]   │  │[Spot / Preemptible] │   │  [On-Demand / Spot]         │   │
│   └──────────┬──────────┘  └──────────┬──────────┘   └──────────────┬──────────────┘   │
└──────────────┼────────────────────────┼─────────────────────────────┼──────────────────┘
               │                        │                             │
               ▼                        ▼                             ▼
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                              EXTERNAL MANAGED SERVICES                                 │
│                                                                                        │
│  ┌──────────────────────┐  ┌────────────────────────┐  ┌────────────────────────────┐  │
│  │ AWS SQS / RabbitMQ   │  │  Postgres (RDS/Aurora) │  │  AWS S3 / Cloud Blob       │  │
│  │ (Live, Backfill, DLQ)│  │  (Event Store & State) │  │  (Raw Blocks & Snapshots)  │  │
│  └──────────────────────┘  └────────────────────────┘  └────────────────────────────┘  │
│                                                                                        │
│  ┌──────────────────────────────────────────────────────────────────────────────────┐  │
│  │ EVM JSON-RPC Nodes (RPC Gateway: Alchemy, Infura, QuickNode, Private Nodes)     │  │
│  └──────────────────────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 1. Tool Matrix & Responsibilities

| Tool | What It Scales | Use in Logrix? | Cardinal Rules |
| :--- | :--- | :--- | :--- |
| **KEDA** | Pod count based on external events (queue backlog, lag, custom metrics) | **Yes** — Decoders and Webhook workers | Provides scale-to-zero for backfill and rich trigger catalog. |
| **HPA** (Built-in) | Pod count based on resource saturation (CPU, Memory, HTTP latency) | **Yes** — API service | Traditional request-response traffic scaling. |
| **VPA** (Vertical Pod Autoscaler) | Pod CPU and memory request/limit sizes | **Recommend-only mode** | **NEVER** use VPA auto-mutating mode alongside HPA. Use it to tune baseline requests. |
| **Karpenter / Cluster Autoscaler** | Infrastructure nodes (EC2 instances, VMs) | **Yes** — Cluster node layer | Ensures pending pods immediately provision right-sized nodes. |

### Two Golden Rules
1. **Never attach a KEDA `ScaledObject` and an independent Kubernetes `HorizontalPodAutoscaler` to the same Deployment.** KEDA generates and manages its own internal HPA; an external HPA will fight it in an infinite loop.
2. **Never configure VPA and HPA to scale on the same metric.**

---

## 2. Workload & Scaler Configuration per Role

| Role | Workload Type | Scaler | Scale Signal | Min Replicas | Max Replicas | Scale to Zero? | Node Type |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Decoder (Live)** | Deployment | KEDA or Fixed | Head lag (blocks behind head), message age | 2 | Small (4–6) | **No** | On-Demand |
| **Decoder (Backfill)** | Deployment | KEDA | Controller metric: `logrix_desired_replicas` (or native queue depth) | 0 | Capped by DB write budget & RPC limit (e.g. 50) | **Yes** | Spot / Preemptible |
| **Webhook Worker** | Deployment | KEDA | Webhook queue depth | 1 | Per-endpoint limits (e.g. 10) | Optional | On-Demand / Spot |
| **API** | Deployment | Built-in HPA | CPU utilization (>65%) & request latency | 2 | Capped by DB read capacity (e.g. 15) | **No** | On-Demand |
| **Listener** | Deployment | **None** (Leader Election) | n/a (Single active per chain) | 2 (1 active + 1 standby) | 2 | **No** | On-Demand |
| **Controller** | Deployment | **None** (Leader Election) | n/a (Single active per deployment) | 2 (1 active + 1 standby) | 2 | **No** | On-Demand |
| **Backfill Planner** | Part of Controller | **None** | n/a | Singleton | Singleton | **No** | On-Demand |
| **gRPC Plugins** | Sidecar or Deployment | HPA | CPU / Memory | Follows Decoder | Follows Decoder | Follows Decoder | Same as host |
| **Nodes** | Karpenter / CA | Node Autoscaler | Pending pod resource requests | Min cluster size | Cloud quota | n/a | Mixed |

### Why Singletons are Not Autoscaled
The **Listener** and **Controller** maintain chain-specific sequential state (WebSocket connections, parent-hash tracking, reorg detection, reconciler loops). Adding replicas cannot parallelize this work; it would only cause duplicate RPC subscriptions and split-brain reorg triggers.
- High availability is achieved using **Kubernetes Leases** (`coordination.k8s.io`) or **Postgres advisory locks**.
- Two pods run per chain: **1 Leader (active)** and **1 Follower (hot standby)** with pod anti-affinity.
- Scaling across more chains is achieved by running separate singletons partitioned by `chain_id`.

### Live Indexing vs Backfill Scaling Dynamics
- **Live indexing** is low-volume and continuous (e.g., 1 block every 12s on Ethereum, 2s on Base). Queue depth is almost always 0 or 1. Scaling live decoders on queue depth does nothing. Live decoders run at a steady minimum of 2 for high availability, scaling up only if head lag breaches thresholds.
- **Backfill** is high-throughput, bursty, and parallelizable. Decoders must scale to 0 when idle and scale up to dozens of pods during backfills.

---

## 3. Don't Scale on Queue Depth Alone: The 2-Layer Budget Design

### The Naive Autoscaling Trap
If the backfill queue has 100,000 blocks, a naive queue-depth scaler will scale pods to maximum (e.g., 50 pods). However, if:
- The Postgres database write pool is saturated, or
- The RPC provider is returning `429 Too Many Requests`,

adding more pods will accelerate connection exhaustion and trigger a catastrophic failure cascade.

### The 2-Layer Control Solution

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        2-LAYER CONTROL SYSTEM                          │
│                                                                        │
│  [Inside Each Pod] - Fast Sub-Second Loop:                             │
│  Adaptive Concurrency (AIMD)                                           │
│  • Senses DB query latency & RPC 429s                                  │
│  • Cuts in-flight workers locally (TCP-like backoff)                   │
│                                                                        │
│  [Across Pods] - Slow Decisecond Loop:                                 │
│  Logrix Controller Budget Computation                                  │
│  • Aggregates cluster-wide DB & RPC headroom                           │
│  • Calculates logrix_desired_replicas                                  │
│  • Publishes metric to Prometheus                                      │
│  • KEDA scales pods up/down within safe limits                         │
└────────────────────────────────────────────────────────────────────────┘
```

The Logrix Controller computes the desired replica count every 15 seconds:

$$\text{desired\_replicas} = \min \left( \text{replicas}_{\text{queue\_depth}}, \frac{\text{DB write budget}}{\text{writers per pod}}, \frac{\text{RPC throughput cap}}{\text{throughput per pod}} \right)$$

This metric is exported as:
```promql
logrix_desired_replicas{role="decoder_backfill", project="my-project"}
```

KEDA scales on this metric, ensuring the cluster **never scales into an outage**.

---

## 4. Native Scalers vs Controller Metric

| Aspect | Native KEDA Scaler (SQS, RabbitMQ, Redis) | Controller Metric (`logrix_desired_replicas`) |
| :--- | :--- | :--- |
| **How it works** | KEDA queries SQS/RabbitMQ API directly | KEDA queries Prometheus for the Controller's calculation |
| **Dependencies** | Requires only KEDA operator | Requires KEDA + Prometheus + active Controller |
| **Budget Awareness** | ❌ None (blind to DB and RPC limits) | ✅ Full awareness of DB write pool and RPC CU budgets |
| **Multi-Queue / Plugins**| Must configure separate scaler per queue tech | ✅ Unified signal regardless of queue driver |
| **Recommended Use** | Simpler setups or fallback | **Production default** for backfill decoders |

### Queue-Specific Traps Handled
- **AWS SQS:** `ApproximateNumberOfMessages` is delayed (up to 60s) and does not count in-flight messages unless configured with `ApproximateNumberOfMessagesNotVisible`.
- **RabbitMQ:** Must distinguish between `messages_ready` and `messages_unacknowledged`.
- **Redis / BullMQ:** BullMQ stores jobs across multiple keys (`wait`, `active`, `prioritized`, `delayed`). A simple `LLEN` query will miss delayed or prioritized work.

---

## 5. Kubernetes Resources Required (The Complete Inventory)

To operate Logrix in production on Kubernetes, the following resources must be deployed:

```text
k8s/
├── base/
│   ├── kustomization.yaml
│   ├── namespaces.yaml                   # logrix namespace
│   ├── rbac/
│   │   ├── serviceaccount-irsa.yaml      # AWS IAM role annotation for SQS/S3
│   │   ├── role-lease-coordination.yaml  # K8s Lease permissions for Leader Election
│   │   └── rolebinding.yaml
│   ├── configs/
│   │   ├── configmap-logrix.yaml         # logrix.yaml configuration
│   │   └── configmap-presets.yaml        # chain presets & provider profiles
│   ├── secrets/
│   │   └── externalsecret.yaml           # ExternalSecrets / AWS Secrets Store CSI
│   ├── workloads/
│   │   ├── deployment-listener.yaml      # 2 replicas, leader election
│   │   ├── deployment-controller.yaml    # 2 replicas, leader election
│   │   ├── deployment-decoder-live.yaml  # 2-4 replicas, high priority
│   │   ├── deployment-decoder-backfill.yaml # 0-50 replicas, spot tolerations
│   │   ├── deployment-webhook.yaml       # 1-10 replicas
│   │   └── deployment-api.yaml           # 2-15 replicas
│   ├── autoscaling/
│   │   ├── keda-triggerauth-aws.yaml     # IRSA auth for KEDA
│   │   ├── keda-scaledobject-backfill.yaml # Scale-to-zero backfill
│   │   ├── keda-scaledobject-webhook.yaml  # Webhook queue scaler
│   │   └── hpa-api.yaml                  # Plain HPA for API
│   ├── networking/
│   │   ├── service-api.yaml              # ClusterIP for GraphQL/REST
│   │   ├── ingress.yaml                  # Ingress with TLS & rate limits
│   │   └── networkpolicies.yaml          # Egress/Ingress micro-segmentation
│   ├── scheduling/
│   │   ├── poddisruptionbudgets.yaml     # PDBs for high availability
│   │   └── karpenter-nodepool.yaml       # NodePools (Spot vs On-Demand)
│   └── monitoring/
│       ├── servicemonitor.yaml           # Prometheus Operator scraping
│       └── prometheusrule.yaml           # Alerting rules
```

---

## 6. Concrete Manifests & Specifications

### 6.1 ServiceAccount with AWS IRSA (Keyless Cloud Access)
Decoders and listeners need access to external AWS SQS and AWS S3 without hardcoded credentials:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: logrix-workload-sa
  namespace: logrix
  annotations:
    eks.amazonaws.com/role-arn: arn:aws:iam::123456789012:role/LogrixWorkloadRole
```

### 6.2 Leader Election RBAC (for Listener & Controller Singletons)
Singletons coordinate through native Kubernetes Leases (`coordination.k8s.io`):

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: Role
metadata:
  name: logrix-leader-election-role
  namespace: logrix
rules:
  - apiGroups: ["coordination.k8s.io"]
    resources: ["leases"]
    verbs: ["get", "list", "watch", "create", "update", "patch", "delete"]
---
apiVersion: rbac.authorization.k8s.io/v1
kind: RoleBinding
metadata:
  name: logrix-leader-election-rb
  namespace: logrix
subjects:
  - kind: ServiceAccount
    name: logrix-workload-sa
    namespace: logrix
roleRef:
  kind: Role
  name: logrix-leader-election-role
  apiGroup: rbac.authorization.k8s.io
```

### 6.3 Backfill Decoder Deployment (Spot Friendly & Graceful Shutdown)

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: logrix-decoder-backfill
  namespace: logrix
  labels:
    app.kubernetes.io/name: logrix
    app.kubernetes.io/component: decoder-backfill
spec:
  replicas: 0 # Managed entirely by KEDA
  selector:
    matchLabels:
      app.kubernetes.io/name: logrix
      app.kubernetes.io/component: decoder-backfill
  template:
    metadata:
      labels:
        app.kubernetes.io/name: logrix
        app.kubernetes.io/component: decoder-backfill
    spec:
      serviceAccountName: logrix-workload-sa
      terminationGracePeriodSeconds: 300 # Allow in-flight range chunk to finish and checkpoint
      tolerations:
        - key: "spot"
          operator: "Exists"
          effect: "NoSchedule"
      affinity:
        nodeAffinity:
          preferredDuringSchedulingIgnoredDuringExecution:
            - weight: 100
              preference:
                matchExpressions:
                  - key: "karpenter.sh/capacity-type"
                    operator: In
                    values: ["spot"]
      containers:
        - name: decoder
          image: ghcr.io/logrix/logrix:v0.1.0
          args: ["start", "--role=decoder", "--queue-type=backfill"]
          resources:
            requests:
              cpu: "500m"
              memory: "512Mi"
            limits:
              cpu: "2000m"
              memory: "2Gi"
          lifecycle:
            preStop:
              exec:
                command: ["/bin/sh", "-c", "logrix ctl drain --timeout=280"]
          envFrom:
            - configMapRef:
                name: logrix-config
            - secretRef:
                name: logrix-secrets
```

### 6.4 KEDA ScaledObject: Controller-Driven Metric (Recommended)

```yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: logrix-decoder-backfill-scaler
  namespace: logrix
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: logrix-decoder-backfill
  minReplicaCount: 0 # Scales to zero when no backfills exist
  maxReplicaCount: 50 # Hard boundary protected by DB concurrency budget
  pollingInterval: 15
  cooldownPeriod: 300
  advanced:
    horizontalPodAutoscalerConfig:
      behavior:
        scaleUp:
          stabilizationWindowSeconds: 0 # Scale up quickly when backfill begins
          policies:
            - type: Percent
              value: 100
              periodSeconds: 15
            - type: Pods
              value: 10
              periodSeconds: 15
          selectPolicy: Max
        scaleDown:
          stabilizationWindowSeconds: 600 # 10-minute hold to prevent flapping
          policies:
            - type: Pods
              value: 2
              periodSeconds: 60
  triggers:
    - type: prometheus
      metadata:
        serverAddress: http://prometheus-operated.monitoring.svc.cluster.local:9090
        query: logrix_desired_replicas{role="decoder_backfill"}
        threshold: "1" # 1 replica per 1 unit of desired replicas
```

### 6.5 KEDA ScaledObject: Direct SQS Fallback Trigger

```yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: logrix-decoder-backfill-sqs-scaler
  namespace: logrix
spec:
  scaleTargetRef:
    name: logrix-decoder-backfill
  minReplicaCount: 0
  maxReplicaCount: 30
  pollingInterval: 15
  cooldownPeriod: 300
  triggers:
    - type: aws-sqs-queue
      authenticationRef:
        name: keda-aws-credentials
      metadata:
        queueURL: https://sqs.us-east-1.amazonaws.com/123456789012/logrix-backfill-queue
        queueLength: "10" # Target 10 chunk jobs per pod
        awsRegion: "us-east-1"
```

### 6.6 Native HPA for API Tier

```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: logrix-api-hpa
  namespace: logrix
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: logrix-api
  minReplicas: 2
  maxReplicas: 15 # Guarded by DB read-replica connection capacity
  metrics:
    - type: Resource
      resource:
        name: cpu
        target:
          type: Utilization
          averageUtilization: 65
    - type: Resource
      resource:
        name: memory
        target:
          type: Utilization
          averageUtilization: 75
  behavior:
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
        - type: Percent
          value: 20
          periodSeconds: 60
```

### 6.7 PodDisruptionBudgets (PDB)

```yaml
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: logrix-live-decoder-pdb
  namespace: logrix
spec:
  minAvailable: 1
  selector:
    matchLabels:
      app.kubernetes.io/name: logrix
      app.kubernetes.io/component: decoder-live
---
apiVersion: policy/v1
kind: PodDisruptionBudget
metadata:
  name: logrix-api-pdb
  namespace: logrix
spec:
  minAvailable: 1
  selector:
    matchLabels:
      app.kubernetes.io/name: logrix
      app.kubernetes.io/component: api
```

### 6.8 Karpenter NodePool Specification (Spot vs On-Demand)

```yaml
apiVersion: karpenter.sh/v1beta1
kind: NodePool
metadata:
  name: logrix-spot-workers
spec:
  template:
    spec:
      requirements:
        - key: "karpenter.sh/capacity-type"
          operator: In
          values: ["spot"]
        - key: "kubernetes.io/arch"
          operator: In
          values: ["amd64", "arm64"]
        - key: "node.kubernetes.io/instance-type"
          operator: In
          values: ["c6i.xlarge", "c6a.xlarge", "c7g.xlarge", "m6i.xlarge"]
      taints:
        - key: "spot"
          value: "true"
          effect: "NoSchedule"
  limits:
    cpu: "200"
    memory: "400Gi"
  disruption:
    consolidationPolicy: WhenUnderutilized
    expireAfter: 720h
```

---

## 7. Critical Pitfalls & Engineering Defenses

### 1. Scaling on CPU for Queue Workers (The Anti-Pattern)
- **Problem:** Blockchain decoders spend significant time awaiting network I/O (RPC responses, Postgres batch transactions). CPU utilization often remains below 25% while thousands of blocks pile up in the queue.
- **Defense:** Never attach CPU-based HPA to decoders. Always scale on **queue backlog / lag** or **`logrix_desired_replicas`**.

### 2. Abrupt Termination Mid-Batch (Data Safety)
- **Problem:** When KEDA scales down, Kubernetes sends `SIGTERM`. If the decoder is killed mid-write, transactions rollback and messages must be redelivered.
- **Defense:**
  1. Set `terminationGracePeriodSeconds: 300`.
  2. Implement an in-binary `SIGTERM` handler: immediately stop consuming from queue, finish processing the current range chunk, commit the DB transaction, and exit `0`.
  3. Ensure the queue visibility timeout (e.g., SQS Visibility Timeout = 600s) exceeds `terminationGracePeriodSeconds`.

### 3. Metric Flapping (Thrashing)
- **Problem:** Bursty backfills cause rapid oscillation between scaling up 20 pods and scaling down to 0, overloading both the Kubernetes API and node provisioners.
- **Defense:** Asymmetric scaling behavior:
  - Scale up: aggressive (0s stabilization window, 100% burst).
  - Scale down: conservative (600s stabilization window, max 2 pods dropped per minute).

### 4. Partition Limits vs Consumer Count
- **Problem:** If a workload is strictly ordered per contract address (or using SQS FIFO message group IDs), running more pods than active partitions results in idle workers that burn memory without throughput gains.
- **Defense:** The Controller's `logrix_desired_replicas` formula factors in `active_partitions_count`:
  $$\text{desired\_replicas} \le \text{active\_partitions}$$

### 5. High-Availability KEDA
- **Problem:** If the KEDA operator fails or crashes, scaling freezes.
- **Defense:** Run the KEDA operator in HA mode (2 replicas with leader election) and specify `fallback` replicas in `ScaledObject` spec in case metrics queries fail.

---

## 8. Outside Kubernetes: Orchestrator Portability

Logrix preserves its cloud-agnostic ethos by keeping all autoscaling logic decoupled from Kubernetes primitives:

| Platform | Autoscaling Approach | Signal Source |
| :--- | :--- | :--- |
| **Docker Compose / Local** | Static replica count (`--scale decoder=4`) | In-pod adaptive concurrency (AIMD) handles local CPU/DB pressure |
| **AWS ECS** | AWS Application Auto Scaling | SQS `ApproximateNumberOfMessagesVisible` per task |
| **Azure Container Apps** | Native built-in KEDA | Uses identical `ScaledObject` trigger definitions |
| **Nomad / Cloud Run** | Native webhook or Prometheus autoscalers | Scrapes `/metrics` for `logrix_desired_replicas` |

---

## 9. Implementation Roadmap & Verification

### Kubernetes Deliverables (Phase 6 Integration)
- [ ] Implement leader election in `logrix-core` using `coordination.k8s.io` Leases and Postgres advisory lock fallback.
- [ ] Export `logrix_desired_replicas{role=...}` from the Controller service.
- [ ] Package official Helm Chart (`deploy/helm/logrix`) with configurable profiles (`aws-sqs-rds`, `generic-k8s`).
- [ ] Build end-to-end KEDA scaling test in CI with `LocalStack` and `kind` (Kubernetes in Docker).
- [ ] Validate scale-down graceful drain (`1 -> 50 -> 1` pods) with zero uncommitted transactions or duplicate event errors.
