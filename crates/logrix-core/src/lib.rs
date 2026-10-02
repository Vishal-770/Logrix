//! Core domain primitives, error taxonomy, and abstract ports for Logrix.

pub mod domain;
pub mod error;
pub mod ports;

// Re-export core domain types
pub use domain::block::{BlockEnvelope, BlockRef, EnvelopeKind, EventLog};
pub use domain::chain::ChainId;
pub use domain::checkpoint::Checkpoint;
pub use domain::job::{BlockRangeJob, LiveBlockJob, MessageHandle, QueueMessage, QueueType};

// Re-export error types
pub use error::error_class::ErrorClass;
pub use error::logrix_error::{ErrorSource, LogrixError, LogrixResult};

// Re-export port traits
pub use ports::blob::BlobPort;
pub use ports::chain::ChainPort;
pub use ports::queue::QueuePort;
pub use ports::sink::SinkPort;
pub use ports::store::StorePort;
