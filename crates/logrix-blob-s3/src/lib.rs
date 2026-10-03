//! AWS S3 blob store adapter implementing `BlobPort`.
//!
//! Provides durable object storage for raw block envelopes, transaction receipts,
//! and state checkpoints with optional Zstandard compression.
//! Supports AWS S3, LocalStack, MinIO, and Cloudflare R2.

pub mod blob;
pub mod client;
pub mod compression;

pub use blob::S3BlobStore;
pub use client::S3Config;
pub use compression::{compress_zstd, decompress_zstd, DEFAULT_ZSTD_LEVEL};
