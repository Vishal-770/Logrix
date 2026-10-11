use crate::client::S3Config;
use async_trait::async_trait;
use aws_sdk_s3::{operation::get_object::GetObjectError, primitives::ByteStream};
use logrix_core::{
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::BlobPort,
};
use tracing::debug;

/// AWS S3 implementation of `BlobPort`.
#[derive(Clone)]
pub struct S3BlobStore {
    config: S3Config,
}

impl S3BlobStore {
    pub fn new(config: S3Config) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &S3Config {
        &self.config
    }

    /// Upload data compressed with Zstandard.
    pub async fn put_compressed(&self, key: &str, data: &[u8], level: i32) -> LogrixResult<()> {
        let compressed = crate::compression::compress_zstd(data, level).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::BlobStore,
                format!("Failed to compress blob with zstd: {e}"),
            )
        })?;
        self.put(key, &compressed).await
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

#[async_trait]
impl BlobPort for S3BlobStore {
    async fn put(&self, key: &str, data: &[u8]) -> LogrixResult<()> {
        let resolved_key = self.config.resolve_key(key);
        let body = ByteStream::from(data.to_vec());

        self.config
            .client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&resolved_key)
            .body(body)
            .send()
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::BlobStore,
                    format!("Failed to put S3 object at {resolved_key}: {e}"),
                )
            })?;

        debug!(key = %resolved_key, "Uploaded object to S3");
        Ok(())
    }

    async fn exists(&self, key: &str) -> LogrixResult<bool> {
        let resolved_key = self.config.resolve_key(key);
        let resp = self
            .config
            .client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&resolved_key)
            .send()
            .await;

        match resp {
            Ok(_) => Ok(true),
            Err(sdk_err) => {
                if let Some(service_err) = sdk_err.as_service_error() {
                    if matches!(
                        service_err,
                        aws_sdk_s3::operation::head_object::HeadObjectError::NotFound(_)
                    ) {
                        return Ok(false);
                    }
                }
                if let aws_sdk_s3::error::SdkError::ServiceError(ref err) = sdk_err {
                    if err.raw().status().as_u16() == 404 {
                        return Ok(false);
                    }
                }
                Err(LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::BlobStore,
                    format!("Failed to head S3 object at {resolved_key}: {sdk_err}"),
                ))
            }
        }
    }

    async fn get(&self, key: &str) -> LogrixResult<Option<Vec<u8>>> {
        let resolved_key = self.config.resolve_key(key);

        let resp = self
            .config
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&resolved_key)
            .send()
            .await;

        match resp {
            Ok(output) => {
                let bytes = output.body.collect().await.map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Transient,
                        ErrorSource::BlobStore,
                        format!("Failed to stream S3 object body for {resolved_key}: {e}"),
                    )
                })?;
                Ok(Some(bytes.to_vec()))
            }
            Err(sdk_err) => {
                if let Some(service_err) = sdk_err.as_service_error() {
                    if matches!(service_err, GetObjectError::NoSuchKey(_)) {
                        return Ok(None);
                    }
                }
                Err(LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::BlobStore,
                    format!("Failed to get S3 object at {resolved_key}: {sdk_err}"),
                ))
            }
        }
    }

    async fn delete(&self, key: &str) -> LogrixResult<()> {
        let resolved_key = self.config.resolve_key(key);

        self.config
            .client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&resolved_key)
            .send()
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::BlobStore,
                    format!("Failed to delete S3 object at {resolved_key}: {e}"),
                )
            })?;

        debug!(key = %resolved_key, "Deleted object from S3");
        Ok(())
    }
}
