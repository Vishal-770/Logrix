# Logrix

> **High-performance, modular, self-hostable blockchain indexer built in Rust.**  
> Point it at an EVM RPC, define your contracts and events, and sync a queryable database in seconds.

[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![Language](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Architecture](https://img.shields.io/badge/architecture-Modular%20Ports%20%26%20Adapters-green.svg)](#architecture)

---

## The Vision: Zero-Friction Local DX to Infinite Cloud Scale

Most blockchain indexers force an uncomfortable choice:
1. **Lightweight dev tools** that are easy to run locally on a laptop, but fall apart under heavy historical backfills or production multi-chain loads.
2. **Enterprise indexing infra** that requires running 10 microservices, Kafka, Redis, and Postgres before you can test a single event locally.

**Logrix eliminates this tradeoff.**

- **Kubernetes-Native Everywhere (Local to Cloud):** Logrix is fundamentally a Kubernetes-backed indexer. Locally, `logrix dev` spins up and connects to a local Kubernetes environment (via `kind` or existing k8s context). In production, it deploys via Helm to EKS, GKE, or AKS.
- **Identical Pod Workloads in All Environments:** Every role runs as a dedicated Kubernetes pod:
  - `logrix-listener`: Stateful singleton pod per chain with Kubernetes Lease leader election.
  - `logrix-decoder-live`: Low-latency live block decoders.
  - `logrix-decoder-backfill`: Event-driven backfill decoders autoscaled from 0 to 50 via KEDA.
  - `logrix-webhook-worker`: Outgoing webhook delivery pods.
  - `logrix-api`: Dynamic GraphQL server pods autoscaled via HPA.

```text
Local Kubernetes (kind / k8s context)          Production Kubernetes (EKS / GKE / AKS)
┌──────────────────────────────────────┐      ┌──────────────────────────────────────┐
│  $ logrix dev (Local K8s namespace)  │      │  Production Helm + KEDA + Karpenter  │
│  • logrix-listener pod (Lease leader)│      │  • logrix-listener (HA active/standby│
│  • logrix-decoder-live pod           │ ───► │  • logrix-decoder-backfill (0 to 50) │
│  • logrix-decoder-backfill pod (KEDA)│      │  • logrix-api (HPA scaled)           │
│  • logrix-api pod (GraphQL :4000)    │      │  • External AWS SQS / RDS Aurora     │
│  • Local Postgres + RabbitMQ pods    │      │  • Ingress + NetworkPolicies + IRSA  │
└──────────────────────────────────────┘      └──────────────────────────────────────┘
```

---

## Key Pillars

1. **Every Piece is Swappable (Ports & Adapters):**
   - **Queues:** SQS, RabbitMQ, Redis (BullMQ), Memory, or gRPC plugin.
   - **Databases:** Postgres, SQLite, or plugin (ClickHouse, DuckDB).
   - **Blob Stores:** AWS S3, MinIO, Local Disk, or plugin.
   - **Indexing Logic:** Declarative YAML, WASM (Rust/Go/Zig), TypeScript, or remote gRPC services.
2. **Cost-Aware RPC Gateway:**
   - Single gateway component managing provider pools (Alchemy, Infura, QuickNode, own nodes).
   - Budget tracking in native Compute Units (CU), adaptive range sizing, and rate limit defense.
   - Dry-run cost estimator (`logrix backfill --dry-run`) before launching multi-million block syncs.
3. **Bulletproof Resilience & Self-Healing:**
   - Unified error classifier (Transient, RateLimited, Shrinkable, Permanent, Integrity, Fatal).
   - Adaptive AIMD backpressure: slow consumers slow producers automatically.
   - Continuous self-healing reconciler that audits checkpoint continuity and refills missing gaps.
   - Reorg-safe: automatic rollback from fork point and webhook `event.reverted` notifications.
4. **Targeted Kubernetes Autoscaling:**
   - KEDA for queue-driven workers (scale-to-zero for backfill).
   - Built-in HPA for stateless GraphQL API instances.
   - Karpenter provisioning Spot nodes for heavy backfills, slashing cloud compute costs by 70–80%.
   - Budget-capped controller signal (`logrix_desired_replicas`) preventing clusters from scaling into a database outage.

---

## Documentation Index

Explore the complete architecture and operational specifications:

| Document | Purpose |
| :--- | :--- |
| **[`PROJECT_PLAN.md`](PROJECT_PLAN.md)** | **Master Architectural Plan:** Complete v6 specification, design decisions (D1–D28), 10-phase roadmap, and future-feature register. |
| **[`KUBERNETES_PLAN.md`](KUBERNETES_PLAN.md)** | **Production Kubernetes Architecture:** In-depth guide on KEDA, HPA, Karpenter, Workload manifests, IRSA security, and cloud separation. |
| **[`docs/01-architecture.md`](docs/01-architecture.md)** | Pipeline anatomy (Source ➔ Filter ➔ Decode ➔ Handle ➔ Sinks), traits, and extension model (WASM & gRPC). |
| **[`docs/02-rpc-gateway.md`](docs/02-rpc-gateway.md)** | RPC gateway internals, provider profiles, adaptive chunking, and CU cost management. |
| **[`docs/03-resilience-reorgs.md`](docs/03-resilience-reorgs.md)** | Failure taxonomy, backpressure mechanics, leader election, and the self-healing reconciler. |
| **[`docs/04-developer-experience.md`](docs/04-developer-experience.md)** | Local quickstart, CLI commands, YAML declarative mapping, and WASM/TypeScript handler authoring. |
| **[`docs/05-open-source-strategy.md`](docs/05-open-source-strategy.md)** | Open source pitfalls, competitive moat, community plugins, and distribution model. |

---

## 60-Second Quickstart (Local Dev Mode)

```bash
# 1. Install Logrix CLI
curl -fsSL https://get.logrix.dev | sh

# 2. Initialize a new indexer project from template
logrix init my-indexer --template erc20
cd my-indexer

# 3. Start indexing locally (in-memory queue + SQLite + dynamic GraphQL)
logrix dev
```
Open `http://localhost:4000/graphql` to query live and historical indexed data immediately.
