use crate::schema::{build_schema, LogrixSchema};
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
use std::sync::Arc;
use tracing::info;

async fn graphql_handler(schema: Extension<LogrixSchema>, req: GraphQLRequest) -> GraphQLResponse {
    schema.execute(req.into_inner()).await.into()
}

async fn graphiql() -> impl IntoResponse {
    Html(GraphiQLSource::build().endpoint("/graphql").finish())
}

async fn health_check() -> &'static str {
    "OK"
}

/// Create the Axum HTTP router with GraphiQL playground and healthz probe.
pub fn create_router(schema: LogrixSchema) -> Router {
    Router::new()
        .route("/", get(graphiql))
        .route("/graphql", post(graphql_handler))
        .route("/healthz", get(health_check))
        .layer(Extension(schema))
}

/// Start the Logrix GraphQL API server on the specified address.
pub async fn start_api_server(
    store: Arc<PostgresStore>,
    addr: SocketAddr,
) -> Result<(), std::io::Error> {
    let schema = build_schema(store);
    let app = create_router(schema);

    info!("Starting GraphQL server on http://{}", addr);
    info!("GraphiQL interactive UI available at http://{}/", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await
}
