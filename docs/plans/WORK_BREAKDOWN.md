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

### Part 2: RPC Gateway & Backfill Engine - [COMPLETED]
- **Objective:** Multi-provider RPC gateway, CU cost budgeting, adaptive range sizing, and bulk data streaming.
- **Completed Components & Technical Implementation:**
  1. `crates/logrix-rpc-gateway`: Dedicated workspace crate with SRP isolation:
     - `budget.rs`: Method-weighted Compute Unit (CU) ledger (`eth_getLogs`=75, `eth_getBlock`=20, `eth_blockNumber`=10) with atomic counters (`AtomicU64`) and automated backfill pausing on budget exhaustion while leaving the GraphQL query API intact. Includes real-time cost estimation ($1.00 / 1M CU).
     - `singleflight.rs`: Concurrent in-flight request deduplication merging duplicate queries across workers into a single network execution via `tokio::sync::broadcast` channels.
     - `provider.rs`: Managed provider state with rolling latency moving average (`avg_latency_ms`), per-provider circuit breakers, and exponential cooldown recovery probes (`10s -> 20s -> 40s -> 120s`). Enhanced with **Decorrelated Full Jitter** bounded in `[base/2, base]` to eliminate thundering herd recovery storms.
     - `pool.rs`: Multi-provider router with latency-weighted priority fallback. Includes `select_provider_excluding(&tried_names)` to guarantee no failed node is repeatedly queried during single-request failover retries.
     - `bulk.rs`: Fast-path bulk streaming connector for SQD Network / HyperSync archives (50,000+ blocks/sec at zero CU cost) with transparent fallback to JSON-RPC.
     - `gateway.rs`: Central `RpcGateway` implementing `ChainPort` with transparent failover, automatic metrics recording, and fallback error propagation.
  2. `crates/logrix-cli`:
     - Added `logrix backfill --dry-run` pre-flight cost estimator (reporting block counts, chunks, CUs, USD costs, and estimated duration).
     - Wired `RpcGateway` into all pipeline modes (`ingester`, `processor`, `all-in-one`) with support for `--rpc-fallback-urls` and `--cu-budget`.
  3. `crates/logrix-resilience`:
     - Dual-layer jitter protection: In-flight retry jitter (`RetryPolicy`) and Decorrelated Full Jitter for circuit breaker cooldowns (`cooldown_duration`).
- **Custom Automated Tests (100% Passing):**
  - `gateway_tests.rs`:
    1. `test_provider_pool_routing_and_priority`: Priority-tier routing and circuit breaker failover.
    2. `test_cu_budget_enforcement`: Method-weighted CU accumulation and threshold exhaustion check.
    3. `test_singleflight_concurrent_dedup`: 8 concurrent tasks dispatched, exactly 1 network execution performed.
    4. `test_gateway_fault_tolerance_automatic_failover_and_recovery`: Simulates HTTP 429 on primary, transparent failover to secondary, circuit tripping, and recovery jitter bounds.
    5. `test_gateway_all_providers_failing_error_propagation`: Verifies proper `ErrorClass::Transient` propagation when entire pool fails.
    6. `test_gateway_budget_exhaustion_blocks_further_network_calls`: Proves budget exhaustion immediately rejects calls without hitting downstream providers (network closure invocation counter = 0).
    7. `test_provider_latency_moving_average_and_jitter`: Mathematical verification of exponential moving average (`(old*4 + new)/5`) and decorrelated jitter random distribution within `[base/2, base]`.
  - **33 total unit & integration tests passing across all workspace crates.**
- **Git Commits:**
  - `4aebdac`: `feat(rpc): cost-aware gateway, adaptive chunking, and bulk stream fast-path`
  - `093228d`: `feat(resilience): add decorrelated full-jitter to provider recovery cooldown`
  - `63a5d1d`: `test(gateway): add comprehensive fault-tolerance, failover recovery, and jitter tests`

---

### Part 3: Reorgs & Self-Healing Reconciler (COMPLETED)
- **Objective:** Guarantee zero silent data loss under blockchain reorgs and dropped messages.
- **Granular Steps Completed:**
  1. Implemented parent-hash continuity check with in-memory `RollingBlockBuffer` and remote RPC ancestor walking in `ReorgDetector`.
  2. Implemented fork-point discovery and atomic database soft-delete rollback (`is_reverted = TRUE, reverted_at = NOW()`) in `ReorgHandler` and `PostgresStore`.
  3. Implemented `event.reverted` webhook dispatching with exponential backoff and DLQ routing in `WebhookDispatcher`.
  4. Implemented self-healing `GapReconciler` background loop: periodically detects missing block gaps between checkpoints and chain head, dispatching refill range jobs.
  5. Integrated full reconciler engine into CLI `processor` and `all-in-one` runtimes with `ProcessorConfig` and configurable flags (`--webhook-url`, `--reconciler-interval-secs`, `--ring-buffer-depth`).
