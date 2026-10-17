<p align="center">
  <img src="logo.png" alt="Logrix" width="100%" />
</p>

# Logrix

A self-hosted EVM log indexer with a Rust runtime, PostgreSQL storage,
RabbitMQ or SQS queues, declarative/WASM handlers, and a GraphQL API.
The AssemblyScript SDK prepares handler projects and Helm values; deployment
is a separate infrastructure operation.

**Current implementation, not a production-readiness guarantee:** review the
[operational limits](#operational-limits) before relying on failover, rollback,
handler state, or external notifications. Performance and cost depend on the
workload and infrastructure; this repository does not establish a universal
events-per-second, latency, or storage-savings guarantee.

## Documentation

- [Configuration: supported controls, precedence, and fixed behavior](docs/configuration.md)
- [All documentation and crate guides](docs/README.md)
- [AssemblyScript SDK and current build workarounds](packages/logrix-sdk/README.md)
- [Deployment overview](deploy/README.md), [Helm](deploy/helm/logrix/README.md),
  [Kustomize](deploy/k8s/README.md), and [AWS Terraform](deploy/terraform/aws/README.md)
- [Development and checks](CONTRIBUTING.md) and [security](SECURITY.md)

## What is implemented

- HTTP JSON-RPC polling with primary/fallback provider routing, a short-lived
  head-height cache, adaptive range recommendations, and estimated CU accounting.
- Separate ingestion, processing, API, and webhook roles, or one `all-in-one`
  process. PostgreSQL is the runtime database; SQLite and an embedded dev queue
  are not provided.
- Raw log/checkpoint persistence, ERC-20 Transfer extraction, and custom
  declarative or Wasmtime guest handlers. SDK handlers use AssemblyScript,
  not ordinary Node.js TypeScript or an embedded JavaScript interpreter.
- Core GraphQL queries, or a schema-defined entity API with optional WebSocket
  subscriptions. Webhook management uses separate REST routes.
- Bounded in-memory continuity detection, checkpoint-lag reconciliation, and
  SQL reversion of later rows, with the limits described below.
- Optional best-effort compressed live-envelope uploads to S3-compatible storage.
- Helm and Kustomize resources plus reference AWS Terraform infrastructure.
  Operators supply prerequisites and validate their own deployment.

## Local runtime quickstart

Run these commands from the repository root on a development machine. This
path uses the built-in ERC-20 processor and does not require SDK compilation,
Kubernetes, an AWS account, a manifest, or a dynamic entity schema.

### 1. Prepare the tools and dependencies

- Rust **1.96.0**, Cargo, rustfmt, and Clippy; see `rust-toolchain.toml`.
- A Linux C/C++ build toolchain, CMake, and `pkg-config` for native Rust dependencies.
  The [Dockerfile](Dockerfile) provides the Linux build environment as an alternative.
- Docker Engine with the Compose plugin for local PostgreSQL and RabbitMQ.
- Internet access to an EVM JSON-RPC endpoint with the history you intend to query.
- SDK contributors additionally use Node.js **24.10.0** (`.nvmrc`) and npm.
  These are reproducible development versions, not minimum-supported-version claims.

First-time dependency preparation can use the network:

```bash
cargo fetch --locked
cargo build --locked -p logrix-cli --bin logrix
```

Subsequent builds can add `--offline` when all locked dependencies are cached.

### 2. Start the local backing services

```bash
docker compose up -d --wait postgres rabbitmq
```

The Compose defaults are development credentials: user/password `logrix` for
both services, database `logrix`, ports 5432 and 5672. Their named volumes retain
data between restarts. Compose starts backing services, not the Logrix runtime.
Use a private environment configuration for real credentials; see
[deployment secrets](deploy/README.md#secrets-and-configuration-precedence).

### 3. Run the indexer

```bash
./target/debug/logrix all-in-one --port 4000
```

The default configuration selects Arbitrum Sepolia (`CHAIN_ID=421614`), its
public RPC, and contract `0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d`.
`all-in-one` runs migrations and starts the ingester, processor, and API.
On a fresh checkpoint, ingestion starts at head minus 50 blocks; otherwise it
resumes at checkpoint plus one. `START_BLOCK` only applies without a checkpoint.
Public RPC availability and event activity are not guaranteed.

To choose a different endpoint or contract, keep the network settings consistent:

```bash
./target/debug/logrix --chain-id 421614 \
  --rpc-url https://sepolia-rollup.arbitrum.io/rpc \
  --contract-address 0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d \
  all-in-one --port 4000
```

Root options go **before** the subcommand. Credential-bearing URLs belong in
the process environment rather than command history. The runtime does not
automatically load `.env` files.

### 4. Query the API

Open [http://localhost:4000/](http://localhost:4000/) for GraphiQL. From another
terminal, query the core schema:

```bash
curl http://localhost:4000/graphql \
  -H 'Content-Type: application/json' \
  -d '{"query":"{ health { status version } checkpoint(chainId: 421614) { lastIndexedBlock } transfers(chainId: 421614, filter: { limit: 5 }) { txHash amount fromAddress toAddress } }"}'
```

The checkpoint can initially be null and transfers can be empty. `health` is a
liveness response, not proof of complete or canonical indexed history.
`/readyz` checks database reachability; `/metrics` exposes runtime metrics.

Stop the runtime with Ctrl-C. Run `docker compose stop postgres rabbitmq` to
stop the local services without deleting their volumes. Shutdown observes
signals but does not guarantee every in-flight operation is drained.

## Custom indexing and configuration

Use the [SDK guide](packages/logrix-sdk/README.md) for scaffolding, ABI bindings,
AssemblyScript compilation, and `export-values`. It documents the currently
required runtime-subpath import workaround and repeated-build limitation.
The independent [starter](templates/typescript-starter/README.md) and
[Uniswap example](arbitrum-uniswap-indexer/README.md) have their own limitations;
they are not certified end-to-end indexers.

For direct runtime loading, set `MANIFEST_PATH` and optionally `SCHEMA_PATH`.
These are different files: handler manifests select logic, while entity schemas
define GraphQL fields. Neither file configures every adapter automatically.
See [configuration](docs/configuration.md) for supported keys and precedence.

All workspace crates have their own configuration and behavior reference:

| Area | Crate guides |
| --- | --- |
| Contracts and testing | [core](crates/logrix-core/README.md), [resilience](crates/logrix-resilience/README.md), [testkit](crates/logrix-testkit/README.md) |
| Runtime roles | [CLI](crates/logrix-cli/README.md), [API](crates/logrix-api/README.md), [handlers](crates/logrix-handlers/README.md), [webhooks](crates/logrix-webhook/README.md) |
| Chain processing | [EVM client](crates/logrix-chain-evm/README.md), [RPC gateway](crates/logrix-rpc-gateway/README.md), [reconciler](crates/logrix-reconciler/README.md) |
| Persistence and transport | [PostgreSQL](crates/logrix-store-postgres/README.md), [RabbitMQ](crates/logrix-queue-rabbitmq/README.md), [SQS](crates/logrix-queue-sqs/README.md), [S3](crates/logrix-blob-s3/README.md) |

## Deploying containers

Use pre-built, reviewed, versioned images; build and publish on a workstation or
in CI, not on production nodes. Start with the [deployment overview](deploy/README.md).
The source Helm chart is `deploy/helm/logrix`; chart/image settings and release
workflows do not prove published artifacts are accessible.

Split-role deployments require a separately run migration step. Helm defaults
also require KEDA and API resource metrics unless their scalers are disabled.
The chart currently sets `MANIFEST_PATH` for an inline manifest but does not
mount that file into the ingester; SDK bundles need a consumer-managed mount
patch before that deployment path can start successfully.
Built-in chart PostgreSQL/RabbitMQ/MinIO are ephemeral demonstration services,
not durable production infrastructure. AWS Terraform provisions reference
infrastructure, not the complete application or its operators.

## Operational limits

- **Leadership:** the Kubernetes elector does not safely acquire/renew leases.
  Do not rely on the chart's default two-ingester setup for HA. Use one ingester
  and prevent overlap during updates; see [deployment guidance](deploy/README.md).
- **Consistency:** raw events and their checkpoint share a SQL transaction;
  entity writes, transfers, engine memory, queue acknowledgment, and notifications
  do not share that transaction. Replaying custom aggregate handlers is not
  automatically safe.
- **Reorgs:** continuity checks run on live envelopes, not range backfills.
  Rollback marks later rows reverted but does not restore prior entity versions
  or handler memory. A checkpoint is not proof of contiguous historical coverage.
- **RPC:** bulk SQD/HyperSync transport is not implemented. CU budgets are
  process-local estimates, not strict billing caps. Provider selection is
  priority-based, not latency-weighted.
- **WASM:** fuel metering is implemented; memory settings are pre/post-execution
  checks, not enforced allocation ceilings. SDK signed numeric decoding and
  metadata parsing have additional documented limitations.
- **Notifications:** subscriptions are non-durable. Webhook producer/filter
  names are not fully aligned; entity subscriptions are not entity webhooks.
- **Security:** API/webhook management routes have no built-in authentication.
  Restrict access, add an authenticated boundary, and verify webhook signatures
  with replay protection. See [security guidance](SECURITY.md).

## Build and test

Prepare SDK contributor dependencies once with `npm ci` in
`packages/logrix-sdk`, after preparing Cargo's cache as above. Then the single
offline project test command is:

```bash
make test
```

It runs locked offline Rust workspace tests and the SDK's CLI build/converter
tests. It is not a real-service or cloud deployment certification. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the exact commands, lint/format checks,
tool prerequisites, and deployment validation.

## License

[Apache License 2.0](LICENSE).
