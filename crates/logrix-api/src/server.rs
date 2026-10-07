use crate::dynamic_schema::DynamicSchemaEngine;
use crate::schema::{build_schema, LogrixSchema};
use crate::schema_parser::SchemaDefinition;
use crate::subscriptions::SubscriptionBroadcaster;
use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use axum::{
    extract::Extension,
    response::{Html, IntoResponse},
    routing::{get, post},
    Router,
};
use logrix_store_postgres::PostgresStore;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
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

async fn graphiql() -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/ws")
            .title("Logrix Studio - GraphQL IDE")
            .finish(),
    )
}

async fn health_check() -> &'static str {
    "OK"
}

async fn prometheus_metrics() -> impl IntoResponse {
    let body = "# HELP logrix_api_health API server health status\n\
# TYPE logrix_api_health gauge\n\
logrix_api_health 1\n\
# HELP logrix_api_version Build version info\n\
# TYPE logrix_api_version info\n\
logrix_api_version{version=\"0.1.0\"} 1\n";
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        body,
    )
}

/// Create router for static LogrixSchema.
pub fn create_router(schema: LogrixSchema) -> Router {
    Router::new()
        .route("/", get(graphiql))
        .route("/graphiql", get(graphiql))
        .route("/graphql", post(graphql_handler))
        .route("/healthz", get(health_check))
        .route("/readyz", get(health_check))
        .route("/metrics", get(prometheus_metrics))
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
        .route("/readyz", get(health_check))
        .route("/metrics", get(prometheus_metrics));

    // Wire WebSocket subscriptions when a broadcaster is present
    if broadcaster.is_some() {
        router = router.route_service("/ws", GraphQLSubscription::new(schema.clone()));
    }

    router.layer(Extension(schema))
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
    info!("Starting Logrix GraphQL server on http://{}", addr);
    info!(
        max_depth = conf.max_depth,
        max_complexity = conf.max_complexity,
        "Configured query guardrails"
    );
    if broadcaster.is_some() {
        info!(
            "GraphQL subscriptions enabled via WebSocket at ws://{}/ws",
            addr
        );
    }
    info!(
        "GraphiQL interactive UI available at http://{}/ and http://{}/graphiql",
        addr, addr
    );

    let app = if let Some(path_str) = schema_path {
        let p = Path::new(path_str);
        if p.exists() {
            info!(path = path_str, "Loading dynamic GraphQL entity schema");
            let schema_def = SchemaDefinition::from_file(p)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
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
            info!("Schema path provided does not exist; using default core schema");
            create_router(build_schema(store.clone()))
        }
    } else {
        create_router(build_schema(store.clone()))
    };

    let mut known_events = std::collections::HashSet::new();
    known_events.insert("reorg".to_string());
    known_events.insert("checkpoint".to_string());
    let webhook_state = crate::routes::WebhookApiState::new(
        logrix_webhook::WebhookStore::new(store.pool().clone()),
        known_events,
    );
    let app = app.merge(crate::routes::webhook_routes(webhook_state));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
