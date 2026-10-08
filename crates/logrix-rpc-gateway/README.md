# logrix-rpc-gateway

High-throughput, resilient, cost-aware JSON-RPC gateway engine for EVM chain indexing.

`logrix-rpc-gateway` wraps JSON-RPC network access across Ethereum and EVM-compatible chains. It is engineered for 99.99% indexing uptime, zero-downtime provider failover, compute unit (CU) budget protection, request deduplication (SingleFlight), and response caching.

---

## Key Features

### 1. Multi-Tier Priority Routing & Provider Pool
- **Tier-Based Fallbacks**: Providers are organized into priority tiers (Tier 1 = primary paid RPCs like Alchemy/Infura, Tier 2 = secondary paid backup, Tier 3 = public/community fallbacks).
- **Latency-Weighted EWMA**: Tracks response latency using an Exponential Weighted Moving Average (`avg_latency_ms`, alpha = 0.2). Within the same priority tier, faster providers are automatically preferred.
- **Round-Robin Load Balancing**: Evenly distributes traffic across multiple healthy providers within the same tier.
- **Circuit Breakers with Full Jitter**: Monitors consecutive HTTP/RPC failures. Once the failure threshold is reached, the failing provider is marked unhealthy and placed in cooldown (exponential backoff from 10s to 120s with full jitter), protecting downstream nodes and avoiding repeated timeout penalties.
- **Stale Tip Detection**: Tracks the highest block number observed across all providers. Providers lagging behind the network tip by more than a configurable block threshold receive health penalties to prevent indexing from stale nodes.

### 2. SingleFlight Request Coalescing
- Deduplicates identical concurrent RPC queries (such as identical `eth_getBlockByNumber` or `eth_getLogs` ranges across concurrent worker tasks).
- Multiple callers share a single in-flight network request. The leader executes the call, and followers await and receive clones of the result through broadcast channels.
- Eliminates thundering herds during indexer startup, backfill bursts, and heavy block processing.

### 3. RAII Memory Safety & Cancellation Protection
- Includes an RAII `FlightGuard` drop guard on SingleFlight operations.
- If a leader task future is dropped or cancelled (e.g. client cancellation, timeout, worker task shutdown), `FlightGuard` automatically removes the pending key from the `in_flight` registry and broadcasts a structured error to all waiting followers.
- Guarantees zero memory leaks, eliminates orphaned async channels, and prevents waiting tasks from stalling indefinitely.

### 4. Compute Unit (CU) Budget Guardrails
- **Method-Specific Cost Accounting**: Weights RPC methods according to provider billing standards (e.g., `eth_getLogs` = 75 CU, `eth_getBlockByNumber` with txs = 20 CU, `eth_blockNumber` = 10 CU).
- **Hard Spend Caps**: Configurable global CU budget (`CU_BUDGET`) prevents runaway cloud bills during large-scale historical backfills.
- **Dry-Run Estimation**: Calculates projected cost before dispatching network calls and halts queries before breaching limits.

### 5. MicroCache (LRU TTL Caching)
- Micro-caches deterministic and finalized RPC data (finalized blocks, transaction receipts, immutable chain parameters).
- Drastically reduces compute unit consumption and network round-trips for repetitive queries.

### 6. Fast-Path Bulk Streaming (`BulkStreamClient`)
- Integrated SQD Network / HyperSync archive streaming for supported chains (Ethereum Mainnet, Arbitrum, Polygon, Base).
- Enables 50,000+ blocks/sec historical backfills at 0 Compute Unit cost, seamlessly falling back to standard JSON-RPC when approaching unfinalized chain heads.

---

## Workspace Integration

`logrix-rpc-gateway` serves as the resilient network foundation for the Logrix workspace:

```
+-----------------------------------------------------------+
|                        logrix-cli                         |
|   (CLI commands, environment variables, pipeline runner)  |
+-----------------------------+-----------------------------+
                              |
+-----------------------------v-----------------------------+
|                      logrix-chain-evm                     |
|        (EvmClient, ABI decoding, event batch parser)      |
+-----------------------------+-----------------------------+
                              |
+-----------------------------v-----------------------------+
|                     logrix-rpc-gateway                    |
|   (ProviderPool, SingleFlight, CircuitBreaker, CU Budget) |
+-----------------------------+-----------------------------+
                              | implements ChainPort
+-----------------------------v-----------------------------+
|                        logrix-core                        |
|        (ChainPort trait, domain types, error models)      |
+-----------------------------------------------------------+
```

