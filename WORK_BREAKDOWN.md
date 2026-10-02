# Logrix: Work Breakdown & Implementation Protocol

**Protocol Rules:**
1. **Never code without prior alignment:** Before starting any Part, a deep conversation must occur to clarify assumptions, edge cases, and eliminate hallucinations.
2. **Strict Git Tracking:** Each Part is implemented in dedicated, well-scoped commits with no messy files or unverified changes.
3. **Custom Tests Required:** Every Part must ship with custom automated tests (unit, integration, or fault-injection) that prove it works before marking the Part complete.

---

## Overview of Parts

| Part | Title | Scope & Crates | Milestone Outcome |
| :--- | :--- | :--- | :--- |
| **Part 0** | **Foundations & Core Ports** | `logrix-core`, `logrix-plugin-api`, `logrix-resilience` | Cargo workspace, domain types, traits (ports), error taxonomy, and mock tests |
| **Part 1** | **Walking Skeleton (Arbitrum Sepolia)** | `logrix-chain-evm`, `logrix-store-postgres`, `logrix-queue-rabbitmq`, `logrix-cli` | `logrix dev` auto-manages Docker (PG+RabbitMQ), indexes ERC-20 transfers into Postgres, queryable via GraphQL |
| **Part 2** | **RPC Gateway & Backfill Engine** | `logrix-chain-evm` (Gateway, Scheduler, CU Budgets, Adaptive Chunking, Bulk Streams) | High-speed backfill with cost estimator (`--dry-run`) and provider failover |
| **Part 3** | **Reorgs & Self-Healing Reconciler** | `logrix-reconciler`, `logrix-pipeline` (Rollback engine, Revert webhooks) | Automatic reorg rollback to fork point, gap refill loop, zero silent data loss |
| **Part 4** | **User Logic Engine (YAML + TS/WASM)** | `logrix-handlers`, `logrix-plugin-host` | Declarative YAML mapping + sandboxed TypeScript handlers executing in `wasmtime` |
| **Part 5** | **Dynamic GraphQL & Schema Engine** | `logrix-api`, `logrix-config` | Dynamic schema generation from `schema.yaml`, filtering, sorting, and live WebSocket subscriptions |
| **Part 6** | **Kubernetes & KEDA Autoscaling** | `deploy/helm/logrix`, KEDA ScaledObjects, Karpenter | Leader-elected singletons, 0-to-50 scale-to-zero backfill decoders on Spot nodes |
| **Part 7** | **Cloud Pack (AWS SQS, S3, RDS)** | `logrix-queue-sqs`, `logrix-blob-s3`, Terraform AWS | Native AWS integration with IRSA keyless IAM authentication and LocalStack tests |

---

## Detailed Breakdown of Each Part

### Part 0: Foundations & Core Ports
- **Objective:** Establish the Cargo workspace, domain models, error taxonomy, and abstract ports.
- **Granular Steps:**
  1. Initialize Cargo workspace with shared dependencies (`tokio`, `serde`, `thiserror`, `alloy-primitives`, `tracing`).
  2. Implement `logrix-core`: Define `BlockEnvelope`, `EventLog`, `Checkpoint`, `ChainId`, and the 6-class `ErrorClass` / `LogrixError`.
  3. Define stable port traits in `logrix-core`:
     - `QueuePort`: `publish`, `consume`, `ack`, `nack`, `depth`.
     - `StorePort`: `write_events_and_checkpoint` (atomic), `get_checkpoint`, `rollback_to_block`.
     - `ChainPort`: `fetch_logs`, `subscribe_heads`, `get_block`.
  4. Implement `logrix-resilience`: Exponential backoff with jitter, retry budgets, and token-bucket rate limiters.
- **Custom Tests:**
  - Unit tests for error classification and retry policy.
  - Property tests for checkpoint advancing and block hash uniqueness.
  - Mock port implementations validating trait compliance.
- **Git Commit:** `feat(core): workspace setup, domain models, port traits, and resilience`

---

### Part 1: Walking Skeleton (Arbitrum Sepolia ERC-20 on Kubernetes + AWS Cloud) - [COMPLETED]
- **Objective:** First working end-to-end pipeline: index live ERC-20 `Transfer` events from Arbitrum Sepolia into PostgreSQL (local or AWS RDS) via RabbitMQ or AWS SQS, queryable via GraphQL.
- **Completed Components:**
  1. `crates/logrix-store-postgres`: Vectorized `UNNEST` batch inserts for logs and token transfers, atomic checkpoint transactions, and automated SQL migrations. Works natively with both local PostgreSQL and AWS RDS / Aurora.
  2. `crates/logrix-queue-rabbitmq`: AMQP durable exchange and queues with bounded QoS prefetch and DLQ isolation for KEDA autoscaling.
  3. `crates/logrix-queue-sqs`: AWS SQS queue adapter with IAM authentication, LocalStack support, and native DLQ redrive.
  4. `crates/logrix-chain-evm`: Arbitrum Sepolia JSON-RPC client with AIMD adaptive block chunking and zero-copy ERC-20 `Transfer` event decoding.
  5. `crates/logrix-api`: `async-graphql` + `axum` GraphQL server with GraphiQL playground at `/` and query endpoint at `/graphql`.
  6. `crates/logrix-cli`: Multi-role entrypoint supporting `ingester`, `processor`, `api`, `all-in-one`, and `migrate` with dynamic `--queue-driver` selection (RabbitMQ or AWS SQS).
  7. `deploy/k8s/local/`: Declarative Kustomize manifests for local Kubernetes (PostgreSQL StatefulSet, RabbitMQ Deployment, Logrix Deployment, Service, ConfigMap).
  8. `deploy/k8s/aws-rds-sqs-example.yaml`: Production EKS manifest using IRSA for SQS and SSL-secured AWS RDS.
