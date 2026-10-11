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

async fn graphql_handler(schema: Extension<LogrixSchema>, req: GraphQLRequest) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn dynamic_graphql_handler(
    schema: Extension<async_graphql::dynamic::Schema>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

/// Create router for static LogrixSchema.
pub fn create_router(schema: LogrixSchema) -> Router {
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
}

/// Create router for dynamic schema with optional WebSocket subscription endpoint.
pub fn create_dynamic_router(
    schema: async_graphql::dynamic::Schema,
    broadcaster: Option<Arc<SubscriptionBroadcaster>>,
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
}

/// Configuration for the Logrix GraphQL API server and query guardrails.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub max_depth: usize,
    pub max_complexity: usize,
    pub default_limit: usize,
    pub max_limit: usize,
    pub query_timeout_secs: u64,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            max_depth: 7,
            max_complexity: 200,
            default_limit: 100,
            max_limit: 1000,
            query_timeout_secs: 5,
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
            let dynamic_schema = DynamicSchemaEngine::build_with_config(
                &schema_def,
                store.clone(),
                broadcaster.clone(),
                conf.max_depth,
                conf.max_complexity,
            )
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            create_dynamic_router(dynamic_schema, broadcaster)
        } else {
            info!("Schema path does not exist; using core schema");
            create_router(build_schema(store.clone()))
        }
    } else {
        create_router(build_schema(store.clone()))
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
