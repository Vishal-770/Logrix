# Logrix S3 Blob Storage Architecture & Operational Guide

This document provides a comprehensive technical guide on the architecture, implementation, key schemas, and production use cases of Amazon S3 object storage in the Logrix indexing engine (`crates/logrix-blob-s3`).

---

## 1. Executive Summary & Tiered Storage Philosophy

In high-throughput EVM blockchain indexing, persisting raw blocks, full transaction receipts, and execution logs directly into a relational database causes rapid disk bloat, cache thrashing, and high database infrastructure costs.

Logrix solves this with a **Two-Tier Storage Architecture**:

| Storage Layer | Technology | Primary Role | Cost Profile | Access Pattern |
| :--- | :--- | :--- | :--- | :--- |
| **Hot Serving Layer** | Aurora PostgreSQL | Real-time domain entities (`User`, `Reserve`, `BorrowPosition`) queried by dApps via GraphQL | ~$0.12 / GB / month | Concurrent indexed SQL reads, transactional upserts |
| **Cold Archival Layer** | Amazon S3 / MinIO | Raw block envelopes, transaction receipts, WASM bytecode, checkpoint snapshots | ~$0.023 down to $0.004 / GB / month | Sequential streaming, high-throughput bulk reads/writes |

By storing raw chain data in S3 with Zstandard (`zstd`) compression, Logrix reduces storage costs by over **80%** while keeping PostgreSQL lean and query latencies low.

---

## 2. Hexagonal Architecture: The `BlobPort` Abstraction

Logrix enforces strict boundary separation using Hexagonal Architecture (Ports and Adapters):
* **The Port**: `logrix-core::ports::BlobPort` defines an abstract object storage contract. The core indexing engine does not depend on AWS SDKs or concrete drivers.
* **The Adapter**: `crates/logrix-blob-s3` provides the production implementation using `aws-sdk-s3`.

```rust
#[async_trait]
pub trait BlobPort: Send + Sync {
    /// Save raw data buffer at key location
    async fn put(&self, key: &str, data: &[u8]) -> LogrixResult<()>;

    /// Retrieve raw data buffer by key location
    async fn get(&self, key: &str) -> LogrixResult<Option<Vec<u8>>>;

    /// Check if object exists without downloading body
    async fn exists(&self, key: &str) -> LogrixResult<bool>;

    /// Delete object at key location
    async fn delete(&self, key: &str) -> LogrixResult<()>;
}
```

---

## 3. The 4 Production Use Cases of S3 in Logrix

### 3.1 Raw Block Envelope & Transaction Receipt Archival
* **Context**: Ethereum blocks and receipts contain large JSON/RLP payloads. Persisting them in PostgreSQL tables degrades table indexing and database memory caches.
* **Implementation**: The Ingester batches raw block payloads, compresses them with Zstandard, and writes them to S3. Processors extract relevant event logs and store only the resulting structured entities in PostgreSQL.

### 3.2 High-Throughput Historical Backfills (Bypassing RPC Limits)
* **Context**: EVM JSON-RPC nodes (Alchemy, Infura, QuickNode) enforce strict rate limits (50-200 RPS) and charge per compute unit. Backfilling millions of historical blocks from RPC is slow and expensive.
* **Implementation**: Once blocks are archived in S3, backfill workers stream blocks directly from S3 at gigabit speeds over AWS private networking with **zero RPC rate limits and zero RPC fees**.

### 3.3 Reorg Rollbacks & Deterministic Checkpoint Snapping
* **Context**: Blockchain forks require indexers to revert unfinalized state back to a common ancestor block.
* **Implementation**: At epoch boundaries (e.g. every 1,000 blocks), Logrix creates a point-in-time state checkpoint and writes it to S3. If an extended reorganization or catastrophic failure occurs, processors can restore state directly from the verified S3 snapshot.

### 3.4 WebAssembly (WASM) Bytecode & Contract Manifests
* **Context**: Logrix executes user-defined event mappings inside a deterministic Wasmer WebAssembly sandbox.
* **Implementation**: Compiled WASM binaries (`.wasm`) and contract ABI definitions are stored in S3. Dynamically autoscaled processor pods pull the WASM artifacts from S3 on initialization.

---

## 4. Key Naming Scheme & Partitioning Strategy

