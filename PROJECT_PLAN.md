# Logrix: Project Plan (v6, Rust, built to last)

**Status:** Language = Rust. Audience = open source, self-hosted. Chains = any EVM chain. Design goals = customizable at every layer, scalable on any cloud or locally, resilient to failures, secure, and able to grow for years. This is a long-term platform, not a one-week project (see the roadmap for realistic timing).

**Design Rule Zero:** Before adding anything to this plan (or to the code), run the Future-Proofing Check (Section 16.5). Imagine every likely future improvement and feature, and make sure the design leaves a seam for it. Build the seam now; build the feature when it is needed.

Logrix is a self-hostable blockchain indexer. You point it at an RPC, tell it which contracts and events you care about, add your own indexing logic, and it keeps a queryable database in sync.

Its main selling point: every piece can be swapped or extended, without forking the core.

| Layer | Options (v1) | How others can add more |
| :--- | :--- | :--- |
| **Queue** | AWS SQS, RabbitMQ, BullMQ (Redis), in-memory | Rust crate or gRPC plugin |
| **Database** | Postgres, SQLite | Rust crate or gRPC plugin |
| **Blob storage** | AWS S3, MinIO, local disk | Rust crate or gRPC plugin |
| **Chain source** | JSON-RPC (any EVM chain) | Rust crate or gRPC plugin |
| **Indexing logic** | YAML, WASM (any language), TypeScript, remote service | WASM / gRPC |
| **Outputs (sinks)** | Database, webhooks | Rust crate, WASM or gRPC plugin |
| **Run mode** | Local binary, Docker Compose, any Kubernetes, AWS (first), GCP and Azure (cloud packs), hybrid | Cloud packs and Terraform modules |

---

## How to read this document

