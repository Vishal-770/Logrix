//! High-performance, fault-tolerant, and cost-aware RPC Gateway for Logrix.
//!
//! Features:
//! - Priority & Latency weighted multi-provider pool
//! - Seamless failover with exponential backoff circuit breakers
//! - Compute Unit (CU) budget ledger & automatic backfill pausing
//! - Single-flight concurrent request deduplication
//! - SQD / HyperSync bulk stream fast-path

pub mod budget;
pub mod bulk;
pub mod chain_port;
pub mod config;
pub mod gateway;
pub mod pool;
pub mod provider;
pub mod singleflight;

pub use budget::CuBudgetTracker;
pub use bulk::BulkStreamClient;
pub use config::GatewayConfig;
pub use gateway::RpcGateway;
pub use pool::ProviderPool;
pub use provider::{redact_rpc_url, ManagedProvider};
pub use singleflight::SingleFlight;
