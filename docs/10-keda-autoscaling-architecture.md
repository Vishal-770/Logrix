# KEDA Autoscaling Architecture: Complete Plain-English Guide

This guide explains how **KEDA (Kubernetes Event-driven Autoscaling)** is designed, configured, and operated in the Logrix production indexer cluster.

It demystifies the relationship between **AWS Cloud** (Amazon SQS, IAM, IRSA) and **Kubernetes** (CRDs, ScaledObjects, HPAs, and Deployments) in clear, simple terms.

---

## 1. What is KEDA and Why Do We Need It?

### The Traditional Problem (Standard Kubernetes HPA)
Standard Kubernetes autoscaling only understands **internal pod metrics**:
* CPU usage (e.g. scale up if CPU > 80%)
* Memory usage (e.g. scale up if RAM > 80%)

**Why this fails for a blockchain indexer:**
* If 6,000 blocks are suddenly waiting in a queue to be processed, the processor pod might only be using **5% CPU** because it is waiting on network RPC responses.
* Standard Kubernetes sees low CPU and **refuses to launch more pods**.
* Your indexing pipeline falls hours behind even though your servers have plenty of room to scale.

### The KEDA Solution
KEDA brings **external events** (like queue depths in AWS SQS or RabbitMQ) directly into Kubernetes.
Instead of asking: *"Is the CPU busy?"*, KEDA asks:
> *"How many messages are waiting in the AWS queue right now?"*

If the queue has work, KEDA scales the worker pods up immediately before the system gets overwhelmed. When the queue is empty, KEDA scales the workers down to save money.

---

## 2. High-Level Architecture: Where Does KEDA Live?

KEDA lives **inside Kubernetes**, but it reaches out to **AWS Cloud**:

```
+-----------------------------------------------------------------------------------------+
|                                        AWS CLOUD                                        |
|                                                                                         |
|  [ Amazon SQS Queues ]                                                                  |
|   • logrix-production-live                                                              |
|   • logrix-production-backfill                                                          |
+-----------------------------------------------------------------------------------------+
       ▲                                                 ▲
       │ 1. Polls queue depth every 15s                  │ 2. Authenticates via AWS IAM
       │    (calls sqs:GetQueueAttributes)               │    (IRSA - no hardcoded keys)
       │                                                 │
+------┼-------------------------------------------------┼--------------------------------+
|      │                KUBERNETES CLUSTER (EKS)         │                                |
|      │                                                 │                                |
|      │                         +-----------------------+                                |
|      │                         │                                                        |
|      │            [ TriggerAuthentication ]                                             |
|      │            (Links K8s ServiceAccount to AWS IAM Role)                            |
|      │                         │                                                        |
|      │                         ▼                                                        |
|  [ KEDA Controller Pod ] ◄-----+                                                        |
|  (Installed in `keda` namespace)                                                        |
|      │                                                                                  |
|      │ Reads rules from                                                                 |
|      ▼                                                                                  |
|  [ ScaledObject ] (Custom Resource)                                                     |
|  • Target: Deployment logrix-processor                                                  |
|  • Min: 1 pod | Max: 50 pods                                                            |
|  • Target Queue Depth: 30 live / 50 backfill                                            |
|      │                                                                                  |
|      │ Creates & manages                                                                |
|      ▼                                                                                  |
|  [ Kubernetes HPA ] (Horizontal Pod Autoscaler)                                         |
|      │                                                                                  |
|      │ Scales pod replicas up / down                                                    |
|      ▼                                                                                  |
|  [ Deployment: logrix-processor ]                                                       |
|    [Pod 1]  [Pod 2]  [Pod 3] ... [Pod 8]                                                |
+-----------------------------------------------------------------------------------------+
```

---

## 3. The 4 Essential Pieces of the Configuration

KEDA works through four coordinated layers across AWS and Kubernetes:

### Piece 1: The AWS Queue & IAM Role (AWS Side)
Created via Terraform in `deploy/terraform`:
1. **Amazon SQS Queues**: `logrix-production-live` and `logrix-production-backfill`.
2. **AWS IAM Role (`logrix-production-keda-role`)**: Grants permission to query queue statistics:
   ```json
   {
     "Effect": "Allow",
     "Action": [
       "sqs:GetQueueAttributes",
       "sqs:GetQueueUrl",
       "sqs:ListQueues"
     ],
     "Resource": "arn:aws:sqs:us-east-1:*:logrix-production-*"
   }
   ```

