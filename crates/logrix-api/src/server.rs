use crate::config::ApiConfig;
use crate::dynamic_schema::DynamicSchemaEngine;
use crate::health::{graphiql, health_check, prometheus_metrics, readiness_check};
use crate::schema::{build_schema_with_limits, LogrixSchema};
use crate::schema_parser::SchemaDefinition;
use crate::subscriptions::SubscriptionBroadcaster;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use axum::{
    extract::Extension,
    http::{header, Method},
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

fn build_cors_layer(config: &ApiConfig) -> CorsLayer {
    if config.allowed_origins.is_empty() {
        CorsLayer::permissive()
    } else {
        let origins: Vec<_> = config
            .allowed_origins
            .iter()
            .filter_map(|o| o.parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT])
    }
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

pub fn create_router(schema: LogrixSchema) -> Router {
    create_router_with_config(schema, ApiConfig::default())
}

pub fn create_router_with_config(schema: LogrixSchema, config: ApiConfig) -> Router {
    let cors = build_cors_layer(&config);
    Router::new()
        .route("/", get(graphiql))
        .route("/graphiql", get(graphiql))
        .route("/graphql", post(graphql_handler))
        .route("/healthz", get(health_check))
        .route("/readyz", get(readiness_check))
        .route("/metrics", get(prometheus_metrics))
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(Extension(schema))
        .layer(Extension(config))
}

pub fn create_dynamic_router(
    schema: async_graphql::dynamic::Schema,
    broadcaster: Option<Arc<SubscriptionBroadcaster>>,
) -> Router {
    create_dynamic_router_with_config(schema, broadcaster, ApiConfig::default())
}

pub fn create_dynamic_router_with_config(
    schema: async_graphql::dynamic::Schema,
    broadcaster: Option<Arc<SubscriptionBroadcaster>>,
    config: ApiConfig,
) -> Router {
    let cors = build_cors_layer(&config);
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
        .layer(cors)
        .layer(CompressionLayer::new())
        .layer(Extension(schema))
        .layer(Extension(config))
}

pub async fn start_api_server(
    store: Arc<PostgresStore>,
    addr: SocketAddr,
) -> Result<(), std::io::Error> {
    start_api_server_with_schema(store, None, None, None, addr).await
}

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
            info!("Schema path does not exist; using core schema with limits");
            create_router_with_config(
                build_schema_with_limits(store.clone(), conf.max_depth, conf.max_complexity),
                conf,
            )
        }
    } else {
        create_router_with_config(
            build_schema_with_limits(store.clone(), conf.max_depth, conf.max_complexity),
            conf,
        )
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
