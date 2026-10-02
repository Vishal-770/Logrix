use crate::error::LogrixResult;
use async_trait::async_trait;

/// Abstract object / blob store port for archiving raw block envelopes and snapshots.
///
/// Implemented by S3, MinIO, Local Disk, and Cloudflare R2.
#[async_trait]
pub trait BlobPort: Send + Sync {
    /// Save raw data buffer at key location.
    async fn put(&self, key: &str, data: &[u8]) -> LogrixResult<()>;

    /// Retrieve raw data buffer by key location.
    async fn get(&self, key: &str) -> LogrixResult<Option<Vec<u8>>>;

    /// Delete object at key location.
    async fn delete(&self, key: &str) -> LogrixResult<()>;
}
