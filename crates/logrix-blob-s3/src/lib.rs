//! AWS S3 blob store adapter implementing `BlobPort`.
//!
//! Provides durable object storage for raw block envelopes, transaction receipts,
//! and state checkpoints with optional Zstandard compression.
//! Supports AWS S3, LocalStack, MinIO, and Cloudflare R2.

pub mod builder;
pub mod client;
pub mod compression;
pub mod port;
pub mod store;

pub use builder::S3ClientOptionsBuilder;
pub use client::{S3Config, DEFAULT_REQUEST_TIMEOUT};
pub use compression::{compress_zstd, decompress_zstd, DEFAULT_ZSTD_LEVEL};
pub use store::S3BlobStore;