To prevent S3 partition throttling and ensure high I/O parallelism, Logrix organizes S3 object keys using a structured, deterministic hierarchy:

```text
s3://<bucket-name>/<prefix>/
  |-- chains/
  |   \-- 1/                                   # Ethereum Mainnet (Chain ID: 1)
  |       |-- blocks/
  |       |   \-- 26000000-26099999/           # 100k block partition folders
  |       |       |-- 26269700.bin.zst         # Compressed block envelope
  |       |       \-- 26269701.bin.zst
  |       |-- receipts/
  |       |   \-- 26269700.receipts.zst        # Full transaction receipts
  |       \-- checkpoints/
  |           \-- epoch_26269000.snap.zst      # State snapshot
  \-- artifacts/
      \-- aave-v3/
          |-- mapping.wasm                     # Compiled user WASM mapping
          \-- abi.json                         # Contract ABI schema
```

---

## 5. Performance & Cost Optimizations

### 5.1 Zstandard (Zstd) High-Ratio Compression
* Implemented in `crates/logrix-blob-s3/src/compression.rs`.
* Raw block JSON payloads compress by **75% to 85%** at Zstd Level 3.
* Uploaded objects automatically include the HTTP header `Content-Encoding: zstd`.
* Methods `put_compressed` and `get_decompressed` provide end-to-end transparent compression.

### 5.2 Bandwidth-Efficient Existence Checks (`HeadObject`)
* Standard `get()` methods download the full object body just to verify presence.
* In `crates/logrix-blob-s3/src/port.rs`, `exists()` executes an S3 `HeadObject` call:
  * Only object headers are fetched (zero payload bytes transferred).
  * 404 responses are caught gracefully and return `Ok(false)` without throwing exceptions.

### 5.3 AWS VPC Gateway Endpoint ($0 Data Transfer Fee)
* Provisioned in `deploy/terraform/aws/modules/vpc/main.tf` (`aws_vpc_endpoint.s3`).
* Kubernetes pods communicate with S3 through an internal VPC gateway route table.
* Traffic never traverses the public internet or NAT Gateway, resulting in **$0.00 data transfer costs** and sub-millisecond latency.

### 5.4 Automated S3 Lifecycle Tiering
* Provisioned in `deploy/terraform/aws/modules/s3/main.tf`:
  * **Day 0 to 30**: Stored in **S3 Standard** (hot access for active ingestion and processing).
  * **Day 30+**: Automatically transitioned to **S3 Standard-IA** (Infrequent Access, ~50% cost reduction).
  * **Day 90+**: Automatically transitioned to **S3 Glacier Instant Retrieval** (~70% cost reduction, millisecond retrieval).

---

## 6. Security & Production Hardening

* **Server-Side Encryption**: All objects are encrypted at rest using AES-256 (`aws_s3_bucket_server_side_encryption_configuration`).
* **Public Access Block**: All public ACLs, policies, and bucket access are completely disabled (`aws_s3_bucket_public_access_block`).
* **IAM Roles for Service Accounts (IRSA)**: Pods authenticate using temporary OpenID Connect (OIDC) security tokens with least-privilege IAM permissions (`s3:PutObject`, `s3:GetObject`, `s3:DeleteObject`).
* **Request Boundaries**: All operations enforce strict timeouts (`DEFAULT_REQUEST_TIMEOUT = 30s`) to prevent hung network connections.

---

## 7. Multi-Cloud & Local Testing Support

The `S3Config` builder supports alternative S3-compatible storage providers:
* **AWS S3**: Default cloud provider with automatic region and IAM credential resolution.
* **MinIO / LocalStack**: Enabled via custom endpoint (`http://localhost:9000`), automatically activating path-style addressing (`force_path_style = true`).
* **Cloudflare R2**: Compatible via custom R2 endpoint and API credentials with zero code changes.

```rust
use logrix_blob_s3::S3Config;

// Production AWS S3
let config = S3Config::builder("my-logrix-bucket")
    .region("us-east-1")
    .key_prefix("ethereum-mainnet")
    .build()
    .await?;

// Local MinIO / LocalStack
let local_config = S3Config::builder("logrix-local")
    .endpoint("http://localhost:9000")
    .force_path_style(true)
    .build()
    .await?;
```
