# logrix-blob-s3

Amazon S3 and cloud object store integration for Logrix raw block and event log archives.

## Overview
Stores compressed historical block logs, transaction receipts, and entity snapshots in S3-compatible object storage (AWS S3, Cloudflare R2, MinIO, LocalStack) for cold-tier indexing, audit logs, and bulk backfills.

## Features
- **S3 BlobPort Implementation**: Provides `put`, `get`, `delete`, and bandwidth-efficient `exists` via S3 `head_object` without downloading object bodies.
- **Embedded Zstandard Compression**: High-throughput compression via `put_compressed` and `get_decompressed` (~80% compression ratio on block JSON).
- **Flexible Endpoint Compatibility**: Supports path-style addressing and endpoint overrides for MinIO, LocalStack, and Ceph.
- **Fluent Configuration Builder**: `S3Config::builder("bucket")` simplifies programmatic client customization (region, prefix, custom endpoint, path-style addressing).

