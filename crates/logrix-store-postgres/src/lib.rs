//! PostgreSQL storage adapter implementing `StorePort`.
//!
//! Provides high-throughput vectorized UNNEST batch inserts, atomic transaction
//! boundaries across events and checkpoints, and automatic reorg rollbacks.

pub mod entities;
pub mod events;
pub mod model;
pub mod rollback;
pub mod store;
pub mod transfers;

pub use model::{DynamicEntityRecord, EntityInsert, TokenTransfer, TransferFilter};
pub use store::PostgresStore;
