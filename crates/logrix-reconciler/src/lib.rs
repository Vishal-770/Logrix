//! Reorganization and self-healing reconciler engine for Logrix.
//!
//! Features:
//! - Rolling ring-buffer parent-hash continuity check
//! - Common ancestor fork-point discovery
//! - Atomic database state rollback and audit-trail soft-deleting (`is_reverted = TRUE`)
//! - Self-healing continuous gap reconciler
//! - `event.reverted` webhook dispatcher with exponential backoff & DLQ routing

pub mod ancestor;
pub mod buffer;
pub mod detector;
pub mod reconciler;
pub mod reconciler_loop;
pub mod rollback;

pub use buffer::{RollingBlockBuffer, DEFAULT_RING_BUFFER_CAPACITY};
pub use detector::{ContinuityStatus, ReorgDetector, DEFAULT_MAX_REORG_DEPTH};
pub use reconciler::{GapReconciler, DEFAULT_HEAD_OFFSET, DEFAULT_LAG_THRESHOLD};
pub use rollback::ReorgHandler;
