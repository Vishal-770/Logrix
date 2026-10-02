use super::error_class::ErrorClass;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Subsystem source where the error originated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorSource {
    ChainRpc,
    Queue,
    Database,
    BlobStore,
    Plugin,
    Config,
    Reconciler,
    Internal,
}

/// Unified error type carried across Logrix.
///
/// Encapsulates the canonical `ErrorClass`, origin `ErrorSource`, message, and optional context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogrixError {
    pub class: ErrorClass,
    pub source: ErrorSource,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
}

impl LogrixError {
    /// Create a new LogrixError with class, source, and description.
    pub fn new(class: ErrorClass, source: ErrorSource, message: impl Into<String>) -> Self {
        Self {
            class,
            source,
            message: message.into(),
            chain_id: None,
            block_number: None,
            job_id: None,
        }
    }

    /// Attach chain ID to error context.
    #[must_use]
    pub fn with_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    /// Attach block number to error context.
    #[must_use]
    pub fn with_block_number(mut self, block_number: u64) -> Self {
        self.block_number = Some(block_number);
        self
    }

    /// Attach job ID to error context.
    #[must_use]
    pub fn with_job_id(mut self, job_id: impl Into<String>) -> Self {
        self.job_id = Some(job_id.into());
        self
    }

    // Helper constructors for canonical error classes
    pub fn transient(source: ErrorSource, message: impl Into<String>) -> Self {
        Self::new(ErrorClass::Transient, source, message)
    }

    pub fn rate_limited(
        source: ErrorSource,
        message: impl Into<String>,
        retry_after: Option<std::time::Duration>,
    ) -> Self {
        Self::new(ErrorClass::RateLimited { retry_after }, source, message)
    }

    pub fn shrinkable(source: ErrorSource, message: impl Into<String>) -> Self {
        Self::new(ErrorClass::Shrinkable, source, message)
    }

    pub fn permanent(source: ErrorSource, message: impl Into<String>) -> Self {
        Self::new(ErrorClass::Permanent, source, message)
    }

    pub fn integrity(source: ErrorSource, message: impl Into<String>) -> Self {
        Self::new(ErrorClass::Integrity, source, message)
    }

    pub fn fatal(source: ErrorSource, message: impl Into<String>) -> Self {
        Self::new(ErrorClass::Fatal, source, message)
    }
}

impl fmt::Display for LogrixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}::{:?}] {}", self.source, self.class, self.message)?;
        if let Some(chain) = self.chain_id {
            write!(f, " (chain: {chain})")?;
        }
        if let Some(block) = self.block_number {
            write!(f, " (block: {block})")?;
        }
        if let Some(job) = &self.job_id {
            write!(f, " (job: {job})")?;
        }
        Ok(())
    }
}

impl std::error::Error for LogrixError {}

/// Standard Result alias for Logrix operations.
pub type LogrixResult<T> = Result<T, LogrixError>;
