//! GraphQL and HTTP API layer for Logrix.
//!
//! Provides high-performance sub-millisecond query access, interactive GraphiQL playground,
//! dynamic schema generation, and Kubernetes liveness/readiness probes.

pub mod config;
pub mod dynamic_schema;
pub mod entity_object;
pub mod filter_input;
pub mod health;
pub mod list_resolver;
pub mod pluralize;
pub mod query_builder;
pub mod routes;
pub mod schema;
pub mod schema_parser;
pub mod schema_types;
pub mod server;
pub mod subscriptions;
pub mod validation;

pub use config::{
    ApiConfig, DEFAULT_MAX_COMPLEXITY, DEFAULT_MAX_DEPTH, DEFAULT_MAX_QUERY_LIMIT,
    DEFAULT_QUERY_LIMIT, DEFAULT_QUERY_TIMEOUT_SECS, DEFAULT_SUBSCRIPTION_CHANNEL_CAPACITY,
};
pub use dynamic_schema::DynamicSchemaEngine;
pub use pluralize::pluralize_entity_name;
pub use query_builder::{DynamicQueryParams, FieldFilter, FilterOperator};
pub use schema::{
    build_schema, build_schema_with_limits, CheckpointStatus, HealthStatus, LogrixSchema, Transfer,
};
pub use schema_parser::SchemaParserError;
pub use schema_types::{DerivedFrom, EntityDef, FieldDef, FieldType, SchemaDefinition};
pub use server::{
    create_dynamic_router, create_router, start_api_server, start_api_server_with_schema,
};
pub use subscriptions::{EntityMutationEvent, SubscriptionBroadcaster};
