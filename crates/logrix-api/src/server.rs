use crate::dynamic_schema::DynamicSchemaEngine;
use crate::schema::{build_schema, LogrixSchema};
use crate::schema_parser::SchemaDefinition;
use async_graphql::http::GraphiQLSource;
use async_graphql_axum::{GraphQLRequest, GraphQLResponse};
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
            .finish(),
    )
}

async fn health_check() -> &'static str {
    "OK"
}

/// Create router for static LogrixSchema.
pub fn create_router(schema: LogrixSchema) -> Router {
    Router::new()
        .route("/", get(graphiql))
        .route("/graphiql", get(graphiql))
        .route("/graphql", post(graphql_handler))
        .route("/healthz", get(health_check))
        .layer(Extension(schema))
}

/// Create router for dynamic async_graphql::dynamic::Schema.
pub fn create_dynamic_router(schema: async_graphql::dynamic::Schema) -> Router {
    Router::new()
        .route("/", get(graphiql))
        .route("/graphiql", get(graphiql))
        .route("/graphql", post(dynamic_graphql_handler))
        .route("/healthz", get(health_check))
        .layer(Extension(schema))
}

/// Start GraphQL API server with optional user-defined schema path (.graphql / .yaml).
pub async fn start_api_server(
    store: Arc<PostgresStore>,
    addr: SocketAddr,
) -> Result<(), std::io::Error> {
    start_api_server_with_schema(store, None, addr).await
}

/// Start GraphQL API server with optional dynamic schema definition path.
pub async fn start_api_server_with_schema(
    store: Arc<PostgresStore>,
    schema_path: Option<&str>,
    addr: SocketAddr,
) -> Result<(), std::io::Error> {
    info!("Starting Logrix GraphQL server on http://{}", addr);
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
            let dynamic_schema = DynamicSchemaEngine::build(&schema_def, store.clone())
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
            create_dynamic_router(dynamic_schema)
        } else {
            info!("Schema path provided does not exist; using default core schema");
            create_router(build_schema(store))
        }
    } else {
        create_router(build_schema(store))
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
