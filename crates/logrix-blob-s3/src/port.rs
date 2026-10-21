use crate::store::S3BlobStore;
use async_trait::async_trait;
use aws_sdk_s3::{operation::get_object::GetObjectError, primitives::ByteStream};
use logrix_core::{
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::BlobPort,
};
use tracing::debug;

#[async_trait]
impl BlobPort for S3BlobStore {
    async fn put(&self, key: &str, data: &[u8]) -> LogrixResult<()> {
        let resolved_key = self.config.resolve_key(key);
        let body = ByteStream::from(data.to_vec());

        let put_fut = self
            .config
            .client
            .put_object()
            .bucket(&self.config.bucket)
            .key(&resolved_key)
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

        debug!(key = %resolved_key, "Uploaded object to S3");
        Ok(())
    }

    async fn exists(&self, key: &str) -> LogrixResult<bool> {
        let resolved_key = self.config.resolve_key(key);
        let head_fut = self
            .config
            .client
            .head_object()
            .bucket(&self.config.bucket)
            .key(&resolved_key)
            .send();

        let resp = tokio::time::timeout(self.config.request_timeout, head_fut)
            .await
            .map_err(|_| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::BlobStore,
                    format!("Timeout heading S3 object at {resolved_key}"),
                )
            })?;

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

        let get_fut = self
            .config
            .client
            .get_object()
            .bucket(&self.config.bucket)
            .key(&resolved_key)
            .send();

        let resp = tokio::time::timeout(self.config.request_timeout, get_fut)
            .await
            .map_err(|_| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::BlobStore,
                    format!("Timeout getting S3 object at {resolved_key}"),
                )
            })?;

        match resp {
            Ok(output) => {
                let stream_fut = output.body.collect();
                let bytes = tokio::time::timeout(self.config.request_timeout, stream_fut)
                    .await
                    .map_err(|_| {
                        LogrixError::new(
                            ErrorClass::Transient,
                            ErrorSource::BlobStore,
                            format!("Timeout streaming S3 body for {resolved_key}"),
                        )
                    })?
                    .map_err(|e| {
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

        let del_fut = self
            .config
            .client
            .delete_object()
            .bucket(&self.config.bucket)
            .key(&resolved_key)
            .send();

        tokio::time::timeout(self.config.request_timeout, del_fut)
            .await
            .map_err(|_| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::BlobStore,
                    format!("Timeout deleting S3 object at {resolved_key}"),
                )
            })?
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
