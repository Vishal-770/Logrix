//! GraphQL and HTTP API layer for Logrix.
//!
//! Provides high-performance sub-millisecond query access, interactive GraphiQL playground,
//! dynamic schema generation, and Kubernetes liveness/readiness probes.

pub mod dynamic_schema;
pub mod filter_input;
pub mod list_resolver;
pub mod query_builder;
pub mod schema;
pub mod schema_parser;
pub mod server;
pub mod subscriptions;

pub use dynamic_schema::DynamicSchemaEngine;
pub use query_builder::{DynamicQueryParams, FieldFilter, FilterOperator};
pub use schema::{build_schema, CheckpointStatus, HealthStatus, LogrixSchema, Transfer};
pub use schema_parser::{EntityDef, FieldDef, FieldType, SchemaDefinition, SchemaParserError};
pub use server::{
    create_dynamic_router, create_router, start_api_server, start_api_server_with_schema,
};
pub use subscriptions::{EntityMutationEvent, SubscriptionBroadcaster};
