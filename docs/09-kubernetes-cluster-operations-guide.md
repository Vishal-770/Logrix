# Logrix Kubernetes Cluster Operations & Troubleshooting Guide

This document provides a comprehensive operational guide for inspecting, debugging, and managing all Kubernetes resources in the Logrix production cluster (`namespace: logrix`).

---

## 1. Pod Inspection & Health

### 1.1 List All Pods with Node Placement and IPs
Display all running pods, readiness status, restart counts, age, internal container IPs, and host worker nodes:
```bash
kubectl get pods -n logrix -o wide
```

### 1.2 Inspect Container Events and Detailed Pod State
When a pod is stuck in `CrashLoopBackOff`, `Pending`, or `ImagePullBackOff`, describe the pod to inspect lifecycle state, container exit codes, termination reasons, and cluster scheduler events:
```bash
# Describe a specific pod
kubectl describe pod <pod-name> -n logrix

# Example: Describe the active processor pod
kubectl describe pod -l app.kubernetes.io/component=processor -n logrix
```

### 1.3 View Previous Container Crash Logs
If a container crashed and restarted, view the stdout/stderr logs from the **previous terminated instance**:
```bash
kubectl logs <pod-name> -n logrix --previous --tail=100
```

### 1.4 Real-Time Streaming Logs with Component Filtering
Stream live logs across deployments using component labels:
```bash
# Stream Processor logs (ingestion, block processing, state mutations)
kubectl logs -f deployment/logrix-processor -n logrix

# Stream Ingester logs (EVM RPC polling, leader election status)
kubectl logs -f deployment/logrix-ingester -n logrix

# Stream GraphQL API logs (client queries, request latency)
kubectl logs -f deployment/logrix-api -n logrix
```

---

## 2. Resource Utilization & Node Scheduling

### 2.1 Check Pod Resource Limits and Requests
Inspect configured CPU and memory requests/limits versus actual node capacity:
```bash
kubectl get pods -n logrix -o custom-columns=\
NAME:.metadata.name,\
CPU_REQ:.spec.containers[*].resources.requests.cpu,\
CPU_LIM:.spec.containers[*].resources.limits.cpu,\
MEM_REQ:.spec.containers[*].resources.requests.memory,\
MEM_LIM:.spec.containers[*].resources.limits.memory
```

### 2.2 Inspect Node Capacity and Allocatable Resources
Inspect physical worker node CPU, RAM, and maximum allocatable pod capacity:
```bash
kubectl describe nodes | grep -A 8 "Allocated resources:"
```

---

## 3. High Availability, Leader Election & Deployments

### 3.1 Inspect Active Ingester Leader Lease
The Ingester runs in an active-passive configuration using a Kubernetes Coordination Lease (`logrix-ingester-lease`). Check which pod holds the lock and its lease duration:
```bash
kubectl get lease -n logrix

# Inspect current leader holder identity and renewal timestamps
kubectl get lease logrix-ingester-lease -n logrix -o yaml
```

### 3.2 Inspect Deployment Rollout Status and History
Track rolling upgrades and view deployment revision history:
```bash
# Check rollout status
kubectl rollout status deployment/logrix-processor -n logrix

# View revision history
kubectl rollout history deployment/logrix-processor -n logrix

# Roll back to the previous deployment revision if needed
# kubectl rollout undo deployment/logrix-processor -n logrix
```

### 3.3 Restart Deployments Gracefully
Trigger a zero-downtime rolling restart of a microservice:
```bash
kubectl rollout restart deployment/logrix-processor -n logrix
```

---

## 4. KEDA Event-Driven Autoscaling (ScaledObject)

### 4.1 Inspect Autoscaler Health and Metrics
Check if KEDA has successfully connected to the SQS queue triggers and is actively evaluating queue lengths:
```bash
kubectl get scaledobject -n logrix

# Inspect detailed trigger evaluation, queue URLs, and target thresholds
kubectl describe scaledobject logrix-processor-autoscaler -n logrix
```

### 4.2 Inspect the Underlying Horizontal Pod Autoscaler (HPA)
KEDA automatically generates and controls an underlying Kubernetes HPA. Inspect target vs current metric values:
```bash
kubectl get hpa -n logrix

# Inspect HPA events, scaling conditions, and replica calculations
kubectl describe hpa keda-hpa-logrix-processor-autoscaler -n logrix
```

---

## 5. Networking, Services & Cluster Connectivity

### 5.1 Inspect ClusterIP Services & Endpoints
Verify that Kubernetes Services correctly discover and route traffic to healthy ready pods:
```bash
# Check Service ports and ClusterIPs
kubectl get svc -n logrix

# Verify active backend Pod Endpoints
kubectl get endpoints -n logrix
```

### 5.2 Port-Forwarding (Local Access to Private Services)
Connect your local workstation directly to private cluster services without exposing them publicly:
```bash
# Connect to GraphQL Playground & Studio (http://localhost:4000)
kubectl port-forward svc/logrix-api 4000:4000 -n logrix

# Connect to Prometheus Metrics (http://localhost:4000/metrics)
kubectl port-forward deployment/logrix-api 4000:4000 -n logrix
```

### 5.3 In-Cluster Network Testing via Ephemeral Curl Pod
Debug internal DNS resolution, network policies, or API connectivity from inside the private cluster network:
```bash
kubectl run net-test --image=curlimages/curl:latest --rm -i --restart=Never -n logrix -- \
  curl -s http://logrix-api:4000/healthz
```

---

## 6. Configuration & Secrets Inspection

### 6.1 Inspect Injected ConfigMaps
View runtime environment configuration, contract manifests, and GraphQL schemas:
```bash
kubectl get configmap -n logrix

# Inspect full config data
kubectl describe configmap logrix-config -n logrix
```

### 6.2 Inspect Secrets Metadata (Safe)
Verify secret presence, key names, and creation timestamps without exposing plaintext values:
```bash
kubectl get secrets -n logrix

# Inspect Secret keys and annotations
kubectl describe secret logrix-secrets -n logrix
```

---

## 7. Security, RBAC & ServiceAccounts

### 7.1 Inspect IAM Roles for Service Accounts (IRSA) Annotations
Verify that the pod ServiceAccount is properly annotated with the AWS IAM Role ARN:
```bash
kubectl get serviceaccount logrix-sa -n logrix -o yaml
```

### 7.2 Test RBAC Authorization Permissions
Check if a service account has permissions to perform specific Kubernetes API operations:
```bash
# Test if logrix-sa can update coordination leases (Leader Election)
kubectl auth can-i update leases.coordination.k8s.io \
  --as=system:serviceaccount:logrix:logrix-sa \
  -n logrix

# Test if logrix-sa can read secrets
kubectl auth can-i get secrets \
  --as=system:serviceaccount:logrix:logrix-sa \
  -n logrix
```

---

## 8. Cluster-Wide Troubleshooting & Event Auditing

### 8.1 View Real-Time Cluster Events Sorted by Time
View warning events, failed container starts, OOM kills, and scheduling failures across the namespace:
```bash
kubectl get events -n logrix --sort-by='.lastTimestamp'
```

### 8.2 Filter Warning Events Only
Isolate errors, eviction notices, and failures:
```bash
kubectl get events -n logrix --field-selector type=Warning --sort-by='.lastTimestamp'
```

### 8.3 Execute Interactive Shell Inside Pod
Open a debug shell inside a running container:
```bash
kubectl exec -it deployment/logrix-api -c api -n logrix -- /bin/sh
```
