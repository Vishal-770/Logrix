//! GraphQL and HTTP API layer for Logrix.
//!
//! Provides high-performance sub-millisecond query access, interactive GraphiQL playground,
//! and Kubernetes liveness/readiness probes.

pub mod schema;
pub mod server;

pub use schema::{build_schema, CheckpointStatus, HealthStatus, LogrixSchema, Transfer};
pub use server::{create_router, start_api_server};
