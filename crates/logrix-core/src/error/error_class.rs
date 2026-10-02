use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Canonical classification of every error across Logrix.
///
/// High-level components look only at this class, never at provider-specific text.
/// The class directly drives the recovery policy (retry, park, halve window, rollback, or halt).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    /// Transient blip (network timeout, 5xx, node temporarily behind).
    /// Action: Retry with exponential backoff and jitter within retry budget.
    Transient,

    /// Rate limited (HTTP 429, "compute units exhausted", "usage limit reached").
    /// Action: Honor Retry-After if provided, slow this provider down, route elsewhere.
    RateLimited {
        #[serde(skip_serializing_if = "Option::is_none")]
        retry_after: Option<Duration>,
    },

    /// Query chunk too large ("range too large", "too many results", response size cap).
    /// Action: Halve the query block window and retry immediately; persist learned limit.
    Shrinkable,

    /// Permanent data/logic error for this specific job (undecodable event, bad handler arithmetic).
    /// Action: Do NOT retry indefinitely; park in Dead-Letter Queue (DLQ) with context.
    Permanent,

    /// Blockchain consistency failure (parent-hash mismatch, missing logs, reorg during write).
    /// Action: Stop partition, execute rollback or gap reconciliation, then resume.
    Integrity,

    /// Unrecoverable system failure (wrong chain ID, revoked RPC credentials, invalid schema).
    /// Action: Halt the component immediately and alert loudly; never retry.
    Fatal,
}

impl ErrorClass {
    /// Whether this error class is considered retryable by the resilience engine.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Transient | Self::RateLimited { .. } | Self::Shrinkable
        )
    }

    /// Whether this error represents a permanent failure that should be parked in the DLQ.
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        matches!(self, Self::Permanent)
    }

    /// Whether this error represents an unrecoverable failure requiring system halt.
    #[must_use]
    pub fn is_fatal(&self) -> bool {
        matches!(self, Self::Fatal)
    }

    /// Whether this error indicates a block hash / reorg integrity problem.
    #[must_use]
    pub fn is_integrity(&self) -> bool {
        matches!(self, Self::Integrity)
    }
}