- [1. What we're building](#1-what-were-building)
- [2. Decisions](#2-decisions)
- [3. Architecture](#3-architecture)
- [4. Extensibility and customization model](#4-extensibility-and-customization-model) ← the heart of Logrix
- [5. Pluggable layers](#5-pluggable-layers)
- [6. RPC layer: live indexing and backfill](#6-rpc-layer-live-indexing-and-backfill) ← costs, public vs paid, gateway design
- [7. APIs and client languages](#7-apis-and-client-languages)
- [8. User configuration and indexing logic](#8-user-configuration-and-indexing-logic)
- [9. Correctness rules](#9-correctness-rules)
- [10. Resilience and failure handling](#10-resilience-and-failure-handling) ← backpressure and the full error catalog (10.8 to 10.12)
- [11. Scalability](#11-scalability)
- [12. Multi-cloud and local portability](#12-multi-cloud-and-local-portability)
- [13. Deployment modes](#13-deployment-modes)
- [14. Observability and security](#14-observability-and-security)
- [15. Rust stack and repo layout](#15-rust-stack-and-repo-layout)
- [16. Long-term evolution and future-proofing](#16-long-term-evolution-and-future-proofing) ← Design Rule Zero (16.5) and the future-feature register (16.6)
- [17. Roadmap](#17-roadmap)
- [18. Testing](#18-testing)
- [19. Risks](#19-risks)
- [20. Next steps](#20-next-steps)

---

## 1. What we're building

### In scope for v1
- Index events (logs) from user-chosen contracts on any EVM chain (chain presets + capability probing)
- Live indexing and historical backfill
- Reorg-safe
- User-defined logic: YAML only, or code in any language (via WASM), or a remote service
- Auto-generated GraphQL API over the user's data (plus a few more API styles, see Section 7)
- Webhooks for event delivery
- Pluggable queue, database, blob store
- A stable plugin system so the community can extend Logrix without touching the core
- Runs locally, on any Kubernetes, on AWS (first-class), and on GCP/Azure through cloud packs, or mixed
- Production-grade failure handling: network failures, slow dependencies, backpressure, poison messages, partial outages, self-healing
- Horizontal scalability on every role, with a clear scaling path from a laptop to a multi-region deployment
- Security by default: threat-modeled, least privilege, sandboxed plugins, hardened supply chain
- Customization of every resource (queue, database, blob store, chain source) at three levels: config, middleware, full custom adapter
- A cost-aware RPC gateway that serves live indexing and backfill from one provider pool (Section 6)
- A complete error model: every error is classified once and handled by policy (Section 10.8 onward)
- Future-proof design: a standing check and a register of anticipated features (Sections 16.5 and 16.6)

### Out of scope for v1 (later)
- Transaction traces and internal calls
- Non-EVM chains (the design leaves room, see 4.6)
- Web dashboard UI
- User accounts, billing, multi-tenant hosting (everyone self-hosts)
- Databases other than Postgres and SQLite (they come as plugins)

---

## 2. Decisions

### Locked (my recommendation, change if you disagree)

| # | Decision | Choice | Reason |
| :--- | :--- | :--- | :--- |
| **D1** | Language | Rust | Speed, low memory, no GC pauses, strong Ethereum tooling (`alloy`), best WASM host (`wasmtime`) |
| **D2** | Processes | One binary, many roles (`logrix start --role=decoder`) | One image, simple Helm and Docker setup |
| **D3** | Queue guarantee | At-least-once, with idempotent writes | Every queue supports this; duplicates are harmless |
| **D4** | Checkpoints | In the database, same transaction as event writes | Atomic "write + advance" |
| **D5** | Queue messages | Small references (chain, block range), not raw data | SQS 256 KB limit; raw data goes to blob store |
| **D6** | Multi-chain scope | 1 chain per deployment in v1; multi-chain seam in data model | Single-chain isolation for v1 (e.g. Arbitrum Sepolia), but `chain_id` in every key and envelope so multi-chain unlocks seamlessly without schema changes |
| **D7** | Autoscaling | KEDA for all queue types | Supports SQS, RabbitMQ and Redis |
| **D8** | Audience | Open source, self-hosted, one deployment = one team | No multi-tenant code |
| **D9** | License | Apache 2.0 | Permissive, patent grant |
| **D10** | Chains | Any EVM chain via presets + probing | See 5.5 |
| **D11** | Extension model | Seams for all languages; Declarative YAML + TypeScript (WASM) implemented first | Maximum adoption for Web3 devs via YAML + TS, with WASM/gRPC architecture open to all |
| **D12** | No dynamic Rust libraries (`.so`/`.dll`) as plugins | Use WASM or gRPC instead | Rust has no stable ABI; dynamic loading breaks across compiler versions |
| **D13** | Core API | Built-in dynamic GraphQL engine (`axum` + `async-graphql`) | Auto-generated from `schema.yaml` with filtering, sorting, pagination, and subscriptions |
| **D14** | Customizing resources | Three levels for every resource: (1) config, (2) middleware layers, (3) fully custom adapter | Lets users add logic to a queue, DB or blob store without rewriting it (Section 4.7) |
| **D15** | Resilience | One shared resilience layer applied to every external call: timeouts, retries with budgets, circuit breakers, bulkheads, rate limits | Built once, used everywhere (Section 10) |
| **D16** | Backpressure | Bounded everything, live data has priority over backfill, slow consumers slow producers | No unbounded queues or memory (Section 10.3) |
| **D17** | Self-healing | A reconciler compares checkpoints against the chain and re-enqueues any gap | Even a lost message cannot cause permanent missing data |
| **D18** | Cloud portability | Cloud-agnostic core + cloud packs. AWS and generic (RabbitMQ, Redis, Postgres, S3-compatible, any Kubernetes) first; GCP and Azure next | Section 12 |
| **D19** | Versioning | Everything is versioned (config, queue messages, plugin API, system schema), with N-1 compatibility | Rolling upgrades and a year of feature growth without breaking users |
| **D20** | Code safety | `unsafe` forbidden in core crates; security checks are CI gates | Section 14 |
| **D21** | Design Rule Zero | Every addition passes the Future-Proofing Check: leave a seam for likely future features; build only what's needed now | Section 16.5 |
| **D22** | RPC access | Everything talks to nodes only through one RPC gateway (scheduler, budgets, provider pool, error classifier, cache) | Section 6 |
| **D23** | RPC cost | Budgets measured in the provider's cost units (e.g. compute units), with a cost estimator / dry run before big backfills | Section 6.3 |
| **D24** | Provider knowledge | Provider profiles are data (YAML: limits, cost weights, error patterns), community-extensible, not hard-coded | Section 6.7 |
| **D25** | Live fetching | Logs fetched by block hash; head tracking = WebSocket + polling watchdog; gap-fill always | Section 6.5 |
| **D26** | Raw data | Store raw logs before decoding, so re-indexing costs zero RPC calls | Section 6.6 |
| **D27** | Errors | Classified once at the edge into six classes; the class (not the message) drives the action | Section 10.8 |
| **D28** | Handler failure | Per-handler policy: park (continue) or halt (stop that partition); default halt for stateful handlers | Section 10.10 |
| **D29** | Unified Storage & Queue | No toy in-memory shortcuts. Docker (Postgres + RabbitMQ) by default locally, native cloud services in prod | `logrix dev` auto-manages local Postgres + RabbitMQ containers in background; zero behavioral drift between dev and prod |
| **D30** | Hybrid RPC Gateway | JSON-RPC for live blocks + open bulk streams (SQD/HyperSync) as optional backfill fast-path | Standard JSON-RPC works everywhere; bulk streams provide 100x speedup when available |
| **D31** | Local CLI DX | `logrix dev` auto-orchestrates local Docker containers | Instant setup for developers without requiring manual container management |

### Resolved Architectural Points

| # | Topic | Decision |
| :--- | :--- | :--- |
| **Q3** | BullMQ approach | Redis adapter modeled on BullMQ first, real BullMQ compatibility via gRPC plugin later |
| **Q4** | User indexing languages | Declarative YAML + TypeScript (compiled to WASM) active in v1; architecture open to Rust/Go/gRPC |
| **Q6** | Scale target | Millions of blocks backfill, under 5 s block-to-query |
| **Q7** | Non-EVM chains | Not in v1. Keep clean Source/Decode envelope boundary so it is possible later |
| **Q8** | Other sinks (Kafka, ClickHouse) | Plugins after v1 |
| **Q9** | Cloud rollout | AWS + Generic (RabbitMQ, Postgres, S3-compatible, any K8s) first; GCP & Azure next |
| **Q10** | High availability target | Multi-AZ in v1; multi-region DR designed in, built later |
| **Q11** | Bulk data & RPC | Standard JSON-RPC + bulk stream (SQD/HyperSync) fast-path + provider profiles |
| **Q12** | Verification mode | Off by default, opt-in per chain |

---

## 3. Architecture

### 3.1 The big picture

```text
               ┌─────────────────────────────┐
               │  Ethereum RPC (HTTP + WS)   │
               └───────┬───────────────┬──────┘
                       │ new blocks    │ old blocks
                 ┌─────▼──────┐  ┌─────▼──────┐
                 │  Listener  │  │ Backfiller │
                 └─────┬──────┘  └─────┬──────┘
                       │               │
                 ┌─────▼──────┐  ┌─────▼──────┐
                 │ Live Queue │  │ Backfill Q │
                 └─────┬──────┘  └─────┬──────┘
                       └───────┬───────┘
                               │ (live always served first)
                        ┌──────▼───────┐
                        │   Decoders   │ ← autoscaled 1 to 50
                        │  pipeline:   │
                        │   filter →   │
                        │   decode →   │
                        │   handle →   │
                        │    sinks     │
                        └──┬───┬────┬──┘
              ┌────────────┘   │    └────────────┐
        ┌─────▼─────┐    ┌─────▼─────┐    ┌──────▼───────┐
        │ Database  │    │Blob store │    │Webhook Queue │
        └─────▲─────┘    └───────────┘    └──────┬───────┘
              │                                  │
        ┌─────┴─────┐                     ┌──────▼───────┐
        │    API    │→ clients / dApps    │Webhook worker│→ user endpoints
        └───────────┘                     └──────────────┘

Controller: watches for reorgs, plans backfills, owns rollback
```

### 3.2 What each part does

| Part | Job | Scales? |
| :--- | :--- | :--- |
| **Listener** | Watches for new blocks, checks parent hash (reorg detection), puts a job on the Live queue | 1 instance (leader election if more) |
| **Backfiller** | Splits a block range into chunks, puts jobs on the Backfill queue, resumes after a crash | 1 per chain |
| **Decoder** | Takes a job, fetches logs, runs the pipeline (Section 4.2), writes DB + checkpoint in one transaction | Yes, 1 to 50 via KEDA |
| **Controller** | Handles reorgs (pause, roll back, re-queue), plans backfills, reloads config | 1 instance |
| **API** | GraphQL (plus REST, see Section 7) over the DB | Yes |
| **Webhook worker** | Sends events to user URLs with signing and retry | Yes |

### 3.3 Three flows
1. **Live:** new block → Listener → Live queue → Decoder → DB (+ webhook).
2. **Backfill:** user starts backfill → Backfiller makes chunks (e.g. 2,000 blocks) → Backfill queue → Decoders take these only when Live is empty → DB. At the chain head it hands over to live.
3. **Reorg:** Listener sees a parent-hash mismatch → tells the Controller → Controller pauses, deletes data from the fork point, re-queues the blocks → webhooks get `event.reverted` first, then the new events.

### 3.4 Changes from your original diagram

| Original | Change | Why |
| :--- | :--- | :--- |
| Decoder pushes reorg to Live queue | Add a Controller | Rollback needs one owner |
| Checkpoints in S3 | Checkpoints in DB | Atomic with event writes |
| Two arrows to Client | One path: API reads DB | Duplicate arrow |
| AWS-only boxes | Generic ports; AWS is one adapter | The point of Logrix |
| Fixed Go services | Pipeline stages with extension points | Section 4 |

---

## 4. Extensibility and customization model

"Extensible to any extent" can't mean "anything, anywhere". It has to mean a small number of well-defined extension points, each stable and versioned. Without that, every change breaks someone's plugin and the core never settles. So Logrix is built from the pieces below.

### 4.1 Four extension mechanisms

| # | Mechanism | Who writes it | Language | Isolation | Speed | Best for |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **1** | **Rust crates** (compile-time, behind Cargo feature flags) | Core team, Rust contributors | Rust | None (trusted code) | Fastest | Official adapters: queue, DB, blob, chain |
| **2** | **WASM plugins** (run in wasmtime) | Any user | Any language that compiles to WASM (Rust, TypeScript via a JS runtime, Go/TinyGo, C, Zig...) | Sandboxed: CPU, memory and time limits, no ambient network or file access | Fast | User indexing logic, filters, transforms, custom sinks |
| **3** | **gRPC plugins** (separate process, talks to Logrix over a versioned protobuf API) | Any user | Any language | Process-level | Network speed | Adapters in other languages, heavy logic, existing services (e.g. a Node BullMQ bridge) |
| **4** | **Pipeline hooks** (config-driven event hooks) | Any user | Any (HTTP, WASM or gRPC target) | Depends on target | Varies | Notifications, auditing, side effects without changing logic |

> **Why not Rust dynamic libraries (`.so`/`.dll`)?** Rust has no stable binary interface, so a plugin built with a different compiler version can crash the host. WASM and gRPC give the same freedom with real isolation and no ABI risk.

### 4.2 The pipeline: where extensions plug in

Every block job flows through the same stages. Each stage is a Rust trait, and each can be replaced or extended.

```text
Source  ─►  Filter  ─►  Decode  ─►  Handle  ─►  Sink(s)
chain       which        ABI         user        DB / webhook /
port        logs?       decode      logic       files / custom
```

| Stage | Default | Extension examples |
| :--- | :--- | :--- |
| **Source** | EVM JSON-RPC | Archive node, HyperSync-style provider, Reth ExEx (Rust-native node integration), another chain family |
| **Filter** | Address + topic match from config | Custom WASM filter ("only swaps over $10k") |
| **Decode** | ABI decoder (runtime ABIs from JSON) | Custom decoders for non-standard events |
| **Handle** | Declarative mapping | WASM function, remote gRPC service |
| **Sink** | Postgres + webhooks | ClickHouse, Kafka, S3 files, custom |

Hooks fire between stages and on lifecycle events: `on_block`, `before_decode`, `after_write`, `on_reorg`, `on_backfill_complete`, `on_error`.

### 4.3 Registry and wiring
- Every adapter registers itself by name in a registry (`queue.sqs`, `store.postgres`, `sink.kafka`...)
- The config picks adapters by name; unknown names produce a clear error listing what's available
- The official binary includes the default set; custom builds pick features (`cargo build --features queue-rabbitmq,store-sqlite`) for a smaller binary
- gRPC plugins are declared in config with an address, so no rebuild is needed:
  ```yaml
  queue:
    driver: plugin # external gRPC plugin
    plugin: { address: "localhost:7001" }
  ```

### 4.4 Stability promises (so plugins don't break)

| Item | Rule |
| :--- | :--- |
| **Plugin API** (`logrix-plugin-api`, protobuf, WASM interface) | Versioned with semver. Breaking changes only in major versions |
| **Capability negotiation** | A plugin states which API version and features it supports; Logrix refuses to load incompatible ones with a clear message |
| **Conformance suite** | Every adapter, whatever the language, can be tested with `logrix plugin test`. Passing it is the bar for "official" status |
| **Deprecation** | Two minor versions of warning before removal |
| **Plugin templates** | `logrix plugin new --lang rust|ts|go|python` generates a working skeleton |

### 4.5 Extension points checklist

| You can extend... | Via | Needs rebuild? |
| :--- | :--- | :--- |
| **Queue, DB, blob store, secrets, metrics** | Rust crate or gRPC plugin | Crate: yes. gRPC: no |
| **Chain source** | Rust crate or gRPC plugin | Same |
| **Indexing logic** | WASM or gRPC | No |
| **Filters and transforms** | WASM | No |
| **Sinks (outputs)** | Rust crate, WASM or gRPC | Crate: yes. Others: no |
| **API (extra endpoints)** | Rust crate (axum routes) | Yes |
| **Hooks** | Config | No |
| **Chain presets** | YAML file | No |

### 4.6 Planning for non-EVM chains (without over-building)
Abstracting every chain type up front slows v1 and usually guesses wrong. Middle path:
- The `Source` and `Sink` boundary exchanges generic envelopes (`chain_id`, `block_ref`, `payload`, `timestamp`). The core queue, checkpoint and reorg machinery only depends on that envelope.
- EVM-specific code (logs, ABIs, topics) sits behind the `Source` and `Decode` stages.
- v1 ships only the EVM implementation. A second chain family later should mean adding a `Source` + `Decode`, not rewriting the core.
- The envelope carries a `kind` (`log`, `block`, `transaction`, `trace`, `state change`, `custom`) and a free-form `extra` map, so new data types (traces, user operations, pending transactions) need no core change. See the future-feature register in Section 16.6.

### 4.7 Customizing every resource: three levels

Every resource (queue, database, blob store, chain source, cache, secrets) can be customized at three levels. Users go only as deep as they need.

| Level | What | Needs code? | Example |
| :--- | :--- | :--- | :--- |
| **L1: Config** | Tune built-in behavior | No | Batch size, prefetch, timeouts, retry counts |
| **L2: Middleware** | Stack small layers around the resource, built-in or custom (WASM/gRPC) | Only for custom ones | Compress + encrypt messages; custom routing; custom retry; row transforms before write |
| **L3: Custom adapter** | Replace the resource entirely | Yes (Rust crate or gRPC plugin) | A new queue or database |

```yaml
queue:
  driver: sqs
  middleware: # applied in order
    - compress: { algo: zstd }
    - encrypt: { key_ref: kms://logrix-queue }
    - retry: { policy: exponential, max_attempts: 8, jitter: true }
    - plugin: { wasm: ./my-router.wasm } # custom routing logic

database:
  driver: postgres
  middleware:
    - batch: { max_rows: 5000, max_wait_ms: 200 }
    - partition: { strategy: chain_id_and_block_range }
    - plugin: { grpc: "localhost:7002" } # custom pre-write transform

blobstore:
  driver: s3
  middleware:
    - compress: { algo: zstd }
    - encrypt: { key_ref: kms://logrix-archive }
    - tiering: { hot_days: 30, cold_class: GLACIER }
```

#### What each resource's middleware can do

| Resource | Built-in middleware | Custom logic hooks |
| :--- | :--- | :--- |
| **Queue** | Compression, encryption, dedupe, rate limit, retry policy, tracing | Custom routing key / partitioner, priority function, dead-letter handler |
| **Database** | Batching, retry on failover, read-replica routing, table partitioning, slow-query logging | Pre/post-write interceptors, row transforms, custom migration steps |
| **Blob store** | Compression, client-side encryption, checksums, hot/cold tiering | Custom key layout, retention rules |
| **Chain source** | Request caching, provider selection, rate limiting, batching | Custom provider-picking strategy, response validation |

### 4.8 Pluggable policies (behavior, not just resources)

Beyond resources, the key decisions are also replaceable "policies":

| Policy | Default | Can be replaced with |
| :--- | :--- | :--- |
| **Retry and backoff** | Exponential + jitter | Custom (WASM/gRPC) |
| **Partitioning** (what keeps order) | Per contract address | Any user-defined key |
| **Scheduling** (live vs backfill) | Live first, always | Weighted, time-windowed |
| **Scaling signal** | Queue depth + lag | Custom metric |
| **Reorg handling** | Roll back from fork point | Custom (e.g. soft-delete with retention) |
| **Finality** | Chain preset | Custom confirmation rule |
| **Failure action for a plugin** | Retry then dead-letter | Skip, block, alert |

---

## 5. Pluggable layers

### 5.1 How it works
Core code depends only on traits (called ports). Each technology implements the trait as an adapter in its own crate. Config chooses the adapter at startup.

```text
Logrix core ──► Queue trait ──► logrix-queue-sqs / -rabbitmq / -redis / -memory
            ──► Store trait ──► logrix-store-postgres / -sqlite
            ──► Blob trait  ──► logrix-blob-s3 / -fs
            ──► Chain trait ──► logrix-chain-evm
            ──► Sink trait  ──► logrix-sink-db / -webhook
```

### 5.2 Queue port
Operations: `publish`, `consume` (`ack` / `retry` / `dead-letter`), `depth` (for scaling and metrics).
Logical queues: live, backfill, webhook, plus a dead-letter queue for each.

| Adapter | Priority (live first) | Dead letters | Watch out for |
| :--- | :--- | :--- | :--- |
| **SQS** (`aws-sdk-rust`) | Two queues, poll live first | Native redrive policy | Standard queues unordered; 256 KB limit |
| **RabbitMQ** (`lapin`) | Separate queues or priority queue | Native dead-letter exchange | Manual acks and prefetch tuning |
| **BullMQ** (Redis) | Native priority | Failed set, handled by us | BullMQ is a Node.js library with its own Redis scripts |
| **Memory** | Simple | Simple | Tests and `logrix dev` only |

#### BullMQ options (Q3)

| Option | Effort | Risk | Notes |
| :--- | :--- | :--- | :--- |
| **A. Redis adapter modeled on BullMQ semantics** | Low | Low | Not real BullMQ; Node apps can't share the queue |
| **B. gRPC plugin: a small Node bridge using real BullMQ** | Medium | Medium | Real BullMQ; fits the plugin model naturally |
| **C. Reimplement BullMQ's Redis protocol in Rust** | High | High | Breaks when BullMQ internals change; Rust BullMQ ports exist but their maturity must be checked first |

> **Recommendation:** A for v1. B comes almost free once the gRPC plugin system exists.

### 5.3 Database port

| Store | Holds | v1 adapters |
| :--- | :--- | :--- |
| **System store** | Checkpoints, jobs, webhook subscriptions, delivery logs | Postgres, SQLite |
| **Event store** | The user's decoded data (their tables) | Postgres, SQLite |

Every event row carries: `chain_id`, `block_number`, `block_hash`, `tx_hash`, `log_index`. Uniqueness is `(chain_id, block_hash, log_index)`, so duplicate writes are harmless.
Later (as plugins): MySQL, ClickHouse, DuckDB.

### 5.4 Blob store port
`put` / `get` / `list` / `delete`. Adapters: S3, MinIO, local disk. Holds raw blocks and logs (so you can re-index without hitting the RPC), snapshots, and large payloads.

### 5.5 Chain (RPC) port: supporting any EVM chain

#### What the port does (the full design is in Section 6)
- Multiple RPC URLs per chain with failover
- Rate limiting per provider
- Adaptive range sizing: if `eth_getLogs` says "too many results", halve the range and retry
- WebSocket for live blocks, HTTP polling as fallback

What "any EVM chain" means: Logrix works with any chain exposing the standard JSON-RPC methods (`eth_blockNumber`, `eth_getBlockByNumber`, `eth_getLogs`). Chains differ in speed, finality and limits, so:
- **Chain presets:** a built-in file of known chains (ID, block time, finality method, safe confirmations, typical limits, WS availability). Users pick a preset, override values, or define a custom chain.
- **Capability probing at startup:** for each RPC, Logrix checks WebSocket support, finalized/safe tags, max getLogs range, `eth_getBlockReceipts` availability, and archive depth. `logrix chains probe` shows the result.
- Per-chain reorg and finality settings, never hard-coded.

#### Known differences presets must cover (verify each value against chain docs before release)

| Chain family | What's different | How Logrix copes |
| :--- | :--- | :--- |
| **Ethereum L1** | Has a finalized block tag | Use the tag |
| **OP-stack L2s** (Optimism, Base) | Fast blocks, L1 finality arrives later | Preset confirmations, optional safe/finalized |
| **Arbitrum** | Very fast blocks; RPC block numbers are L2 numbers while Solidity `block.number` reports an L1 number | Index by RPC block number; document it |
| **Polygon PoS** | Historically deeper reorgs | Higher default confirmations |
| **BNB Chain and others** | Own finality and block times | Preset values |
| **Not fully EVM-equivalent chains** | RPC may behave differently | Probing flags problems; community-supported |

#### Support tiers

| Tier | Meaning | Examples |
| :--- | :--- | :--- |
| **1** | Tested in CI every release | Ethereum, one OP-stack L2, Arbitrum, Polygon (local forks) |
| **2** | Preset exists, community-verified | Other popular EVM chains |
| **3** | Custom config; works if the RPC is standard | New or private chains |

### 5.6 Other pluggable parts

| Part | v1 | Later |
| :--- | :--- | :--- |
| **Metrics** | Prometheus endpoint | OpenTelemetry, CloudWatch |
| **Secrets** | Environment variables, files | AWS Secrets Manager, Vault, Kubernetes Secrets |

### 5.7 Mixing and matching

```yaml
queue: { driver: sqs }        # sqs | rabbitmq | bullmq | memory | plugin
database: { driver: postgres } # postgres | sqlite | plugin
blobstore: { driver: s3 }     # s3 | minio | fs | plugin
```

Any combination is valid, for example local Docker workers + AWS SQS + RDS.

---

## 6. RPC layer: live indexing and backfill

### 6.1 Why this needs its own section
The RPC is the indexer's biggest cost, its most common failure source, and its main speed limit. So Logrix treats RPC as an unreliable, rate-limited, metered resource, and nothing else in the system talks to a node directly. Everything goes through one component: the RPC gateway.

### 6.2 The RPC gateway

```text
Listener (live)        Backfiller (history)        Reconciler
       │                        │                       │
       ▼                        ▼                       ▼
┌───────────────────────────────────────────────────────────┐
│                        RPC GATEWAY                        │
│  scheduler          live priority > backfill, fair sharing │
│  budgets            per provider, in the provider's cost  │
│                     units                                 │
│  provider pool      health, latency, head height,         │
│                     capabilities                          │
│  error classifier   + retry + circuit breaker             │
│  cache              + single-flight (dedupe identical     │
│                     in-flight calls)                      │
│  learned limits     per provider and chain (range caps...)│
│  usage metering     CU used, cost estimate, budget        │
│                     remaining                             │
└──────┬────────────────────────┬───────────────────┬───────┘
       ▼                        ▼                   ▼
  Provider A               Provider B       Own node / bulk source
```

Callers only say "give me logs for this range" or "give me new heads". The gateway decides which provider, how fast, and what to do on failure. This is what lets you add providers, change pricing, or plug in a bulk data source without touching the rest of the indexer.

### 6.3 Cost model
Most providers bill in compute units (CU), with different weights per method. A `getLogs` call costs far more than `eth_blockNumber`. Prices, free tiers and weights change often and differ between sources, so Logrix never hard-codes them: they live in provider profiles (Section 6.7) and are verified against live pricing pages.
Throughput caps (CU per second) usually limit a backfill before money does.

#### Illustrative live-indexing estimates
*(Assumptions: about 150 CU per block for a `getLogs` call plus a header check; $0.525 per million CU; verify block times and prices before relying on them)*

| Chain | Blocks per month (approx.) | Cost, one call set per block | With batching (e.g. 20 blocks per call) |
| :--- | :--- | :--- | :--- |
| **Ethereum (12 s)** | ~216k | ~$17 | Not needed |
| **Base / Polygon-type (2 s)** | ~1.3M | ~$100 | ~5x to 20x less |
| **Arbitrum (~0.25 s)** | ~10M | ~$800 | ~20x less on the log calls |

#### Illustrative backfill estimates (Ethereum, three years, about 7.9M blocks)

| Scenario | Calls | Cost (approx.) | Time on a 330 CU/s free-tier cap |
| :--- | :--- | :--- | :--- |
| **Filtered `getLogs`, 10,000-block windows** | ~800 | Pennies | Seconds |
| **Same, plus one timestamp call per block (~20 CU each)** | ~7.9M | ~$80 | ~5 to 6 days |
| **Broad filter ("all Transfers"), many splits** | Many thousands | Varies | Long |

#### Cost features Logrix provides
- `logrix backfill --dry-run`: estimates calls, cost units, money and time before starting
- Per-provider monthly budgets with alerts (for example at 80%) and a hard stop option
- A usage meter in the gateway, so cost is visible per provider, per chain, per project

### 6.4 Public vs paid RPC: guidance

| Situation | Recommendation |
| :--- | :--- |
| **Learning and development** | Public endpoint + one free-tier key |
| **Small live indexer** (one slow chain, few contracts) | Two or three free-tier keys, public endpoints as last-resort fallback |
| **Production live indexing** | Paid plan + a second provider for failover |
| **Large backfill** | Bulk historical provider or your own node; always keep the raw archive |
| **Many or very fast chains** | Batched fetching, filtered log subscriptions, own node or dedicated plan |

> **Observed with public endpoints (2026 probes; varies by provider and over time):** `getLogs` range caps from roughly 1,000 to 10,000 blocks, result-count caps, some serving only recent blocks, per-IP rate limits (shared cloud IPs make this worse), occasional "usage limit" errors, and no uptime guarantee.

### 6.5 Live indexing design
- Head tracking uses two signals. WebSocket `newHeads` is the fast path; a slow HTTP poll (about half a block time) is the watchdog. WebSocket pushes are hints, not truth.
- Fetch logs by block hash (`blockHash` parameter), not by number, so you always get logs of the exact block you saw even if a reorg lands a moment later.
- Validate as you go. Check `eth_chainId` at startup; keep recent headers; compare each new `parentHash` to the stored hash. Mismatch means a reorg: find the fork point, tell the Controller, re-process.
- Gap-fill automatically. After a WebSocket reconnect or a missed head, compute the gap and send it down the backfill path with live priority.
- Two finality modes (user choice per chain): fast (process the tip, mark rows unconfirmed, finalize later) or safe (wait N confirmations or the chain's finalized tag).
- Fast chains use batching. Group several blocks per `getLogs` call, sized to a target latency. Optionally use a filtered `eth_subscribe logs` subscription. Check how the provider bills WebSocket subscriptions.
- Dedicate capacity. Live traffic can have providers (or quota slices) reserved for it, so backfill can never starve it.

### 6.6 Backfill design
- **Plan first:** Start at the contract's deployment block (or a user value); end at the finalized head; live takes over after that.
- **Adaptive chunking:** Start from the preset window. On "range too large" or "too many results", halve and retry; on success with sparse results, grow slowly. Persist the learned limit per provider and chain. Parse suggested ranges from error text when providers include them.
- **Fetch in parallel, commit in order:** Workers fetch ranges out of order; a reorder buffer feeds the decoder in order per partition; checkpoint after each range so a crash resumes mid-way.
- **Pick the best source available, in this order:** local raw archive → bulk historical provider → own node → RPC provider pool.
- **Reserve capacity for live:** For example 30% of each provider's budget for live, 70% for backfill; backfill pauses entirely if live falls behind.
- **Store raw logs before decoding:** Re-indexing then costs zero RPC calls (D26).
- **Avoid per-block extras:** Timestamps and receipts are the usual hidden cost; batch them or take them from data you already have.

### 6.7 Provider profiles (data, not code)
Each provider is described by a YAML profile in `presets/providers/`. Community members can contribute profiles without touching Rust. Example (values are illustrative placeholders; real profiles must be verified against the provider's docs):

```yaml
provider: example-provider
billing: compute_units
cost_weights: # per-method cost units (verify against live docs)
  eth_blockNumber: 10
  eth_getLogs: 60
  eth_getBlockByNumber: 20
throughput: { units_per_second: 300 }
limits:
  getlogs: { max_block_range: 10000, max_results: 10000 }
  batch: { max_items: 100, billed_per_item: true }
websocket:
  subscriptions: [newHeads, logs]
  idle_disconnect_seconds: 3600
errors: # table-driven classification (Section 10.8)
  - { match: { http: 429 }, class: rate_limited }
  - { match: { message_regex: "range|too many results" }, class: shrinkable }
  - { match: { http: 401 }, class: fatal }
```

Capability probing (Section 5.5) fills in what a profile doesn't know and checks what it claims.

### 6.8 Source traits (so bulk providers and own nodes plug in)

| Trait | Purpose | Implementations |
| :--- | :--- | :--- |
| **LiveSource** | Heads and new logs | JSON-RPC (WebSocket + polling), own node, Reth ExEx, streaming providers |
| **HistoricalSource** | Ranges of logs/blocks | JSON-RPC pool, local raw archive, bulk providers (SQD Portal, HyperSync, Substreams/Firehose), own node |

The gateway sits behind the JSON-RPC implementations; other sources plug in as Rust crates or gRPC plugins and are chosen by the backfill source priority in 6.6.

### 6.9 Verification mode (optional)
Providers sometimes return incomplete logs. Opt-in checks: compare receipts to the header's `receiptsRoot`; cross-check a second provider for the same range (quorum); verify block hash continuity. Slower and costlier, so off by default (Q12).

### 6.10 Best-practices checklist

| # | Practice | Why |
| :--- | :--- | :--- |
| **1** | Always filter server-side (address + topics) | Smaller responses, lower cost |
| **2** | Budget in cost units, not request count | Methods differ enormously in weight |
| **3** | Classify errors: retryable, shrinkable, permanent | Wrong retries waste money and time |
| **4** | Make error matching table-driven per provider | Message text differs between providers |
| **5** | Retry budget, backoff and jitter | Prevents retry storms |
| **6** | Treat "block not found" and null as normal | Load-balanced nodes can lag behind each other |
| **7** | Use explicit block numbers in backfill, never `latest` | Reproducible results |
| **8** | Single-flight (dedupe) and caching | Several workers often ask for the same thing |
| **9** | No per-block calls just for timestamps | Batch them or reuse data |
| **10** | JSON-RPC batching only where the provider allows and bills sensibly | Many providers cap or bill per item |
| **11** | WebSocket: ping, reconnect, resubscribe, then gap-fill | Dropped connections are normal |
| **12** | Per-provider metrics: latency, error rate, cost used, blocks behind best head | Basis for routing and alerts |
| **13** | Never log RPC URLs (they contain keys) | Secret leakage |
| **14** | Test with a fake RPC that injects failures | Can't test failures on real providers |
| **15** | Dry run with a cost estimate before any big backfill | No surprise bills or day-long runs |

### 6.11 Code layout (inside logrix-chain-evm)
`pool` (providers, health, capability probing) · `scheduler` (priorities, budgets, cost model) · `limits` (learned caps, persisted) · `head` (WebSocket + polling + reorg detection) · `planner` (range planning, adaptive chunks, reorder buffer) · `profiles` (provider profile loader) · `metering` (usage and cost events) · `testkit-fake-rpc` (fault-injecting fake node).

### 6.12 Build order inside the RPC layer
Each step leaves something that works:
1. One provider, fixed-size chunked `getLogs`, polling for new blocks
2. Error classification and adaptive window sizing
3. Provider pool with failover and provider profiles
4. Scheduler with live priority, budgets and the cost estimator
5. WebSocket, gap-fill and reorg handling
6. Verification mode and bulk sources (as plugins)

Steps 1 and 2 already index Ethereum on a free-tier key. Steps 3 to 6 make it production-grade.

---

## 7. APIs and client languages

### 7.1 Your question: "what language does the client use if it's GraphQL?"
Short answer: any language. GraphQL is not tied to a programming language. It is a query language that travels over normal HTTP. A client sends a text query and gets back JSON. Anything that can make an HTTP request can use it.

```text
Client (any language)                       Logrix API
        │                                       │
        │   POST /graphql                       │
        │───► runs the query on the DB ────────►│
        │     { "query": "{ swaps(first: 10) {  │
        │       pool amount0 } }" }             │
        │                                       │
        │◄─── returns JSON ─────────────────────│
```

| Client | How it connects |
| :--- | :--- |
| **JavaScript / TypeScript (React, Next.js, Node)** | Apollo Client, urql, graphql-request, or plain `fetch` |
| **Python** | gql, requests |
| **Rust** | graphql_client, cynic, or reqwest |
| **Go, Java, Kotlin, Swift, C#...** | Their GraphQL libraries, or any HTTP client |
| **Terminal** | curl |

Logrix does not force a client language. Users can also generate typed clients from the schema in their language (for example with GraphQL Code Generator for TypeScript).

### 7.2 Two different "languages" (they are easy to mix up)

| Meaning | Who chooses | Options |
| :--- | :--- | :--- |
| **Client language** | The app developer | The language of the app that reads indexed data (anything, see above) |
| **Handler language** | The Logrix user | The language used to write indexing logic that runs inside Logrix (Any language that compiles to WASM, TypeScript, or a remote service in any language) |
| **Logrix's own language** | Us | What the engine is built in: Rust |

So Rust is only the engine's language. Nobody using Logrix has to know Rust.

### 7.3 API styles

| API | v1? | What it's for |
| :--- | :--- | :--- |
| **GraphQL** (auto-generated, with filtering, sorting, pagination) | Yes | Main query API for apps and dApps |
| **GraphQL subscriptions** (over WebSocket) | Yes | Live updates pushed to clients |
| **REST** (admin and health: `/health`, `/status`, `/metrics`) | Yes | Operations and monitoring |
| **Webhooks** | Yes | Push events to your server |
| **gRPC** (streaming events) | Later | Backend-to-backend, high throughput |
| **Direct SQL access** to the event database | Always possible | Analytics, BI tools, existing SQL skills |

### 7.4 Rust implementation notes
- **HTTP server:** `axum`
- **GraphQL:** `async-graphql` with its dynamic schema feature, which builds the schema at startup from the user's `schema.yaml` (the schema isn't known at compile time)
- **Query safety:** depth and complexity limits, per-key rate limits

---

## 8. User configuration and indexing logic

### 8.1 What a user writes: logrix.yaml

```yaml
project: my-dex
chains:
  - id: 1
    name: ethereum
    preset: ethereum # optional: loads known finality + RPC limits
    rpc:
      http: [${ALCHEMY_URL}, ${INFURA_URL}] # failover list
      ws: [${ALCHEMY_WS}]
    confirmations: 12 # overrides the preset
    start_block: 17000000

contracts:
  - name: Pool
    chain: ethereum
    address: ["0xabc...", "0xdef..."]
    abi: ./abis/Pool.json
    events: [Swap, Mint, Burn]

handlers:
  - event: Pool.Swap
    mode: declarative
    table: swaps
    map:
      pool: log.address
      amount0: args.amount0
      sender: args.sender
```

### 8.2 Four levels of indexing logic

| Level | What the user does | Good for |
| :--- | :--- | :--- |
| **1. Declarative** | Map event fields to columns in YAML | Most use cases, no code |
| **2. WASM** | Compile a function (`on_event`) from any language to WASM. It gets `ctx.db`, `ctx.read_contract()`, `ctx.emit()` through a defined host interface. Sandboxed | Balances, aggregates, derived data, in the user's preferred language |
| **3. TypeScript** | Write TypeScript; Logrix bundles it and runs it inside a JS-in-WASM runtime (same sandbox as level 2) | Easiest path for web developers |
| **4. Remote** | Logrix sends decoded events to the user's own service over gRPC/HTTP and applies the returned changes | Existing backends, any language, heavy logic |

### 8.3 Schema management
- Users declare tables in `schema.yaml`
- Logrix generates migrations and the GraphQL schema automatically
- Adding columns or tables is automatic. Breaking changes require `logrix reindex`, which rebuilds from the raw archive

### 8.4 CLI

| Command | Purpose |
| :--- | :--- |
| `logrix init` | Scaffold a project |
| `logrix validate` | Check config, ABI, schema |
| `logrix dev` | Run everything locally in one process |
| `logrix start --role=...` | Run one role in production |
| `logrix backfill` / `logrix reindex` | Historical work |
| `logrix status` | Current block, head, lag, queue depth |
| `logrix chains probe` | Test an RPC and show what it supports |
| `logrix dlq list / replay / purge` | Handle failed jobs |
| `logrix plugin new / test / list` | Create, test and inspect plugins |

### 8.5 Later features
Factory contract discovery (auto-track new pools), proxy/ABI upgrades by block range, topic filtering, multiple projects per deployment.

---

## 9. Correctness rules

| Problem | Rule |
| :--- | :--- |
| **Duplicate messages** | Idempotent writes (unique key `chain_id` + `block_hash` + `log_index`) |
| **Crash mid-job** | Events and checkpoint in one DB transaction |
| **Reorg** | Keep recent block hashes. On mismatch: find fork point → Controller rolls back → re-queue. Webhooks get `event.reverted`. API exposes finalized vs unconfirmed |
| **Ordering with many decoders** | Stateless inserts run fully parallel. Stateful handlers (levels 2 to 4) are ordered per partition (default: per contract address) |
| **Bad message** | Retry with backoff and jitter → dead-letter queue after N tries → alert → `logrix dlq replay` |
| **Backfill meets live** | Stateful handlers run backfill in order; live is held until backfill reaches the head |
| **Plugin misbehaves** | WASM: hard CPU/memory/time limits, kill and report. gRPC: timeout and circuit breaker. A failing plugin never corrupts the DB (transaction rolls back) |
| **Shutdown** | Finish the current job, hand back the rest |
| **Webhook failure** | Signed payload, retry with backoff, circuit breaker, delivery log, manual replay |

> Ordering is the hardest design problem. I'll write a short design note for it before any decoder code.

---

## 10. Resilience and failure handling

### 10.1 Principles
- Assume everything fails, including the network, the RPC, the queue, the database and our own plugins.
- Bound everything: queue sizes, memory, retries, time per call, concurrency. Unbounded means "will fall over eventually".
- Never lose data silently and never corrupt data. If unsure, stop and alert.
- Degrade gracefully: a failing part should reduce service, not stop everything.
- Self-heal: the system detects and fixes gaps by itself (reconciler, Section 10.6).

### 10.2 One resilience layer for every external call
Every call to an outside system (RPC, queue, DB, blob store, plugin, webhook endpoint) passes through the same layers, implemented once in `logrix-resilience` (built on the `tower` ecosystem: timeouts, retries, rate limits, concurrency limits, load shedding).

| Tool | What it does | Applied how |
| :--- | :--- | :--- |
| **Timeouts** | Connect, request and total-deadline limits | Always set; no call can hang forever |
| **Retries** | Exponential backoff with jitter | Only for idempotent operations; with a retry budget (cap on retries per minute) so retries can't cause a storm |
| **Circuit breaker** | Stops calling a failing dependency; tries again carefully after a pause (`closed` → `open` → `half-open`) | Per dependency |
| **Bulkheads** | Separate connection pools / concurrency limits per dependency | A slow webhook can't starve the DB writer |
| **Rate limiting** | Token bucket per provider or endpoint, honors `Retry-After` | RPC providers, webhook endpoints |
| **Load shedding** | Reject new work early when overloaded (429/503) | API, admin endpoints |
| **Cancellation** | Graceful stop through cancellation tokens | Shutdown, config reload |

### 10.3 Backpressure: how slowness travels backwards

Backpressure means a slow part tells the parts before it to slow down, instead of letting work pile up until memory or disk runs out.

```text
RPC  ─►  Listener  ─►  Queue  ─►  Decoder  ─►  Database
 ▲          ▲            ▲           ▲             │
 │          └────────────┴───────────┴─────────────┘
 └───────────────── pressure signals flow this way
```

| Pressure point | Signal | Response |
| :--- | :--- | :--- |
| **Database slow** | Write latency up, transaction queue growing | Decoders reduce concurrency and prefetch automatically (AIMD: add slowly, cut fast, like TCP), batch sizes adapt |
| **Backfill queue large** | Depth above the high watermark | Backfiller pauses publishing; resumes below the low watermark |
| **Live queue growing** | Live lag above threshold | Decoders stop taking backfill jobs entirely until live is caught up |
| **RPC rate-limited** | 429 responses | Token bucket shrinks, load spreads across providers, adaptive range sizes |
| **In-process buffers** | Bounded channels full | Producers wait; nothing grows without limit |
| **Webhook endpoint slow** | Delivery latency, failures | Per-endpoint queues and limits; one bad endpoint never blocks others |
| **API overloaded** | In-flight queries over the limit | Reject with 429/503, query timeouts, complexity limits |
| **Memory high** | Per-process cap reached | Split jobs, shrink batches, then pause consumption |

- **Rule:** live blocks are never dropped or delayed by backfill. Priority is enforced at the consumer, not just by having separate queues.
- **Autoscaling must respect downstream limits.** KEDA could scale decoders to 50 and overwhelm the database. Logrix uses a concurrency budget: the maximum total concurrent DB writers is configured, and each decoder takes a share, so scaling out never exceeds what the database can take.

### 10.4 Failure matrix

| # | Failure | How it's detected | Response | Data impact |
| :--- | :--- | :--- | :--- | :--- |
| **1** | RPC down or slow | Health checks, timeouts | Fail over to the next provider, backoff, breaker | None; lag alert |
| **2** | RPC returns stale or inconsistent data | Parent-hash validation; optional cross-check with a second provider | Reject the data, switch provider | None |
| **3** | RPC rate limit (429) | Status code, `Retry-After` | Token bucket, spread load | None; slower |
| **4** | WebSocket drops | Missed heartbeat | Reconnect with backoff, fall back to polling, gap-fill the missed range | None |
| **5** | Network partition to queue | Publish/consume errors | Retry with budget, breaker; optional bounded local spool on the listener | None (spool or gap-fill) |
| **6** | Queue loses or drops a message | Reconciler finds a gap between checkpoints and chain head | Re-enqueue the missing range | None |
| **7** | Duplicate delivery | Unique key conflict | Idempotent write does nothing | None |
| **8** | Poison message | Repeated failures | Retry N times → dead-letter queue → alert | One job parked, others continue |
| **9** | Database down | Connection errors | Decoders stop acking; messages return to the queue; breaker; resume automatically | None |
| **10** | Database slow | Latency metrics | Backpressure (Section 10.3) | None; lag grows |
| **11** | Database failover | Connection reset | Reconnect, retry the whole transaction (idempotent) | None |
| **12** | Database disk full | Write errors, alerts | Pause writes, do not ack, page an operator | None; stalled |
| **13** | Blob store down | Errors | Per-config: required (fail job, retry) or best-effort (queue an async retry for the archive) | None, or a delayed archive |
| **14** | Decoder crash mid-job | Unacked message | Redelivered after the visibility timeout; transaction was atomic | None |
| **15** | Decoder out of memory (huge block) | Process killed | Job splits into smaller ranges; memory limits | None |
| **16** | WASM plugin crash, loop or memory bomb | Trap, limit exceeded | Kill the instance, retry, then dead-letter; quarantine the plugin version | Job parked |
| **17** | gRPC plugin down | Timeout, breaker open | Per-plugin policy: block, skip or dead-letter | Depends on policy |
| **18** | Webhook endpoint down | Timeouts, 5xx | Retry with backoff for a configurable window, circuit breaker, replay later | None |
| **19** | Bad config | Validation at startup and on reload | Refuse the change, keep running the old config | None |
| **20** | Clock skew | n/a | Never use wall-clock time for ordering; use block numbers | None |
| **21** | Rolling deployment mid-flight | n/a | Graceful shutdown; versioned messages; N-1 compatibility | None |
| **22** | AZ outage | Cloud health, node loss | Multi-AZ managed services; pods reschedule | None |
| **23** | Region outage | Cloud status | DR plan (restore from backups + raw archive + re-index) | Recovery time, no permanent loss |
| **24** | Bug in user logic | Wrong data noticed | `logrix reindex` from the raw archive | Rebuildable |
| **25** | Operator mistake (wrong command) | n/a | Dry-run mode, confirmation prompts, snapshots before destructive actions | Reversible |

### 10.5 What Logrix promises

| Promise | Meaning |
| :--- | :--- |
| **No silent data loss** | Any missing range is detected and refilled, or an alert fires |
| **At-least-once processing, effectively-once results** | Duplicates are harmless |
| **Ordering per partition** | Not global ordering (it doesn't scale) |
| **Bounded lag under normal conditions** | And visible, with alerts, when exceeded |
| **Rebuildable state** | The raw archive and checkpoints let you re-derive everything |

### 10.6 Self-healing: the reconciler
The Controller runs a reconciler loop:
1. Compare the checkpoint ranges in the DB with the chain head and the expected range.
2. For any gap (lost message, dropped job, partial failure): re-enqueue that range.
3. Check for stuck jobs (in flight too long): re-enqueue them.
4. Verify block hash continuity across stored blocks to catch silent corruption.
5. Emit metrics and alerts for everything it fixed.

This is why no single queue, plugin or process failure can create permanent holes in the data.

### 10.7 Degraded modes

| Mode | When | What still works |
| :--- | :--- | :--- |
| **Live-only** | Backfill overloads the system | Live indexing continues, backfill paused |
| **Read-only** | Writes are failing (DB problem) | API serves existing data; ingestion paused safely |
| **Catch-up** | After an outage | Backfill-style fast processing to the head, then live |

### 10.8 Error classes (classify once, at the edge)
Every adapter (RPC, queue, DB, blob, plugin) converts its own errors into one shared type carrying a class. Everything above looks only at the class, never at provider-specific text. The message matching lives in provider profiles and adapter code.

| Class | Meaning | Default action |
| :--- | :--- | :--- |
| **Transient** | Timeout, connection reset, 5xx, node briefly behind | Retry with exponential backoff and jitter, within a retry budget |
| **Rate-limited** | 429, "usage limit reached" | Honor Retry-After, slow that provider, shift load to others |
| **Shrinkable** | Range too large, too many results, response too big | Halve the work and retry; remember the learned limit |
| **Permanent** (this job) | Bad data for one job: undecodable event, handler bug | No endless retry; park in the DLQ with full context |
| **Integrity** | Hash mismatch, missing logs, reorg during a write | Stop that partition, reconcile or roll back, then resume |
| **Fatal** | Wrong chain ID, invalid config, revoked credentials | Stop that component, alert loudly, never retry forever |

```rust
enum ErrorClass {
    Transient,
    RateLimited { retry_after: Option<Duration> },
    Shrinkable,
    Permanent,
    Integrity,
    Fatal,
}

struct LogrixError {
    class: ErrorClass,
    source: Source,
    context: Context,
}
```

Every error also gets: a metric, a structured log with context (chain, block range, provider, job ID), and an alert level. An error without these is a bug.

### 10.9 Handling by area

#### RPC "can't fetch"

| Symptom | Likely cause | Action |
| :--- | :--- | :--- |
| **Timeout / connection reset** | Provider overloaded, network blip | Retry with backoff, fail over, breaker opens if it persists |
| **429 / usage limit** | Quota or throughput cap | Pause that provider, honor `Retry-After`, route elsewhere, reduce backfill concurrency |
| **Range too large / result cap** | Window too big | Halve, retry, store the new limit |
| **"Block not found" / null** | Node behind, or block reorged | Try another provider; if all agree, wait briefly and re-check the head |
| **Providers disagree on a hash** | Stale node or fork | Prefer the majority/highest head; never trust one source blindly |
| **WebSocket silent or closed** | Dropped connection | Fall back to polling, reconnect, resubscribe, gap-fill |
| **401 / 403** | Key expired or revoked | Fatal for that provider: disable, alert, keep using the others |
| **All providers failing** | Real outage or local network | Degraded mode: keep retrying slowly, alert, never skip blocks; refill when RPC returns |

#### Queue full or unable to process

| Side | Situation | Action |
| :--- | :--- | :--- |
| **Publishing** | Queue full or unreachable | Never drop the message. Listener writes to a bounded local spool and retries. If the spool fills, stop pulling new blocks (backpressure) and alert; the reconciler can refill any gap, so pausing is safe |
| **Publishing** | Backfill queue above high watermark | Backfiller pauses until the low watermark |
| **Consuming** | Decoders slow because the DB is slow | Reduce concurrency (don't add workers) |
| **Consuming** | Decoders CPU-bound | Scale out within the DB concurrency budget |
| **Consuming** | Backfill competing with live | Stop taking backfill jobs |
| **Consuming** | One huge or poison message | Detect repeated redelivery, split or DLQ |

> **Queue-specific traps:** RabbitMQ blocks publishers under memory/disk alarms; SQS has in-flight limits; Redis eviction policies can silently delete jobs, so Logrix requires `noeviction` on any Redis used as a queue and checks it at startup.

#### Live delay (lag)
Measure lag two ways: blocks behind head, and seconds since the newest indexed block's timestamp. Escalation ladder:
- **Normal:** Nothing
- **Threshold 1:** Pause backfill; live gets everything
- **Threshold 2:** Scale decoders (within the DB budget); switch to catch-up mode
- **Threshold 3:** Alert a human; API reports the data as stale

Catch-up mode treats the live gap like a backfill range: bigger batches, bulk writes, optional summary instead of per-block webhooks.

Stage timestamps are recorded at every step (block time, seen by listener, queued, decoded, written), so you can see at a glance whether delay comes from RPC, queue, decoder or DB.

Freshness is exposed in the API (`indexed_through_block`, `lag_seconds`), so apps can show "data may be 20 s behind" instead of silently serving stale data.

#### Other errors

| Error | Handling |
| :--- | :--- |
| **DB deadlock / serialization failure** | Retry the whole transaction (idempotent) |
| **Unique-key conflict** | Expected on redelivery; treat as success |
| **DB connection pool exhausted** | Backpressure and shorter queries; never open more connections |
| **DB disk full** | Stop writing, don't ack, page someone |
| **Unknown or undecodable event** (ABI mismatch, proxy upgrade) | Store the raw log in a "failed decodes" table, continue, alert; reprocess later with a fixed ABI |
| **Handler or plugin crash / timeout** | Retry a few times, then park; quarantine a plugin version that keeps failing |
| **Huge block runs out of memory** | Split into smaller ranges and retry |
| **Reorg arrives while writing** | Transaction compares block hashes; mismatch means discard and re-process |
| **Provider returned incomplete logs** | Optional verification (Section 6.9); reconciler gap checks |
| **Webhook endpoint down** | Per-endpoint retry window, breaker, replay later |
| **Blob store down** | Configurable: required (job fails and retries) or best-effort (archive later) |
| **Bad config on reload** | Reject the change, keep the old config |
| **Migration fails** | Roll back, refuse to start the new version |

### 10.10 Park and continue, or halt?
When a job fails permanently, the right choice depends on the handler:

| Handler type | Policy | Reason |
| :--- | :--- | :--- |
| **Stateless** (plain inserts) | `park`: DLQ it, record a gap marker, continue; replay or reconcile later | Other data is unaffected |
| **Stateful** (running balances, totals) | `halt`: stop that partition, alert, resume after a fix; other partitions keep running | Skipping a block leaves state wrong forever |

This is a per-handler setting (`on_failure: park | halt`), defaulting to `halt` for stateful handlers.

### 10.11 Error-handling rules
- Never ack before the database commit.
- Retry only idempotent operations, with a retry budget.
- Never swallow an error silently. If something is skipped, record it somewhere queryable.
- Retry caps are finite (for example 5 to 8 attempts over a few minutes, then DLQ); fatal errors don't retry.
- Prefer pausing to losing data. Pausing is safe because the reconciler refills gaps.
- Alert on symptoms (lag, DLQ depth, gaps found), not on every retry.

### 10.12 Testing error handling
A fault-injecting fake RPC and fake queue can return 429s, timeouts, range errors, stale heads, WebSocket drops, reorgs, full queues and duplicate messages on command. There is one automated test per row of the tables above, plus chaos runs combining several faults (Section 18).

---

## 11. Scalability

### 11.1 What can grow, and what scales it

| Growth dimension | Bottleneck | How Logrix scales |
| :--- | :--- | :--- |
| **More chains** | Listener instances, RPC capacity | One listener/backfiller per chain; chains assigned across instances |
| **More contracts and events** | Decode CPU, DB writes | More decoder replicas; batching; partitioned tables |
| **Higher event rate** | DB write throughput | Batched inserts, partitioning, then sharding or a column store |
| **Big backfills** | RPC rate limits, DB | Parallel chunks, provider pools, reuse of the raw archive |
| **More API traffic** | DB reads | Stateless API replicas, read replicas, caching |
| **Many webhooks** | Delivery workers | Webhook worker replicas, per-endpoint isolation |
| **More projects** | Overall resources | Per-project partitions and resource budgets |

### 11.2 Scaling each role

| Role | Type | How it scales |
| :--- | :--- | :--- |
| **Decoder, API, webhook worker** | Stateless | Add replicas (KEDA / HPA) |
| **Listener, backfiller, controller** | Stateful singletons per chain | Leader election (Postgres advisory lock or Kubernetes lease) with hot standby; more chains → more instances, assigned by a consistent rule |

### 11.3 Partitioning: the foundation of scaling
One partition key concept is used everywhere, so ordering and distribution stay consistent:

| Where | Partitioned by |
| :--- | :--- |
| **Queue messages** | Message group / routing key = partition key |
| **Consumers** | Each partition handled by one consumer at a time (ordered handlers) |
| **Database tables** | `chain_id`, then block range (time-based partitions: easy to drop, archive, and query) |
| **Blob store** | Key prefix `chain/<id>/<block-range>/` |

Default partition key: contract address. Users can supply their own (Section 4.8).

### 11.4 Database scaling ladder

| Stage | Setup | Triggered when |
| :--- | :--- | :--- |
| **1** | Single Postgres; batched inserts (COPY or multi-row); connection pooler | Start |
| **2** | Table partitioning by chain and block range; tuned indexes; read replica for the API | Write or query load grows |
| **3** | Separate event database per chain or per project | Chains/projects interfere |
| **4** | Column store for analytics (ClickHouse plugin); Postgres stays for operational data | Heavy aggregate queries |
| **5** | Sharding or managed distributed SQL (as an adapter) | Single-node limits reached |

Because the DB is behind a trait with middleware (Section 4.7), each stage is a configuration or adapter change, not a rewrite.

### 11.5 The RPC is usually the real bottleneck
*(Full design: Section 6)*

| Technique | Benefit |
| :--- | :--- |
| **Pool of several providers**, weighted by speed and quota | Higher total throughput, failover |
| **JSON-RPC batching** | Fewer round trips |
| **Larger getLogs ranges** with adaptive sizing | Fewer calls |
| **Reuse the raw archive** for re-indexing | Zero RPC calls for reprocessing |
| **Cache of recent blocks** | Fewer duplicate calls across decoders |
| **Own node or a high-throughput data provider** (as a Source plugin) | Removes provider limits |

### 11.6 Queue and API scaling
- **Queue:** SQS scales essentially without limit; RabbitMQ uses quorum queues and clustering; Redis uses cluster mode. Job size (blocks per message) adapts to keep messages in a healthy range.
- **API:** Stateless replicas, read replicas, a pluggable cache port (in-memory or Redis), query cost limits, persisted queries, subscriptions fanned out through a pub/sub channel.

### 11.7 Autoscaling architecture (targeted scaling per role)

> **Detailed Blueprint:** See [`KUBERNETES_PLAN.md`](file:///home/vishal/Projects/logrix/KUBERNETES_PLAN.md) for full Kubernetes manifests, Karpenter NodePool specs, and production deployment templates.

#### 11.7.1 Core principle: use each autoscaler for what it does best
Instead of forcing one autoscaler everywhere:
- **KEDA:** For queue-driven workers (Decoders, Webhook workers); enables scale-to-zero and rich event triggers.
- **Plain HPA (Built-in Kubernetes):** For the API tier, scaling on CPU, memory, and HTTP request latency.
- **Node Autoscaler (Karpenter / Cluster Autoscaler):** Underneath all pods to dynamically provision right-sized nodes (Spot for backfill, On-Demand for live/stateful).
- **No Autoscaling (Singletons):** For Listener and Controller, using leader election and a hot standby instead of pod scaling.
- **VPA:** Recommendation-only mode to tune requests/limits without competing with HPA.

*Cardinal Rules:*
1. **Never** attach a KEDA `ScaledObject` and an independent HPA to the same Deployment.
2. **Never** run VPA and HPA on the same metric.

#### 11.7.2 Recommended setup per role

| Role | Scaler | Scale on | Min | Max | Scale to Zero? | Node Type |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Decoder (live)** | KEDA or Fixed | Head lag (blocks behind head), message age | 2 | Small (4–6) | No | On-Demand |
| **Decoder (backfill)** | KEDA | Controller metric: `logrix_desired_replicas` (or queue depth) | 0 | Capped by DB write budget & RPC limit (e.g. 50) | **Yes** | Spot / Preemptible |
| **Webhook worker** | KEDA | Webhook queue depth | 1 | Per-endpoint limits | Optional | On-Demand / Spot |
| **API** | Built-in HPA | CPU (>65%) and request latency | 2 | Capped by DB read capacity | No | On-Demand |
| **Listener, Controller, Planner** | **None** | n/a (Stateful singletons per chain) | 2 (1 active + 1 standby) | 2 | No | On-Demand |
| **gRPC Plugins** | HPA or follows Decoder | CPU / Memory | Follows host | Follows host | Follows host | Same as host |
| **Nodes** | Karpenter / CA | Pending pod capacity requests | Cluster min | Cloud quota | n/a | Mixed |

#### 11.7.3 The critical rule: don't scale on queue depth alone
If a queue has 100k messages because the DB or RPC is throttling, a naive queue-depth scaler will add pods until the DB suffers a total outage. Logrix prevents this with a **2-layer control design**:

```text
Layer 1 (Inside Pod - Fast): Adaptive Concurrency (AIMD) backs off local workers on DB/RPC latency
Layer 2 (Across Pods - Slow): Controller computes budget-capped desired replicas for KEDA
```

The Controller calculates and publishes:
$$\text{logrix\_desired\_replicas} = \min \left( \text{replicas}_{\text{queue\_depth}}, \frac{\text{DB write budget}}{\text{writers per pod}}, \frac{\text{RPC capacity}}{\text{throughput per pod}} \right)$$

KEDA scales Decoders on this metric, ensuring the cluster never scales into an outage.

#### 11.7.4 Native trigger vs Logrix Prometheus metric
- **Native KEDA Scaler (SQS, RabbitMQ, Redis):** Simple and independent of Prometheus, but unaware of DB/RPC limits.
- **Logrix Prometheus Scaler (`logrix_desired_replicas`):** Works universally across all queues and plugins, strictly honoring DB concurrency and RPC budgets. (Recommended for production backfill).

#### 11.7.5 Outside Kubernetes
- **Docker Compose / Local:** Static replica count (`--scale decoder=4`) + in-pod adaptive concurrency.
- **AWS ECS:** Application Auto Scaling on SQS backlog per task.
- **Azure Container Apps:** Built-in KEDA running identical `ScaledObject` trigger definitions.
- **Nomad / Cloud Run:** Scrapes Prometheus `/metrics` for `logrix_desired_replicas`.


### 11.8 Scaling ladder (the whole system)

| Stage | Environment | What changes |
| :--- | :--- | :--- |
| **0** | Laptop: `logrix dev` | Single process, memory queue, SQLite |
| **1** | One machine: Docker Compose | Real Postgres and queue |
| **2** | Small Kubernetes cluster | Role separation, KEDA, managed DB |
| **3** | Many chains, high volume | Partitioned DB, read replicas, provider pools, tuned budgets |
| **4** | Multi-region / DR | Replicated storage, regional failover, restore runbooks |

### 11.9 Performance engineering
- Benchmarks (`criterion`) in CI; regressions fail the build
- Targets set in Phase 0 from your scale answer (Q6), then checked every release
- Profiling guides (flamegraphs); zero-copy decoding where possible; batching as the default write path
- Load-test harness replays recorded chain data at multiples of real speed

---

## 12. Multi-cloud and local portability

### 12.1 Principle
The core knows only traits. Everything cloud-specific lives in cloud packs: a set of adapters, a Terraform module, and Helm values per provider. Local and cloud use the same binary.

### 12.2 Equivalent services per environment

| Capability | AWS | GCP | Azure | Anywhere / local |
| :--- | :--- | :--- | :--- | :--- |
| **Queue** | SQS | Pub/Sub | Service Bus | RabbitMQ, Redis, NATS/Kafka (plugins) |
| **Database** | RDS / Aurora Postgres | Cloud SQL / AlloyDB | Azure Database for PostgreSQL | Self-hosted Postgres, SQLite |
| **Blob** | S3 | GCS | Azure Blob | MinIO and any S3-compatible store (Cloudflare R2, DigitalOcean Spaces, Backblaze), local disk |
| **Secrets** | Secrets Manager | Secret Manager | Key Vault | Vault, Kubernetes Secrets, env |
| **Kubernetes** | EKS | GKE | AKS | DOKS, k3s, kind, on-prem |
| **Metrics** | CloudWatch | Cloud Monitoring | Azure Monitor | Prometheus / OpenTelemetry |
| **Keyless identity** | IRSA | Workload Identity | Managed Identity | Service accounts, OIDC |

### 12.3 How we get wide coverage cheaply
- Postgres wire compatibility covers almost every managed Postgres on every cloud with one adapter.
- S3-compatible blob adapter covers many providers; the Rust `object_store` crate can unify S3, GCS, Azure and local disk behind one API (verify it fits our needs when we start).
- RabbitMQ and Redis run anywhere.
- Kubernetes is the common deployment layer; Helm is cloud-agnostic.
- Native queues (Pub/Sub, Service Bus) are separate adapters because their semantics differ (ordering, ack deadlines, dead-lettering). The conformance suite exposes those differences in a table so users know what they get.

### 12.4 Rollout of cloud support

| Pack | Status |
| :--- | :--- |
| **Generic** (RabbitMQ, Redis, Postgres, S3-compatible, local, any Kubernetes) | v1 |
| **AWS** (SQS, S3, RDS, EKS, Terraform) | v1 |
| **GCP** (Pub/Sub, GCS, Cloud SQL, GKE, Terraform) | After v1 (Phase 9) |
| **Azure** (Service Bus, Blob, Azure PostgreSQL, AKS, Terraform) | After v1 (Phase 9) |
| **Others** | Community packs via plugins |

### 12.5 Hybrid and portability rules
- No code path may assume a specific cloud; cloud features are reached only through adapters.
- Config can mix anything (local workers + AWS queue + GCP bucket), with a warning about cross-network latency and egress cost.
- Data portability: `logrix export` and `logrix import` for events, checkpoints and raw archive, to move between environments.

---

## 13. Deployment modes & Kubernetes architecture

> **Detailed Kubernetes Guide:** For complete YAML manifests, KEDA ScaledObjects, Karpenter NodePool definitions, and RBAC configs, see [`KUBERNETES_PLAN.md`](file:///home/vishal/Projects/logrix/KUBERNETES_PLAN.md).

### 13.1 Deployment modes summary

| Mode | How | Use when |
| :--- | :--- | :--- |
| **Local, simple** | `logrix dev` (memory queue + SQLite + local disk) | Trying it, rapid development |
| **Local, full** | Docker Compose with profiles: `rabbitmq`, `bullmq`, `localstack` | Testing real queues and containers |
| **Kubernetes (Primary)** | Helm chart + KEDA ScaledObjects + HPA + Karpenter | Production, any cloud or on-prem |
| **Cloud (IaC)** | Terraform modules per cloud pack (AWS first, GCP/Azure next) | Automated cloud infrastructure provisioning |
| **Hybrid** | Mix via config (e.g., K8s workers + AWS SQS + RDS) | Flexible enterprise topologies |

---

### 13.2 Kubernetes workload architecture: what runs in K8s vs outside

Logrix leverages Kubernetes specifically for stateless autoscaling, self-healing, rolling deploys, and container sandboxing, while stateful data layers (queue, database, blob storage) run in battle-tested external managed cloud services:

#### Outside Kubernetes (Cloud Managed Services)
1. **Queue Service:** AWS SQS (or managed RabbitMQ / Redis / Pub/Sub) — guarantees persistence, eliminates stateful cluster operations in K8s.
2. **Database:** AWS RDS / Aurora PostgreSQL — primary event store, checkpoints, system tables with automated multi-AZ failover and snapshots.
3. **Blob Storage:** AWS S3 (or MinIO / GCS / R2) — immutable raw block and log archive, snapshots.
4. **EVM JSON-RPC Providers:** External provider pool (Alchemy, Infura, QuickNode, or dedicated private nodes).

#### Inside Kubernetes (Application Pods & Workloads)
1. **`logrix-listener` (Deployment, 2 replicas):** Singleton per chain with active/standby leader election via Kubernetes Leases (`coordination.k8s.io`). Runs on On-Demand nodes.
2. **`logrix-controller` (Deployment, 2 replicas):** Singleton per project/chain managing reorg rollback, backfill planning, and the reconciler loop. Runs on On-Demand nodes.
3. **`logrix-decoder-live` (Deployment, 2–6 replicas):** High-priority live event decoders. Minimal fixed pool on On-Demand nodes; scales on head lag.
4. **`logrix-decoder-backfill` (Deployment, 0–50 replicas):** High-throughput historical decoders. Scaled from 0 via KEDA based on the Controller's budget-capped metric (`logrix_desired_replicas`). Scheduled on cost-effective **Spot / Preemptible instances** via Karpenter.
5. **`logrix-webhook-worker` (Deployment, 1–10 replicas):** Dispatches signed HTTP webhooks with backoff and circuit breaking. Scaled via KEDA on webhook queue depth.
6. **`logrix-api` (Deployment, 2–15 replicas):** Serves dynamic GraphQL, REST health/status endpoints. Scaled via built-in **HPA** on CPU utilization (>65%) and request latency.
7. **gRPC Plugins:** Sidecar containers or independent Deployments, sandboxed and network-isolated.

---

### 13.3 Kubernetes resources inventory (what needs to be created)

To run Logrix on Kubernetes, the following resources are maintained in the Helm chart (`deploy/helm/logrix`):
- **Workloads:** Deployments (`listener`, `controller`, `decoder-live`, `decoder-backfill`, `webhook`, `api`).
- **Autoscaling:** KEDA `ScaledObject` (for backfill decoders and webhooks) + native `HorizontalPodAutoscaler` (for API).
- **Node Provisioning:** Karpenter `NodePool` & `EC2NodeClass` (separating Spot worker capacity from On-Demand system capacity).
- **ServiceAccounts & Identity:** `ServiceAccount` with AWS IRSA annotations (`eks.amazonaws.com/role-arn`) for keyless IAM access to SQS and S3.
- **RBAC:** `Role` and `RoleBinding` granting `coordination.k8s.io/leases` access for leader election.
- **Configuration & Secrets:** `ConfigMap` for `logrix.yaml` and chain presets; `ExternalSecret` / `SecretStore` for database credentials and RPC keys.
- **Networking:** `Service` (ClusterIP for API and metrics scraping), `Ingress` (with TLS termination and rate limits), and `NetworkPolicy` (restricting pod egress strictly to RDS, SQS, S3, and RPC endpoints).
- **Lifecycle & Availability:** `PodDisruptionBudget` (PDB) for API and live decoders; `terminationGracePeriodSeconds: 300` and `preStop` hooks for graceful in-flight job drain.


---

## 14. Observability and security

### 14.1 Observability
- **Metrics:** `logrix_head_lag_blocks`, `logrix_events_per_second`, `logrix_queue_depth`, `logrix_reorg_total`, `logrix_dlq_depth`, `logrix_rpc_errors`, `logrix_breaker_state`, `logrix_backpressure_active`, `logrix_reconciler_gaps_fixed`, `logrix_plugin_errors`, `logrix_webhook_success_ratio`
- **RPC, cost and lag metrics:** `logrix_rpc_cost_units_used{provider}`, `logrix_rpc_budget_remaining{provider}`, `logrix_provider_health{provider}`, `logrix_provider_blocks_behind{provider}`, `logrix_learned_range_limit{provider}`, `logrix_spool_depth`, `logrix_stage_latency_seconds{stage}`, `logrix_error_total{class}`
- **Logs and traces:** `tracing` with OpenTelemetry export; a trace follows a block from listener to sink
- **Endpoints:** `/health`, `/readiness`, `/status` (sync status)
- **SLOs** (defined in Phase 0, tuned later): for example head lag under N seconds for 99% of the time, no unrepaired gap older than M minutes
- **Alerts and runbooks:** every alert links to a runbook describing diagnosis and fixes
- **Dashboards:** Grafana ready-made; cloud-native dashboards in the cloud packs

### 14.2 Security: layers

| Area | Threat | Controls |
| :--- | :--- | :--- |
| **Supply chain** | Malicious or vulnerable dependency | `cargo audit` and `cargo deny` in CI, pinned versions, SBOM per release, signed releases and images, minimal base images, reproducible builds as a goal |
| **Code safety** | Memory bugs, logic bugs | Rust, `unsafe` forbidden in core crates, clippy gates, fuzzing (`cargo-fuzz`) of the config parser, ABI decoder and message parsers |
| **Untrusted input** | Hostile ABI/config/RPC response | Size and depth limits, strict parsing, hash validation of chain data, never trust a single RPC blindly |
| **Secrets** | Leaked keys | Secrets port only, never in config files or logs, redaction in logs, short-lived credentials |
| **Identity** | Stolen long-lived cloud keys | Keyless auth per cloud (IRSA, Workload Identity, Managed Identity), least-privilege roles |
| **Network** | Eavesdropping, lateral movement | TLS everywhere, mTLS for plugins and internal traffic, Kubernetes network policies, private endpoints |
| **API access** | Unauthorized use, abuse | API keys with scopes, JWT/OIDC option, role-based access for the admin API, rate limits, GraphQL depth/complexity limits, query timeouts, persisted queries |
| **Plugins (WASM)** | Malicious or runaway code | Sandbox with capability grants (explicit permissions), CPU/memory/time limits, no ambient network or file access |
| **Plugins (gRPC)** | Impersonation, data exposure | Authenticated channels (mTLS or token), allowlist of plugin addresses, optional plugin signing |
| **Data at rest** | Data exposure | Provider encryption plus optional client-side encryption for queue and blob middleware |
| **Webhooks** | SSRF, forged events | HMAC signatures, private IP ranges blocked, timeouts |
| **Auditing** | Untraceable changes | Audit log for admin actions and config changes |
| **Process** | Slow vulnerability response | `SECURITY.md`, private disclosure channel, patch SLAs, a threat model document reviewed every release |
| **Safe defaults** | Accidental exposure by self-hosters | API on localhost until configured; auth required before binding publicly |

---

## 15. Rust stack and repo layout

### 15.1 Main crates (verify versions and maintenance status when we start)

| Need | Crate | Note |
| :--- | :--- | :--- |
| **Async runtime** | `tokio` (+ `tokio-util` for cancellation) | Standard |
| **Resilience building blocks** | `tower` | Timeouts, retries, rate limits, concurrency limits, load shedding |
| **Ethereum RPC, types, ABI** | `alloy` (incl. `alloy-dyn-abi`) | Decodes user-provided ABIs at runtime |
| **HTTP server** | `axum` | Web framework |
| **GraphQL** | `async-graphql` (dynamic schema) | Built from user config |
| **gRPC** | `tonic` + `prost` | Plugins and a future gRPC API |
| **Database** | `sqlx` | Postgres + SQLite |
| **Object storage** | `object_store` (evaluate) and/or the AWS SDK | S3, GCS, Azure, local under one API |
| **AWS** | `aws-sdk-sqs`, `aws-sdk-s3` | Official SDK |
| **RabbitMQ** | `lapin` | Pure-Rust AMQP |
| **Redis** | `redis` | BullMQ-style adapter |
| **WASM host** | `wasmtime` | Sandboxed plugins |
| **Observability** | `tracing`, `opentelemetry`, `metrics` / `prometheus` | Observability stack |
| **CLI** | `clap` | Command-line parsing |
| **Config** | `serde` + a maintained YAML crate | Serialization & parsing |
| **Testing** | `testcontainers`, `proptest`, `cargo-nextest`, `criterion` | Plus `anvil` (Foundry) |
| **Security tooling** | `cargo-audit`, `cargo-deny`, `cargo-fuzz` | CI gates |

### 15.2 Cargo workspace layout

```text
logrix/
├── Cargo.toml workspace
├── crates/
│   ├── logrix-core/           # domain types, traits (ports), errors
│   ├── logrix-plugin-api/     # STABLE, versioned: traits + protobuf + WASM interface
│   ├── logrix-resilience/     # timeouts, retries, breakers, bulkheads, backpressure
│   ├── logrix-middleware/     # built-in middleware for queue / db / blob / chain
│   ├── logrix-config/         # config + schema loading, validation, versioning
│   ├── logrix-chain-evm/      # RPC gateway (Section 6), presets, provider profiles, probing, reorg tracking
│   ├── logrix-queue-memory/
│   ├── logrix-queue-sqs/
│   ├── logrix-queue-rabbitmq/
│   ├── logrix-queue-redis/
│   ├── logrix-store-postgres/
│   ├── logrix-store-sqlite/
│   ├── logrix-blob-s3/
│   ├── logrix-blob-fs/
│   ├── logrix-pipeline/       # filter → decode → handle → sinks, hooks
│   ├── logrix-handlers/       # declarative, wasm, remote
│   ├── logrix-reconciler/     # gap detection and repair
│   ├── logrix-api/            # GraphQL + REST
│   ├── logrix-webhook/
│   ├── logrix-plugin-host/    # loads WASM + gRPC plugins
│   ├── logrix-testkit/        # conformance + chaos helpers for any adapter or plugin
│   ├── logrix-cli/            # the `logrix` binary (selects features)
│   └── (later packs)          # queue-pubsub, queue-servicebus, blob-gcs, blob-azure, sink-kafka, sink-clickhouse
├── proto/                     # protobuf definitions for gRPC plugins
├── presets/                   # chain presets and provider profiles (YAML)
├── deploy/                    # docker/, helm/, terraform/{aws,gcp,azure}/
├── examples/                  # erc20-transfers, uniswap-v3, nft-ownership, wasm-handler
├── templates/                 # plugin skeletons: rust, ts, go, python
└── docs/                      # design notes (ADRs), runbooks, threat model, plugin guide, config reference
```

### 15.3 Rust-specific notes

| Topic | Plan |
| :--- | :--- |
| **Async traits and trait objects** | Adapters are chosen at runtime, so use boxed futures or async-trait where trait objects are needed |
| **Compile times** | Many small crates, shared workspace, sccache, CI caching |
| **Errors** | `thiserror` in libraries; a structured error type with retryable vs permanent classification, used by the resilience layer |
| **Big numbers** | `U256` end to end; `NUMERIC` in Postgres; never floats for token amounts |
| **Contributor barrier** | Plugin templates in other languages (WASM, gRPC) so contributors don't need Rust |

---

## 16. Long-term evolution and future-proofing

This project will grow for years. These rules keep it healthy.

### 16.1 Versioned everything

| Thing | Versioning |
| :--- | :--- |
| **Config file** | `version:` field; `logrix config migrate` upgrades old files |
| **Queue messages** | Schema version in every message; consumers accept N and N-1 |
| **Plugin API** | Semver; capability negotiation at load time |
| **System database schema** | Forward-only migrations, tested for upgrade from every supported version |
| **User GraphQL schema** | Generated; breaking changes need an explicit reindex |
| **Releases** | Semver, changelog, stable and nightly channels, optional LTS |

### 16.2 Safe change process
- Feature flags at build time (Cargo features) and run time (config) so new features ship disabled first
- ADRs / RFCs for any change to a stable surface (plugin API, config format, message format)
- Deprecation policy: at least two minor versions of warnings before removal
- Zero-downtime upgrades: rolling deploys, versioned messages, expand-then-contract DB migrations
- CI gates: format, clippy, tests, conformance suite, benchmark regression, cargo audit/deny, fuzz smoke tests

### 16.3 Open-source health
`CONTRIBUTING.md`, code of conduct, issue and PR templates, a maintainers file, an RFC process, "good first issue" labels, public roadmap, docs treated as a product.

### 16.4 Indicative 12-month feature themes (adjust as we learn)

| Quarter | Theme | Examples |
| :--- | :--- | :--- |
| **Months 1-3** | Solid engine | Core pipeline, Postgres/SQLite, SQS/RabbitMQ/Redis, reliability, reorgs, resilience layer |
| **Months 4-6** | Customization and platform | WASM/TS handlers, plugin system v1, resource middleware, Kubernetes + AWS, scaling features |
| **Months 7-9** | Reach | GCP and Azure packs, more sinks (Kafka, ClickHouse), webhooks v2, security hardening (signed plugins, SBOM), gRPC API |
| **Months 10-12** | Depth | Traces and internal calls, factory/dynamic contracts, dashboard UI, multi-project, Rust-native sources (e.g. Reth ExEx), multi-region DR, non-EVM exploration |

### 16.5 Future-Proofing Check (Design Rule Zero)
Before adding anything (a feature, a table column, a message field, a config option, a crate), imagine that in the future we will add or integrate every likely improvement, and check the design against this list. The goal is to leave a seam, not to build the feature.

| # | Question | Why it matters |
| :--- | :--- | :--- |
| **1** | Is it behind a trait / port, so it can be replaced? | Avoids lock-in to one technology |
| **2** | Does the data model have room to grow? (versioned schema, kind + extra fields, no hard-coded chain assumptions) | New data types without migrations of the whole system |
| **3** | Are keys namespaced (`chain_id`, `project_id`, handler and plugin IDs)? | Multi-project now, more later, without rekeying data |
| **4** | Is behavior a policy or config value rather than hard-coded? | Users can tune without forks |
| **5** | Are wire formats versioned (messages, plugin API, config, archive format)? | Rolling upgrades, N-1 compatibility |
| **6** | Can it ship behind a feature flag, default off? | Safe rollout |
| **7** | Can a plugin (WASM / gRPC) override or extend it? | Extensibility without rebuilds |
| **8** | Does it emit metrics, structured logs and trace IDs? | Operable from day one |
| **9** | Does it have a failure story (error class, retry or park, idempotency)? | Resilience is not an afterthought |
| **10** | Can it be tested by a fake or a conformance test? | Keeps quality as adapters multiply |
| **11** | Does core code avoid cloud-, chain- and provider-specific types? | Portability |
| **12** | Can it scale horizontally and be partitioned? | No single-node ceiling |
| **13** | Is it secure by default (least privilege, untrusted input, no secrets in logs)? | Security grows with features |
| **14** | Does it keep data rebuildable (raw archive, reindex path, handler version recorded)? | Fixable mistakes |
| **15** | Is the decision recorded (ADR) with what was deliberately left out? | Future contributors know why |

> **Counter-rule (avoid over-engineering):** never build a feature "just in case". If a seam costs little now and a lot later, keep it. If it costs a lot now, record it in the register below and revisit when the need is real.

### 16.6 Future-feature register
Anticipated improvements, and the seam that keeps each one possible. Items marked **do now** are cheap seams that are part of v1 design.

| Future feature | Likely timing | Seam that keeps it possible |
| :--- | :--- | :--- |
| **Non-EVM chains** (Solana, Cosmos, Bitcoin...) | Year 2+ | `Source`/`Decode` boundary with generic envelope (4.6) |
| **Traces, internal calls, state diffs** | Year 1 | **do now:** envelope `kind` enum + `extra` map |
| **Pending/mempool data, account abstraction** (user operations) | Year 1-2 | Source plugins; unconfirmed status already modeled |
| **Bulk historical sources** (SQD, HyperSync, Substreams/Firehose) | Year 1 | `HistoricalSource` trait (6.8) |
| **Own-node integration** (Reth ExEx, Erigon) | Year 1 | `LiveSource` / `HistoricalSource` plugins |
| **More sinks:** Kafka, NATS, ClickHouse, BigQuery, Snowflake | Year 1 | `Sink` trait + plugins (Q8) |
| **Data lake export** (Parquet to S3/GCS) | Year 1-2 | **do now:** raw archive in a documented, versioned layout; Sink + blob middleware |
| **Real-time streams** (gRPC streaming, WebSocket, GraphQL subscriptions) | Year 1 | API trait; subscriptions already planned |
| **Time-travel queries** ("state as of block N") | Year 2 | Every row carries block number and hash; reorg-safe retention |
| **Selective reindex per handler or contract** | Year 1 | **do now:** store `handler_version` with derived rows and checkpoints; partition keys |
| **Materialized views and continuous aggregates** | Year 1-2 | DB middleware + column-store sink |
| **Enrichment** (prices, token metadata, ENS, off-chain lookups) | Year 1 | WASM host with capability-granted network access |
| **Cross-chain correlation** | Year 2 | `chain_id` in all keys; multi-chain handlers |
| **Hosted / multi-tenant service** | Optional | **do now:** namespace keys by project; auth scopes and per-project budgets later (D8 keeps tenant code out) |
| **Usage metering and billing** | Optional | **do now:** gateway emits usage events (6.3) |
| **Web dashboard / admin UI** | Year 1-2 | Stable admin API with a published OpenAPI spec |
| **Alerting integrations** (Slack, email, PagerDuty) | Year 1 | Hooks + webhook sink |
| **Plugin marketplace / registry** | Year 2 | **do now:** plugin manifest (name, version, API version, capabilities, optional signature) |
| **Hot reload of handlers and config** | Year 1 | Versioned config; handler versions (above) |
| **Compliance:** audit trail, retention policies, data deletion | Year 1-2 | Audit log in v1; retention in DB middleware |
| **Multi-region active-active** | Year 2+ | **do now:** instance and region IDs in checkpoints and leases; idempotent writes |
| **Edge / embedded single-binary use** | Anytime | SQLite + memory queue already supported |
| **AI-agent access** (e.g. an MCP server over the query API) | Year 1-2 | Stable, documented query API and schema introspection |
| **Cost optimization** (smarter provider routing, caching tiers) | Ongoing | Gateway scheduler and usage metering (6.2) |

> **How to use this register:** when planning a feature, check which seams it needs; when reviewing a change, check it doesn't remove one. Add new rows whenever a new idea comes up.

---

## 17. Roadmap

Estimates assume one focused developer comfortable with Rust; add 20 to 30 percent if Rust is new. A solid, production-grade v1 is roughly 7 to 9 months for one person; a team shortens this. The list is ordered so every phase leaves something working.

| Phase | Goal | Main work | Done when |
| :--- | :--- | :--- | :--- |
| **0. Foundations**<br>(1-2 wks) | Settle hard decisions | Design notes: ordering, reorgs, message format, partition-key model, resilience policy, RPC gateway design, error taxonomy, provider profile format, Future-Proofing Check as an ADR, plugin API outline, BullMQ approach, preset format, SLO draft. Workspace, license, CI (audit, deny, clippy), trait definitions, `logrix-resilience` skeleton | Notes agreed; CI green |
| **1. Walking skeleton**<br>(3-4 wks) | End-to-end on testnet | `alloy` RPC source (fixed chunks + WebSocket polling), listener, decoder, Docker Postgres + RabbitMQ managed by CLI, checkpoints, built-in GraphQL server | `logrix dev` indexes real ERC-20 transfers on Arbitrum Sepolia into Postgres, queryable via GraphQL on `localhost:4000` |
| **2. Reliability core**<br>(4-5 wks) | Trustworthy data | Backfiller, adaptive ranges, live-first priority, retries + DLQ, reorg rollback, reconciler, basic backpressure, circuit breakers, chain presets + probing, raw archive, RPC gateway steps 2 to 5 (error classes, adaptive windows and learned limits, provider pool + profiles, scheduler + budgets + cost estimator, WebSocket + gap-fill), park/halt policy, lag ladder and catch-up mode, fake-RPC fault injector | Scripted reorg, mid-job crash, a dropped message, an RPC outage and a rate-limit storm all end with identical, complete data; cost estimate within a reasonable margin of real usage |
| **3. Queue adapters + resource middleware**<br>(3-4 wks) | Swappable and customizable resources | SQS, RabbitMQ, Redis adapters; middleware framework (compress, encrypt, retry, batch); conformance suite; first chaos tests | Same project runs on all three queues by config change; middleware stack works on queue and blob |
| **4. User logic**<br>(4-5 wks) | Bring your own contracts and logic | Declarative handlers, schema → migrations → dynamic GraphQL, WASM host, TypeScript path | YAML-only user and WASM user both index their own contract |
| **5. Plugin system**<br>(3-4 wks) | Extend without forking | Freeze `logrix-plugin-api` v1, gRPC plugin host, custom middleware via WASM/gRPC, hooks, `logrix plugin new/test`, templates, guide | A third party writes a queue or sink plugin in another language that passes conformance |
| **6. Scale + AWS**<br>(3-4 wks) | Production scale | Helm, KEDA, DB concurrency budget, partitioned tables, batched writes, read replicas, leader election, Terraform (AWS), benchmark suite | Decoders scale 1→50→1 on EKS without overloading the DB; benchmark targets met |
| **7. Security hardening**<br>(2-3 wks) | Safe to expose | API auth and scopes, mTLS for plugins, fuzzing, SBOM and signed releases, threat-model review, safe defaults | Security checklist passes; fuzz targets run in CI |
| **8. Webhooks + sinks**<br>(2-3 wks) | Event delivery | Webhook worker, signing, retry, replay, revert events, sink trait + one extra sink | Endpoint outage → events delivered in order after recovery |
| **9. Multi-cloud packs**<br>(3-4 wks) | Run anywhere | GCP and Azure adapters (queues, blobs), Terraform modules, KEDA scalers, export/import | Same project deploys on AWS, GCP and Azure with only config changes |
| **10. Growth**<br>(ongoing) | Ongoing features | See Section 16.4: more DBs, traces, factory contracts, dashboard, multi-region DR, gRPC API, plugin registry, bulk historical sources and verification mode (RPC step 6), items from the future-feature register (Section 16.6) | Public docs, 4 examples, first outside plugin |

---

## 18. Testing

| Type | What it checks |
| :--- | :--- |
| **Conformance suite** (`logrix-testkit`) | One shared test set every queue, DB, blob, sink and plugin must pass (ack, retry, DLQ, timeouts, concurrency, ordering), whatever its language |
| **Integration** | Real Postgres, RabbitMQ, Redis, LocalStack, MinIO (and later GCP/Azure emulators) in containers |
| **Chain simulation** | Local forked chain (`anvil`) with scripted reorgs; recorded mainnet blocks |
| **Property tests** (`proptest`) | Replaying any sequence of blocks (with duplicates and reorderings) yields the same final state |
| **Chaos and fault injection** | Cut the network, add latency, drop and duplicate messages, kill decoders, fill the disk, fail over the DB, throttle the RPC. Verify no gaps, no corruption, bounded memory, recovery without manual action |
| **Failure-matrix tests** | One automated test for every row of the Section 10.4 matrix |
| **Fake-RPC fault injection** | A fake node returns 429s, timeouts, range and result-cap errors, stale heads, null blocks, incomplete logs, WebSocket drops and reorgs on command; every error class and provider-profile rule has a test |
| **Cost and budget tests** | Estimator vs real metered usage; budgets stop or throttle correctly; live reservation holds under backfill load |
| **Lag-ladder tests** | Induced lag triggers each escalation step and recovers cleanly |
| **Backpressure tests** | Slow the DB and confirm decoders slow down, queues stay bounded, live keeps priority |
| **Plugin abuse tests** | Infinite loops, memory bombs, hangs, malicious responses must not hurt the core |
| **Security tests** | Fuzzing, dependency audits, API abuse (deep queries, floods), SSRF checks |
| **Load and soak** | Backfill blocks/sec, live latency p99, multi-day soak for leaks |
| **Upgrade tests** | Upgrade from each supported version with messages in flight |

---

## 19. Risks

| # | Risk | Impact | Mitigation |
| :--- | :--- | :--- | :--- |
| **1** | "Extensible to any extent" becomes endless design | Never ships | Fixed extension points (Section 4); freeze plugin API v1 only in Phase 5 after real usage |
| **2** | Plugin API mistakes locked in by semver | Painful breaking changes | Keep it small; mark experimental parts; dogfood with official adapters |
| **3** | BullMQ isn't native to Rust | Fragile adapter | Option A for v1; real BullMQ via gRPC plugin |
| **4** | Ordering for stateful handlers | Wrong balances and totals | Design note first; per-partition ordering |
| **5** | Rust learning curve and compile times | Slower progress | Small crates, caching, simple async patterns |
| **6** | WASM host interface design | Hard to change later | Minimal host API first; grow carefully |
| **7** | RPC provider quirks | Gaps or stalls | Failover, rate limits, adaptive ranges, reconciler |
| **8** | Dynamic GraphQL schema complexity | Bugs in generated schema | Strong tests; small feature set in v1 |
| **9** | "Any EVM chain" differences | Wrong finality or broken getLogs | Presets, probing, support tiers |
| **10** | Open-source maintenance load | Support eats time | Conformance suite, guides, strict v1 scope |
| **11** | Scope creep from "everything pluggable" | Never ships | Official adapters limited per Section 12.4; everything else is a community plugin |
| **12** | Resilience complexity (many policies interacting: retries, breakers, backpressure) | Subtle bugs, retry storms | One shared layer, retry budgets, failure-matrix tests, chaos testing from Phase 3 |
| **13** | Autoscaling overloads the database | Outage caused by scaling | DB concurrency budget; scale-up limited by downstream capacity |
| **14** | Cloud semantics differ (ordering, ack deadlines, dead-lettering across SQS, Pub/Sub, Service Bus) | Behavior surprises, hidden bugs | Conformance suite documents capabilities per adapter; adapters declare what they guarantee |
| **15** | Many adapters to maintain | Slow upgrades, rot | Strict "official" list; community packs; conformance as the quality gate |
| **16** | Plugin security | Compromise through a plugin | Sandbox, capability grants, auth, optional signing, allowlists |
| **17** | Long-term compatibility | Upgrades break users | Versioning rules (Section 16.1), N-1 support, upgrade tests |
| **18** | One developer, large scope | Burnout, delay | Phased roadmap where each phase ships something usable; share the plugin work with contributors early |
| **19** | RPC cost surprises | Unexpected bills or multi-day backfills | Cost estimator, per-provider budgets, hard stops, usage metering |
| **20** | Providers return incomplete or inconsistent data | Silent gaps | Reconciler, optional verification mode, multi-provider cross-check |
| **21** | Provider profile maintenance (prices and limits change often) | Stale limits and costs | Profiles are data, community-maintained, verified by capability probing; costs shown as estimates |
| **22** | Over-engineering from "future-proofing everything" | Slow delivery | Counter-rule in 16.5: build the seam, not the feature; register deferred ideas |
| **23** | Redis queue misconfiguration (eviction deletes jobs) | Silent data loss | Startup check requires `noeviction`; reconciler as a safety net |

---

## 20. Next steps

1. **Answer or accept defaults for:**
   - **Q3:** BullMQ (Option A: Redis adapter modeled on BullMQ semantics first, real BullMQ compatibility later)
   - **Q6:** Scale target (millions of blocks backfill, under 5 s block-to-query)
   - **Q7:** Non-EVM later? (Not in v1. Keep one clean boundary so it is possible later)
   - **Q8:** Extra sinks (Plugins after v1)
   - **Q9:** Clouds after AWS (GCP, then Azure; others through generic adapters)
   - **Q10:** HA/DR level (Multi-AZ in v1; multi-region DR designed in, built later)
   - **Q11:** First RPC providers and bulk sources (Generic JSON-RPC + provider profiles for big names and public nodes; bulk sources as plugins later)
   - **Q12:** Verification mode (Off by default, opt-in per chain)
2. **Update the plan** based on any decision refinements.
3. **Start Phase 0:**
   - The design notes first (ordering, reorgs, message format, resilience policy, partition key, RPC gateway, error taxonomy, Future-Proofing Check)
   - Workspace setup, license, CI (audit, deny, clippy), and trait definitions