### Piece 2: The Security Link (TriggerAuthentication via IRSA)
File: `deploy/helm/logrix/templates/triggerauthentication.yaml`

How does KEDA in Kubernetes get permission to call AWS without saving secret access keys in code?
* Through **IRSA (IAM Roles for Service Accounts)**.
* The `TriggerAuthentication` manifest tells KEDA: *"Use the pod's Kubernetes ServiceAccount (`logrix-sa`) to exchange tokens with AWS STS and assume the IAM role."*

```yaml
apiVersion: keda.sh/v1alpha1
kind: TriggerAuthentication
metadata:
  name: logrix-processor-aws
  namespace: logrix
spec:
  podIdentity:
    provider: aws-eks
```

### Piece 3: The Scaling Rules (ScaledObject)
File: `deploy/helm/logrix/templates/scaledobject-processor.yaml`

The `ScaledObject` is a Kubernetes Custom Resource Definition (CRD) that tells KEDA what to watch and when to scale:

```yaml
apiVersion: keda.sh/v1alpha1
kind: ScaledObject
metadata:
  name: logrix-processor-autoscaler
  namespace: logrix
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: logrix-processor
  minReplicaCount: 1
  maxReplicaCount: 50
  pollingInterval: 15
  cooldownPeriod: 300
  triggers:
    # Trigger 1: Watch the Live Head Queue
    - type: aws-sqs-queue
      metadata:
        queueURL: "https://sqs.us-east-1.amazonaws.com/.../logrix-production-live"
        queueLength: "30"
        awsRegion: "us-east-1"
      authenticationRef:
        name: logrix-processor-aws

    # Trigger 2: Watch the Backfill Reconciler Queue
    - type: aws-sqs-queue
      metadata:
        queueURL: "https://sqs.us-east-1.amazonaws.com/.../logrix-production-backfill"
        queueLength: "50"
        awsRegion: "us-east-1"
      authenticationRef:
        name: logrix-processor-aws
```

### Piece 4: The Target Workload (The Deployment)
File: `deploy/helm/logrix/templates/deployment-processor.yaml`

The standard Kubernetes `Deployment` for `logrix-processor`.
* KEDA updates the `replicas` field of this deployment automatically based on queue depth calculations.

---

## 4. How the Math Works (Step-by-Step Scenario)

Every 15 seconds (`pollingInterval: 15`), KEDA calls SQS:

1. **Quiet State (Real-Time Ingestion)**:
   * SQS Backfill Queue: 0 messages.
   * SQS Live Queue: 2 messages.
   * Math: `queueLength (2) / target (30) < 1`.
   * Result: **1 pod** (`minReplicaCount`). You pay almost nothing in compute.

2. **Heavy Load (Gap Detected or Node Restart)**:
   * A chain reorganization or node restart causes 400 backfill ranges to queue up in `logrix-production-backfill`.
   * SQS Backfill Queue: 400 messages.
   * Math: `400 messages / targetQueueDepth (50) = 8 pods`.
   * Result: KEDA commands Kubernetes to scale the deployment to **8 pods**.
   * 8 worker pods process the backlog in parallel within seconds.

3. **Backlog Cleared (Cooldown)**:
   * All 400 jobs are completed and deleted from SQS.
   * Queue depth drops back to 0.
   * KEDA waits for the `cooldownPeriod` (300 seconds / 5 minutes) to ensure no flapping, then scales the deployment back down to **1 pod**.

---

## 5. Summary Cheat Sheet: AWS vs Kubernetes Responsibility

| Component | Where Does It Live? | Responsibility |
| :--- | :--- | :--- |
| **Amazon SQS** | AWS Cloud | Stores block messages safely across distributed queues. |
| **IAM Role & Policies** | AWS Cloud | Defines which AWS permissions KEDA is allowed to use. |
| **KEDA Controller Pods** | Kubernetes (`keda` namespace) | The background daemon that calls AWS APIs every 15s. |
| **TriggerAuthentication** | Kubernetes (`logrix` namespace) | Bridges Kubernetes pod identity to the AWS IAM Role. |
| **ScaledObject** | Kubernetes (`logrix` namespace) | Configures target queues, thresholds (30 / 50), and min/max pods. |
| **HorizontalPodAutoscaler (HPA)** | Kubernetes (Built-in) | Managed automatically by KEDA to instruct the cluster scheduler. |
| **Processor Deployment** | Kubernetes (`logrix` namespace) | The actual Rust microservice pods that do the indexing work. |
