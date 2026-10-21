use crate::client::S3Config;
use logrix_core::{
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::BlobPort,
};

/// AWS S3 implementation of `BlobPort`.
#[derive(Clone)]
pub struct S3BlobStore {
    pub(crate) config: S3Config,
}

impl S3BlobStore {
    pub fn new(config: S3Config) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &S3Config {
        &self.config
    }

    /// Upload data compressed with Zstandard and Content-Encoding: zstd metadata.
    pub async fn put_compressed(&self, key: &str, data: &[u8], level: i32) -> LogrixResult<()> {
        let compressed = crate::compression::compress_zstd(data, level).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::BlobStore,
                format!("Failed to compress blob with zstd: {e}"),
            )
        })?;
        let resolved_key = self.config.resolve_key(key);
        let body = aws_sdk_s3::primitives::ByteStream::from(compressed);
        let put_fut = self
            .config
            .client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&resolved_key)
            .content_encoding("zstd")
            .body(body)
            .send();

        tokio::time::timeout(self.config.request_timeout, put_fut)
            .await
            .map_err(|_| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::BlobStore,
                    format!("Timeout putting S3 object at {resolved_key}"),
                )
            })?
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::BlobStore,
                    format!("Failed to put S3 object at {resolved_key}: {e}"),
                )
            })?;

        tracing::debug!(key = %resolved_key, "Uploaded zstd-compressed object to S3");
        Ok(())
    }

    /// Download and decompress data previously compressed with Zstandard.
    pub async fn get_decompressed(&self, key: &str) -> LogrixResult<Option<Vec<u8>>> {
        match self.get(key).await? {
            Some(bytes) => {
                let decompressed = crate::compression::decompress_zstd(&bytes).map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Integrity,
                        ErrorSource::BlobStore,
                        format!("Failed to decompress zstd blob: {e}"),
                    )
                })?;
                Ok(Some(decompressed))
            }
            None => Ok(None),
        }
    }
}
