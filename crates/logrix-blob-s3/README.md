# logrix-blob-s3

Amazon S3 and cloud object store integration for Logrix raw block and event log archives.

## Overview
Stores compressed historical block logs, transaction receipts, and entity snapshots in S3-compatible object storage (AWS S3, Cloudflare R2, MinIO, LocalStack) for cold-tier indexing, audit logs, and bulk backfills.

## Features
- **S3 BlobPort Implementation**: Provides `put`, `get`, `delete`, and bandwidth-efficient `exists` via S3 `head_object` without downloading object bodies.
- **Embedded Zstandard Compression**: High-throughput compression via `put_compressed` and `get_decompressed` (~80% compression ratio on block JSON).
- **Flexible Endpoint Compatibility**: Supports path-style addressing and endpoint overrides for MinIO, LocalStack, and Ceph.
- **Fluent Configuration Builder**: `S3Config::builder("bucket")` simplifies programmatic client customization (region, prefix, custom endpoint, path-style addressing).

---

## Configuration & Tuning (`S3Config`)

### Environment Variables & CLI Options

| Variable | CLI Flag | Default Value | Required? | Description |
|---|---|---|---|---|
| `S3_BUCKET` | `--s3-bucket` | None | Required if S3 active | Target S3 bucket name for raw block and blob archives |
| `S3_ENDPOINT` | `--s3-endpoint` | None (AWS standard) | Optional | Custom S3 endpoint override (e.g. `http://localhost:4566` for LocalStack, `http://minio:9000`) |
| `S3_PREFIX` | `--s3-prefix` | None (`""`) | Optional | Key prefix prepended to all stored object keys |
| `AWS_REGION` | N/A | `us-east-1` (or AWS default) | Optional | AWS region for S3 client |

> Note: When `S3_ENDPOINT` is provided (e.g. LocalStack or MinIO), `force_path_style(true)` is automatically activated to ensure compatibility with non-AWS object stores.

### Programmatic Configuration

```rust
use logrix_blob_s3::S3Config;

// Initialize from environment with custom endpoint
let config = S3Config::from_env(
    "my-archive-bucket",
    Some("arbitrum-one/blocks".to_string()),
    Some("http://localhost:9000"), // MinIO endpoint
).await?;
```
