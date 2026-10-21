use aws_sdk_s3::{config::Builder as S3ConfigBuilder, Client as S3Client};
use logrix_core::error::LogrixResult;
use std::time::Duration;
use tracing::info;

/// Default timeout boundary for S3 object operations.
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// S3 configuration and client wrapper.
#[derive(Clone)]
pub struct S3Config {
    pub client: S3Client,
    pub bucket: String,
    pub key_prefix: Option<String>,
    pub request_timeout: Duration,
}

impl S3Config {
    pub fn new(client: S3Client, bucket: impl Into<String>, key_prefix: Option<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            key_prefix,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    /// Set custom request timeout.
    #[must_use]
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Initialize from environment variables or custom endpoint (LocalStack, MinIO, R2).
    pub async fn from_env(
        bucket: impl Into<String>,
        key_prefix: Option<String>,
        endpoint_override: Option<&str>,
    ) -> LogrixResult<Self> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        if let Some(endpoint) = endpoint_override {
            config_loader = config_loader.endpoint_url(endpoint);
        }

        let sdk_config = config_loader.load().await;
        let mut s3_builder = S3ConfigBuilder::from(&sdk_config);

        // Path-style addressing is mandatory for LocalStack, MinIO, and local Ceph clusters.
        if endpoint_override.is_some() {
            s3_builder = s3_builder.force_path_style(true);
        }

        let client = S3Client::from_conf(s3_builder.build());
        let bucket_name = bucket.into();
        info!(bucket = %bucket_name, "Configured AWS S3 blob store client");

        Ok(Self {
            client,
            bucket: bucket_name,
            key_prefix,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        })
    }

    /// Format full S3 object key with optional prefix.
    #[must_use]
    pub fn resolve_key(&self, key: &str) -> String {
        match &self.key_prefix {
            Some(prefix) if !prefix.is_empty() => {
                let trimmed = prefix.trim_end_matches('/');
                format!("{trimmed}/{key}")
            }
            _ => key.to_string(),
        }
    }

    /// Create a fluent builder for configuring S3 client and bucket.
    pub fn builder(bucket: impl Into<String>) -> crate::builder::S3ClientOptionsBuilder {
        crate::builder::S3ClientOptionsBuilder::new(bucket)
    }
}
