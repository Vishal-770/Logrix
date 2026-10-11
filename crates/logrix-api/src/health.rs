use async_graphql::http::GraphiQLSource;
use axum::{
    extract::Extension,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use logrix_store_postgres::PostgresStore;
use std::sync::Arc;

pub async fn graphiql() -> impl IntoResponse {
    Html(
        GraphiQLSource::build()
            .endpoint("/graphql")
            .subscription_endpoint("/ws")
            .title("Logrix Studio - GraphQL IDE")
            .finish(),
    )
}

pub async fn health_check() -> &'static str {
    "OK"
}

pub async fn readiness_check(
    store: Option<Extension<Arc<PostgresStore>>>,
) -> (StatusCode, &'static str) {
    if let Some(Extension(store)) = store {
        match sqlx::query("SELECT 1").execute(store.pool()).await {
            Ok(_) => (StatusCode::OK, "OK"),
            Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "DATABASE_UNAVAILABLE"),
        }
    } else {
        (StatusCode::OK, "OK")
    }
}

pub async fn prometheus_metrics() -> impl IntoResponse {
    let version = env!("CARGO_PKG_VERSION");
    let body = format!(
        "# HELP logrix_api_health API server health status\n# TYPE logrix_api_health gauge\nlogrix_api_health 1\n# HELP logrix_api_version Build version info\n# TYPE logrix_api_version info\nlogrix_api_version{{version=\"{version}\"}} 1\n"
    );
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        body,
    )
}