- **Custom Tests:** 23 unit & integration tests passing across all crates (`cargo test`).
- **Git Commits:**
  - `017a68a`: `feat: implement walking skeleton crates and local k8s manifests (Postgres, RabbitMQ, EVM, API, CLI)`
  - `4161de6`: `feat(queue): add native AWS SQS adapter and dynamic driver selection for local & cloud`

---

### Part 2: RPC Gateway & Backfill Engine
- **Objective:** Multi-provider RPC gateway, CU cost budgeting, adaptive range sizing, and bulk data streaming.
- **Granular Steps:**
  1. Implement provider pool with health checks, failover, and provider profile loading (`presets/providers/`).
  2. Implement Compute Unit (CU) budget tracking and `logrix backfill --dry-run` cost estimator.
  3. Implement adaptive chunking: halving ranges on `429` or range limit errors, gradual expansion on sparse blocks.
  4. Implement optional fast-path bulk stream connector (SQD Portal / HyperSync).
- **Custom Tests:**
  - Fake-RPC fault-injection test: simulate 429s, timeouts, and range errors to verify automatic window halving and provider switching.
  - Cost estimator accuracy verification test.
- **Git Commit:** `feat(rpc): cost-aware gateway, adaptive chunking, and bulk stream fast-path`

---

### Part 3: Reorgs & Self-Healing Reconciler
- **Objective:** Guarantee zero silent data loss under blockchain reorgs and dropped messages.
- **Granular Steps:**
  1. Implement parent-hash continuity check in Listener.
  2. Implement fork-point discovery and atomic database rollback in Controller.
  3. Implement `event.reverted` webhook dispatching.
  4. Implement Reconciler loop: detect missing block gaps between checkpoints and re-enqueue them.
- **Custom Tests:**
  - Scripted 3-block reorg simulation on a local Anvil chain: verify rollback, data deletion, and re-indexing.
  - Gap injection test: simulate dropped queue message, verify Reconciler detects and refills the gap.
- **Git Commit:** `feat(resilience): reorg rollback engine, revert webhooks, and reconciler loop`

---

### Part 4: User Logic Engine (YAML + TS/WASM)
- **Objective:** Enable users to define mappings in declarative YAML or write custom TypeScript handlers executing in sandboxed WASM.
- **Granular Steps:**
  1. Implement `logrix-handlers` declarative YAML mapper.
  2. Embed `wasmtime` runtime with guest-host interface: `ctx.db.get()`, `ctx.db.set()`, `ctx.emit()`.
  3. Build TypeScript compilation pipeline to WASM module.
- **Custom Tests:**
  - Unit tests for declarative field extractions (`log.address`, `args.amount0`, etc.).
  - WASM sandbox test: verify running balance calculation in TypeScript handler.
  - Sandbox security test: verify infinite loops or memory bombs are terminated by limits.
- **Git Commit:** `feat(handlers): declarative yaml mapper and sandboxed typescript wasm engine`

---

### Part 5: Dynamic GraphQL & Schema Engine
- **Objective:** Auto-generate GraphQL API schema from `schema.yaml` with filtering, sorting, pagination, and live subscriptions.
- **Granular Steps:**
  1. Parse `schema.yaml` into dynamic GraphQL types at runtime using `async-graphql`.
  2. Implement SQL query generation with parameterized filtering (`where`), sorting (`orderBy`), and pagination (`first`, `after`).
  3. Implement GraphQL subscriptions over WebSockets.
- **Custom Tests:**
  - Query compliance test: execute complex GraphQL queries with nested filters.
  - Query safety test: verify depth limits and complexity limits block abusive queries.
- **Git Commit:** `feat(api): dynamic graphql engine with filtering and websocket subscriptions`

---

### Part 6: Kubernetes & KEDA Autoscaling
- **Objective:** Production-grade Kubernetes orchestration with targeted KEDA, HPA, and Karpenter autoscaling.
- **Granular Steps:**
  1. Implement Kubernetes Lease leader election in `logrix-core`.
  2. Implement `logrix_desired_replicas` metric publisher in Controller.
  3. Package Helm chart (`deploy/helm/logrix`) with Deployments, KEDA `ScaledObject`, PDBs, and IRSA ServiceAccounts.
- **Custom Tests:**
  - `kind` (Kubernetes-in-Docker) e2e test: simulate queue backlog and verify KEDA scales decoders 0 -> 20 -> 0.
  - Graceful shutdown test: verify `preStop` drain completes in-flight batch before pod terminates.
- **Git Commit:** `feat(k8s): helm chart, keda autoscaling, and leader election`

---

### Part 7: Cloud Pack (AWS SQS, S3, RDS)
- **Objective:** First-class AWS cloud deployment with SQS queues, S3 blob archives, and RDS PostgreSQL.
- **Granular Steps:**
  1. Implement `logrix-queue-sqs` using `aws-sdk-sqs`.
  2. Implement `logrix-blob-s3` using `aws-sdk-s3`.
  3. Package Terraform AWS module (`deploy/terraform/aws`).
- **Custom Tests:**
  - LocalStack integration test: verify SQS message redelivery on failure and S3 raw block upload/retrieval.
- **Git Commit:** `feat(aws): sqs queue adapter, s3 blob store, and terraform module`