- **`logrix-core`**: Implements the `ChainPort` trait via `RpcGatewayChainPort`. Any crate in the workspace interacting with chain data relies on this abstraction without binding to network specifics.
- **`logrix-chain-evm`**: Uses `RpcGateway` as the underlying transport layer for `EvmClient`. Network retry loops, provider switching, and request coalescing are isolated from EVM ABI decoding and receipt parsing.
- **`logrix-reconciler`**: Queries `RpcGateway` during reorg detection to locate common ancestor blocks (`find_common_ancestor`). Fast failover ensures reorg handling is never blocked by a single unresponsive RPC provider.
- **`logrix-cli`**: Reads configuration variables (`RPC_URL`, `RPC_FALLBACK_URLS`, `CU_BUDGET`) and instantiates `RpcGateway` during `logrix start` and worker startup.

---

## Memory Safety & Concurrency Architecture

- **Zero-Contention Read Paths**: Hot-path provider selection utilizes read locks (`RwLock::read()`). Exclusive write locks are only held during atomic state transitions (e.g. recording a failure, updating latency moving averages).
- **No Shared Locks Across Network I/O**: Network RPC calls (`reqwest`) are strictly executed outside buffer or state locks. No locks are held across asynchronous await boundaries, preventing cross-worker deadlocks.
- **Managed Sockets & Connection Pools**: The underlying HTTP transport configures TCP keep-alive, idle connection timeouts (`90s`), and host connection limits to eliminate socket leaks and avoid file descriptor exhaustion under load.
- **Strict Error Classification**: Differentiates between transient infrastructure errors (HTTP 429, 502, 503, connection timeouts) and permanent errors (e.g. invalid query ranges or EVM execution errors). Permanent errors fail fast without tripping provider circuit breakers.

---

## Test Suite & Verification

The crate includes exhaustive unit and integration tests under `tests/`:

### `tests/failover_tests.rs`
- **`test_gateway_fault_tolerance_automatic_failover_and_recovery`**: Verifies that when a primary provider emits HTTP 429 rate-limit errors, the gateway instantly routes to the secondary provider, marks the primary as degraded, applies exponential cooldown with jitter, and recovers once health is restored.
- **`test_gateway_all_providers_failing_error_propagation`**: Verifies structured error propagation when all configured providers are exhausted.
- **`test_gateway_budget_exhaustion_blocks_further_network_calls`**: Validates that exceeding the configured CU budget immediately blocks further network calls with an explicit budget exhaustion error.
- **`test_range_limit_error_does_not_trip_circuit_breaker`**: Asserts that permanent errors (e.g., query range exceeds provider limit) are returned immediately without falsely penalizing node health.
- **`test_gateway_micro_cache`**: Confirms cached responses are returned for subsequent identical queries without triggering provider calls.

### `tests/provider_tests.rs`
- **`test_provider_pool_routing_and_priority`**: Tests priority-tier ordering and fallback behavior.
- **`test_tier_round_robin_load_balancing`**: Validates round-robin traffic distribution across multiple providers in the same tier.
- **`test_provider_latency_moving_average_and_jitter`**: Asserts that EWMA latency calculations smoothly update over successive responses.
- **`test_stale_provider_penalty`**: Confirms that providers lagging behind the cluster head block receive appropriate routing penalties.
- **`test_cu_budget_enforcement`**: Tests method-specific compute unit costs and budget tracking.
- **`test_singleflight_concurrent_dedup`**: Validates that 10 concurrent requests for the same payload trigger only 1 network execution, with all 10 tasks receiving identical results.

---

## Configuration & Environment Variables

