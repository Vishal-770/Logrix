use crate::dynamic_schema::DynamicSchemaEngine;
use crate::health::{graphiql, health_check, prometheus_metrics, readiness_check};
use crate::schema::{build_schema, LogrixSchema};
use crate::schema_parser::SchemaDefinition;
use crate::subscriptions::SubscriptionBroadcaster;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use axum::{
    extract::Extension,
    routing::{get, post},
    Router,
};
use logrix_store_postgres::PostgresStore;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tracing::info;

#[inline]
fn timeout_err() -> GraphQLResponse {
    let err = async_graphql::ServerError::new("Query execution timed out", None);
    async_graphql::Response::from_errors(vec![err]).into()
}

async fn graphql_handler(
    schema: Extension<LogrixSchema>,
    conf: Option<Extension<ApiConfig>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let timeout_secs = conf.as_ref().map(|c| c.query_timeout_secs).unwrap_or(5);
    let dur = std::time::Duration::from_secs(timeout_secs);
    match tokio::time::timeout(dur, schema.execute(req.into_inner())).await {
        Ok(res) => res.into(),
        Err(_) => timeout_err(),
    }
}

async fn dynamic_graphql_handler(
    schema: Extension<async_graphql::dynamic::Schema>,
    conf: Option<Extension<ApiConfig>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let timeout_secs = conf.as_ref().map(|c| c.query_timeout_secs).unwrap_or(5);
    let dur = std::time::Duration::from_secs(timeout_secs);
    match tokio::time::timeout(dur, schema.execute(req.into_inner())).await {
        Ok(res) => res.into(),
        Err(_) => timeout_err(),
    }
}

/// Create router for static LogrixSchema.
pub fn create_router(schema: LogrixSchema) -> Router {
    create_router_with_config(schema, ApiConfig::default())
}

/// Create router for static LogrixSchema with ApiConfig.
pub fn create_router_with_config(schema: LogrixSchema, config: ApiConfig) -> Router {
    Router::new()
        .route("/", get(graphiql))
        .route("/graphiql", get(graphiql))
        .route("/graphql", post(graphql_handler))
        .route("/healthz", get(health_check))
        .route("/readyz", get(readiness_check))
        .route("/metrics", get(prometheus_metrics))
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .layer(Extension(schema))
        .layer(Extension(config))
}

/// Create router for dynamic schema with optional WebSocket subscription endpoint.
pub fn create_dynamic_router(
    schema: async_graphql::dynamic::Schema,
    broadcaster: Option<Arc<SubscriptionBroadcaster>>,
) -> Router {
    create_dynamic_router_with_config(schema, broadcaster, ApiConfig::default())
}

/// Create router for dynamic schema with optional WebSocket subscription endpoint and ApiConfig.
pub fn create_dynamic_router_with_config(
    schema: async_graphql::dynamic::Schema,
    broadcaster: Option<Arc<SubscriptionBroadcaster>>,
    config: ApiConfig,
) -> Router {
    let mut router = Router::new()
        .route("/", get(graphiql))
        .route("/graphiql", get(graphiql))
        .route("/graphql", post(dynamic_graphql_handler))
        .route("/healthz", get(health_check))
        .route("/readyz", get(readiness_check))
        .route("/metrics", get(prometheus_metrics));

    if broadcaster.is_some() {
        router = router.route_service("/ws", GraphQLSubscription::new(schema.clone()));
    }
    router
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .layer(Extension(schema))
        .layer(Extension(config))
}

/// Configuration for the Logrix GraphQL API server and query guardrails.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub max_depth: usize,
    pub max_complexity: usize,
    pub default_limit: usize,
    pub max_limit: usize,
    pub query_timeout_secs: u64,
    pub chain_id: Option<u64>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            max_depth: 7,
            max_complexity: 200,
            default_limit: 100,
            max_limit: 1000,
            query_timeout_secs: 5,
            chain_id: None,
        }
    }
}

/// Start GraphQL API server with optional user-defined schema path (.graphql / .yaml).
pub async fn start_api_server(
    store: Arc<PostgresStore>,
    addr: SocketAddr,
) -> Result<(), std::io::Error> {
    start_api_server_with_schema(store, None, None, None, addr).await
}

/// Start GraphQL API server with optional dynamic schema and subscription broadcaster.
pub async fn start_api_server_with_schema(
    store: Arc<PostgresStore>,
    schema_path: Option<&str>,
    config: Option<ApiConfig>,
    broadcaster: Option<Arc<SubscriptionBroadcaster>>,
    addr: SocketAddr,
) -> Result<(), std::io::Error> {
    let conf = config.unwrap_or_default();
    info!(addr = %addr, max_depth = conf.max_depth, max_complexity = conf.max_complexity, "Starting Logrix GraphQL server");
    if broadcaster.is_some() {
        info!(addr = %addr, "GraphQL subscriptions enabled via WebSocket at /ws");
    }
    info!(addr = %addr, "GraphiQL UI available at / and /graphiql");

    let mut known_events = std::collections::HashSet::new();
    known_events.insert("reorg".to_string());
    known_events.insert("checkpoint".to_string());

    let app = if let Some(path_str) = schema_path {
        let p = Path::new(path_str);
        if p.exists() {
            info!(path = path_str, "Loading dynamic GraphQL entity schema");
            let schema_def = SchemaDefinition::from_file(p)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            for entity in &schema_def.entities {
                known_events.insert(entity.name.clone());
            }
            let dynamic_schema = DynamicSchemaEngine::build_with_api_config(
                &schema_def,
                store.clone(),
                broadcaster.clone(),
                &conf,
            )
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            create_dynamic_router_with_config(dynamic_schema, broadcaster, conf)
        } else {
            info!("Schema path does not exist; using core schema");
            create_router_with_config(build_schema(store.clone()), conf)
        }
    } else {
        create_router_with_config(build_schema(store.clone()), conf)
    };

    let webhook_state = crate::routes::WebhookApiState::new(
        logrix_webhook::WebhookStore::new(store.pool().clone()),
        known_events,
    );
    let app = app
        .merge(crate::routes::webhook_routes(webhook_state))
        .layer(Extension(store));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