- **Tests Added & Verified:**
  1. `test_rolling_buffer_parent_continuity_and_rewind`: Verifies sliding buffer capacity, parent matching, and rewind.
  2. `test_reorg_detector_catches_fork_and_ancestor`: Verifies detection of divergent fork and ancestor resolution.
  3. `test_reorg_detector_gap_and_duplicate_handling`: Verifies gap detection on jumps and height reorgs on divergent hashes.
  4. `test_reorg_handler_executes_atomic_rollback_and_webhook_dispatch`: Verifies atomic soft-delete, checkpoint rewind, and webhook event dispatch.
  5. `test_gap_reconciler_detects_and_refills_missing_ranges`: Verifies periodic scanning for missing block gaps and backfill dispatch.
  6. `test_webhook_dispatcher_no_target_acks_immediately`: Verifies graceful ACK when no external webhook URL is configured.
  - **40 total unit & integration tests passing across all workspace crates.**
- **Git Commit:** `feat(reconciler): parent-hash reorg engine, soft-delete rollback, and self-healing gap loop`

---

### Part 4: User Logic Engine (YAML + TS/WASM) — COMPLETED
- **Objective:** Enable users to define mappings in declarative YAML or write custom TypeScript handlers executing in sandboxed WASM.
- **Granular Steps Completed:**
  1. Implemented `crates/logrix-handlers` standalone crate with modular, single-responsibility architecture (<200 lines per file).
  2. Built declarative mapping engine (`src/declarative.rs`) parsing dot-notation paths (`log.address`, `log.block_number`, `log.topics[n]`, `log.data[start..end]`) into structured entities.
  3. Integrated `wasmtime = "29"` JIT engine with strict sandboxing: fuel metering (1M units), epoch deadline interruption, and linear memory ceiling (64MB).
  4. Built guest-host ABI bridge (`src/wasm/host_funcs.rs`) exposing zero-copy host functions: `logrix_db_get`, `logrix_db_set`, `logrix_emit`, `logrix_log`.
  5. Implemented transactional staging buffer (`src/staging.rs`) ensuring isolation and atomic commit or rollback on trap/panic.
  6. Created developer starter kit (`templates/typescript-starter/`) with AssemblyScript SDK and sample manifest.
  7. Integrated `--manifest-path` CLI option and wired user logic engine into `run_processor` for both live blocks and backfill ranges.
- **Tests Added & Verified:**
  1. `test_declarative_mapping_field_extraction`: Verifies extraction of contract address, indexed topics, block metadata, and data byte slices.
  2. `test_manifest_yaml_parsing`: Verifies parsing of declarative mappings, WASM handlers, and memory configurations from YAML.
  3. `test_wasm_state_mutation_and_entity_emission`: Verifies guest execution calling host state updates (`logrix_db_set`) and emitting custom entities (`logrix_emit`).
  4. `test_wasm_memory_ceiling_limit`: Verifies trapping when memory growth exceeds the configured maximum page ceiling.
  5. `test_wasm_infinite_loop_fuel_exhaustion_traps`: Verifies deterministic termination of runaway execution via fuel metering.
  6. `test_wasm_transactional_rollback_on_panic`: Verifies that trapped or panicked executions cleanly roll back all staged mutations.
  7. `test_user_logic_engine_orchestration_and_state_persistence`: Verifies end-to-end multi-event state accumulation and commit across both declarative and WASM pipelines.
  - **47 total unit & integration tests passing across all workspace crates.**
- **Git Commit:** `feat(handlers): declarative yaml mapper and sandboxed typescript wasm engine`

---

