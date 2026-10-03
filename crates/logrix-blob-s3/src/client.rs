use aws_sdk_s3::{config::Builder as S3ConfigBuilder, Client as S3Client};
use logrix_core::error::LogrixResult;
use tracing::info;

/// S3 configuration and client wrapper.
#[derive(Clone)]
pub struct S3Config {
    pub client: S3Client,
    pub bucket: String,
    pub key_prefix: Option<String>,
}

impl S3Config {
    pub fn new(client: S3Client, bucket: impl Into<String>, key_prefix: Option<String>) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            key_prefix,
        }
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
}
