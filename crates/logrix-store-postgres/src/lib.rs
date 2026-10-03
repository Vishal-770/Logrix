//! PostgreSQL storage adapter implementing `StorePort`.
//!
//! Provides high-throughput vectorized UNNEST batch inserts, atomic transaction
//! boundaries across events and checkpoints, and automatic reorg rollbacks.

pub mod model;
pub mod store;

pub use model::{DynamicEntityRecord, EntityInsert, TokenTransfer, TransferFilter};
pub use store::PostgresStore;