| Variable | Description | Default | Example |
| :--- | :--- | :--- | :--- |
| `RPC_URL` | Primary JSON-RPC endpoint (Tier 1) | Required | `https://arb-mainnet.g.alchemy.com/v2/KEY` |
| `RPC_FALLBACK_URLS` | Comma-delimited fallback endpoints (Tier 2/3) | None | `https://rpc.ankr.com/arbitrum,https://arb1.arbitrum.io/rpc` |
| `CU_BUDGET` | Maximum Compute Units allowed before halting queries | Unlimited | `10000000` |
| `RPC_MAX_RETRIES` | Maximum retry attempts per request across providers | `3` | `5` |
| `RPC_TIMEOUT_SECS` | Network request timeout in seconds | `15` | `30` |
| `BULK_STREAM_URL` | Custom SQD / HyperSync archive endpoint | Chain default | `https://v2.archive.subsquid.io/network/arbitrum-one` |

---

## Production Deployment Workflow

Logrix is an SDK-first indexing platform. Developers do not manually write low-level Kubernetes pod manifests. Instead, `@logrix/sdk` scaffolds your project, compiles the AssemblyScript event handlers into WebAssembly, and exports a production-ready Helm values bundle.

### 1. Developer Flow: Scaffold, Build, and Export Values

```bash
# 1. Scaffold indexer project
npx @logrix/sdk init my-indexer
cd my-indexer

# 2. Write schema.graphql and handlers, then build WASM binary
npm run codegen
npm run build

# 3. Export production Helm values (bundles manifest, schema, and base64 WASM)
npx @logrix/sdk export-values --out indexer-values.yaml
```

The exported `indexer-values.yaml` contains your compiled WASM logic, GraphQL entity schemas, and default network parameters ready for any Kubernetes cluster.

---

### 2. Deploying According to Your Target Infrastructure

You can deploy the generated `indexer-values.yaml` to your environment of choice:

#### Target A: Local Kubernetes (Minikube / Kind / K3s)
Deploys with built-in in-cluster PostgreSQL and RabbitMQ for instant local testing:
```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f indexer-values.yaml \
  --set localDev.enabled=true
```

#### Target B: Production AWS EKS
Pairs your indexer logic with AWS Aurora Serverless PostgreSQL, Amazon SQS, Amazon S3, and KEDA autoscaling using the provided AWS profile:
```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f deploy/helm/values-aws.yaml \
  -f indexer-values.yaml \
  --set config.existingSecret="logrix-aws-secrets"
```

#### Target C: Custom / Bring-Your-Own Kubernetes Cluster
Deploys to any existing Kubernetes cluster with your own database and queue credentials:
```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f my-cluster-infra.yaml \
  -f indexer-values.yaml
```

---

### 3. Passing RPC URLs and Secrets to the Pods

Depending on your security posture, RPC endpoints can be supplied in two ways:

#### Option 1: Direct Helm CLI Override (Development & Testing)
Supply paid Alchemy/Infura endpoints or fallback providers directly at deployment time:
```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f indexer-values.yaml \
  --set config.rpcUrl="https://arb-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY" \
  --set config.rpcFallbackUrls="https://arbitrum-mainnet.infura.io/v3/YOUR_INFURA_KEY,https://arb1.arbitrum.io/rpc" \
  --set config.cuBudget="25000000"
```

#### Option 2: Pre-Created Kubernetes Secret (Production & GitOps)
To keep sensitive API keys out of Git and CI/CD logs:

1. Create a Kubernetes Secret containing your RPC URLs:
```bash
kubectl create secret generic logrix-rpc-secrets \
  --namespace logrix \
  --from-literal=RPC_URL="https://arb-mainnet.g.alchemy.com/v2/YOUR_ALCHEMY_KEY" \
  --from-literal=RPC_FALLBACK_URLS="https://arbitrum-mainnet.infura.io/v3/YOUR_INFURA_KEY,https://arb1.arbitrum.io/rpc"
```

2. Point Helm to the secret using `config.existingSecret`:
```bash
helm install my-indexer oci://ghcr.io/vishal-770/charts/logrix \
  -f indexer-values.yaml \
  --set config.existingSecret="logrix-rpc-secrets"
```

Under the hood, the Helm templates mount the secret via `envFrom.secretRef` directly into the Ingester and Processor pods. Keys in the Secret securely override matching ConfigMap placeholders and are loaded directly into the Rust gateway process memory.
