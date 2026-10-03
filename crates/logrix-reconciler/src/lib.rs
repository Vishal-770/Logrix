//! Reorganization and self-healing reconciler engine for Logrix.
//!
//! Features:
//! - Rolling ring-buffer parent-hash continuity check
//! - Common ancestor fork-point discovery
//! - Atomic database state rollback and audit-trail soft-deleting (`is_reverted = TRUE`)
//! - Self-healing continuous gap reconciler
//! - `event.reverted` webhook dispatcher with exponential backoff & DLQ routing

pub mod buffer;
pub mod detector;
pub mod reconciler;
pub mod rollback;
pub mod webhook;

pub use buffer::RollingBlockBuffer;
pub use detector::{ContinuityStatus, ReorgDetector};
pub use reconciler::GapReconciler;
pub use rollback::ReorgHandler;
pub use webhook::WebhookDispatcher;
