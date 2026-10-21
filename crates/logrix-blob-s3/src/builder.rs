use crate::client::{S3Config, DEFAULT_REQUEST_TIMEOUT};
use aws_sdk_s3::{config::Builder as S3ConfigBuilder, Client as S3Client};
use logrix_core::error::LogrixResult;
use std::time::Duration;
use tracing::info;

/// Fluent builder for constructing S3Config with customizable parameters.
#[derive(Debug, Clone)]
pub struct S3ClientOptionsBuilder {
    bucket: String,
    key_prefix: Option<String>,
    endpoint_override: Option<String>,
    region: Option<String>,
    force_path_style: Option<bool>,
    request_timeout: Duration,
}

impl S3ClientOptionsBuilder {
    pub fn new(bucket: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            key_prefix: None,
            endpoint_override: None,
            region: None,
            force_path_style: None,
            request_timeout: DEFAULT_REQUEST_TIMEOUT,
        }
    }

    #[must_use]
    pub fn key_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.key_prefix = Some(prefix.into());
        self
    }

    #[must_use]
    pub fn endpoint_url(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint_override = Some(endpoint.into());
        self
    }

    #[must_use]
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    #[must_use]
    pub fn force_path_style(mut self, force: bool) -> Self {
        self.force_path_style = Some(force);
        self
    }

    #[must_use]
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub async fn build(self) -> LogrixResult<S3Config> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest());
        if let Some(ref endpoint) = self.endpoint_override {
            config_loader = config_loader.endpoint_url(endpoint);
        }
        if let Some(ref reg) = self.region {
            config_loader = config_loader.region(aws_sdk_s3::config::Region::new(reg.clone()));
        }

        let sdk_config = config_loader.load().await;
        let mut s3_builder = S3ConfigBuilder::from(&sdk_config);

        let force_path = self
            .force_path_style
            .unwrap_or(self.endpoint_override.is_some());
        if force_path {
            s3_builder = s3_builder.force_path_style(true);
        }

        let client = S3Client::from_conf(s3_builder.build());
        info!(bucket = %self.bucket, "Configured AWS S3 blob store client via builder");

        Ok(S3Config {
            client,
            bucket: self.bucket,
            key_prefix: self.key_prefix,
            request_timeout: self.request_timeout,
        })
    }
}