### Part 5: Dynamic GraphQL & Schema Engine — COMPLETED
- **Objective:** Auto-generate GraphQL API schema from `schema.graphql` (SDL) or `schema.yaml` with filtering, sorting, pagination, live subscriptions, and embedded GraphiQL UI.
- **Granular Steps Completed:**
  1. Implemented `schema_parser.rs` supporting both GraphQL SDL (`schema.graphql`) with `@entity` directives and YAML schemas (`schema.yaml`).
  2. Built `dynamic_schema.rs` and `list_resolver.rs` compiling user entities into dynamic GraphQL types at runtime using `async_graphql::dynamic`.
  3. Implemented PostgreSQL migration `20261003000002_dynamic_entities.sql` creating `logrix_entities` with JSONB payloads, composite B-tree lookup indexes, and GIN `jsonb_path_ops` indexing.
  4. Implemented `query_builder.rs` translating GraphQL `where` filter arguments (`_gt`, `_lt`, `_contains`, `_not`, etc.) and pagination into parameterized SQL queries via `sqlx::QueryBuilder` with strict SQL injection protection.
  5. Implemented `subscriptions.rs` with `SubscriptionBroadcaster` over Tokio broadcast ring and PostgreSQL `LISTEN/NOTIFY` bridge for Kubernetes multi-pod live subscriptions.
  6. Embedded interactive **GraphiQL IDE** in Axum at `/` and `/graphiql` with live subscription testing and schema auto-complete.
  7. Enforced query guardrails: max query depth (7), max complexity (200), max page size (1000), and 5-second database query timeout.
  8. Integrated `--schema-path` CLI option and wired dynamic entity persistence into `run_processor` for both live blocks and backfill ranges.
- **Tests Added & Verified:**
  1. `test_parse_graphql_sdl_entities`: Verifies parsing of GraphQL SDL entity models, scalar fields, and directives.
  2. `test_parse_yaml_schema_entities`: Verifies equivalent entity parsing from YAML schemas.
  3. `test_query_builder_equality_and_comparison_filters`: Verifies parameterized SQL generation for numeric comparisons and cursor pagination.
  4. `test_query_builder_string_operators`: Verifies SQL `ILIKE` pattern generation for string contains/starts_with filters.
  5. `test_query_builder_injection_protection`: Verifies rejection of malicious identifiers and SQL injection attempts.
  6. `test_dynamic_graphql_schema_execution_and_health`: Verifies runtime dynamic schema compilation and query execution against Postgres.
  7. `test_dynamic_graphql_query_depth_guardrail`: Verifies rejection of queries exceeding max depth 7.
  8. `test_subscription_broadcaster_pub_sub`: Verifies asynchronous event emission and reception across WebSocket subscribers.
  - **56 total unit & integration tests passing across all workspace crates.**
- **Git Commit:** `feat(api): dynamic graphql engine with filtering, websocket subscriptions, and embedded graphiql`

---

### Part 6: Kubernetes & KEDA Autoscaling — COMPLETED
- **Objective:** Production-grade Kubernetes orchestration with targeted KEDA, HPA, and Active-Passive Leader Election.
- **Granular Steps Completed:**
  1. Implemented `LeaderElectionPort` port trait in `logrix-core/src/ports/leader.rs`.
  2. Implemented `LocalLeaderElector` (for standalone/local testing) and `KubernetesLeaseElector` (`coordination.k8s.io/v1`) in `logrix-core/src/leader.rs`.
  3. Integrated active-passive HA into CLI `run_ingester` with `--enable-leader-election`, `--lease-name`, `--k8s-namespace`, and `--pod-name` flags.
  4. Implemented two-tier graceful drain in CLI `run_processor`: listens for SIGTERM/SIGINT, pauses new message ingestion, and allows in-flight block processing to complete cleanly within Kubernetes `terminationGracePeriodSeconds: 60`.
  5. Built production Helm 3 chart in `deploy/helm/logrix/` (`Chart.yaml`, `values.yaml`, `_helpers.tpl`, `configmap.yaml`, `rbac.yaml`, `deployment-ingester.yaml`, `deployment-processor.yaml`, `deployment-api.yaml`, `scaledobject-processor.yaml`, `hpa-api.yaml`, `pdb.yaml`).
  6. Verified synchronized Kustomize base and local environments (`kustomize build deploy/k8s/base` and `kustomize build deploy/k8s/local`).
- **Tests Added & Verified:**
  1. `test_local_leader_elector`: Verifies local leader election, step down, and re-acquisition.
  2. `test_kubernetes_lease_elector_unreachable_api_graceful_fail`: Verifies graceful fallback to standby without panic when K8s API server is unreachable.
  - **58 total unit & integration tests passing across all workspace crates.**
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
