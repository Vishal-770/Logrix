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

pub async fn prometheus_metrics(store: Option<Extension<Arc<PostgresStore>>>) -> impl IntoResponse {
    let version = env!("CARGO_PKG_VERSION");
    let mut metrics = format!(
        "# HELP logrix_api_health API server health status\n\
         # TYPE logrix_api_health gauge\n\
         logrix_api_health 1\n\
         # HELP logrix_api_version Build version info\n\
         # TYPE logrix_api_version info\n\
         logrix_api_version{{version=\"{version}\"}} 1\n"
    );

    if let Some(Extension(store)) = store {
        let pool = store.pool();
        let pool_size = pool.size();
        let pool_idle = pool.num_idle();

        metrics.push_str(&format!(
            "# HELP logrix_db_pool_connections Database connection pool stats\n\
             # TYPE logrix_db_pool_connections gauge\n\
             logrix_db_pool_connections{{state=\"total\"}} {pool_size}\n\
             logrix_db_pool_connections{{state=\"idle\"}} {pool_idle}\n"
        ));

        let cp_row: Option<(i64,)> = sqlx::query_as(
            "SELECT last_indexed_block FROM checkpoints ORDER BY updated_at DESC LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        let indexed_block = cp_row.map(|(b,)| b).unwrap_or(0);
        metrics.push_str(&format!(
            "# HELP logrix_last_indexed_block Highest block number indexed into storage\n\
             # TYPE logrix_last_indexed_block gauge\n\
             logrix_last_indexed_block {indexed_block}\n"
        ));

        let event_cnt: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM event_logs")
            .fetch_one(pool)
            .await
            .unwrap_or((0,));
        metrics.push_str(&format!(
            "# HELP logrix_events_indexed_total Total raw and decoded event logs indexed\n\
             # TYPE logrix_events_indexed_total counter\n\
             logrix_events_indexed_total {}\n",
            event_cnt.0
        ));

        let reorg_cnt: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM event_logs WHERE is_reverted = TRUE")
                .fetch_one(pool)
                .await
                .unwrap_or((0,));
        metrics.push_str(&format!(
            "# HELP logrix_reorg_events_total Total event logs marked reverted by reorg rollbacks\n\
             # TYPE logrix_reorg_events_total counter\n\
             logrix_reorg_events_total {}\n",
            reorg_cnt.0
        ));

        let entity_cnt: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM logrix_entities")
            .fetch_one(pool)
            .await
            .unwrap_or((0,));
        metrics.push_str(&format!(
            "# HELP logrix_entities_total Total dynamic custom domain entities stored\n\
             # TYPE logrix_entities_total gauge\n\
             logrix_entities_total {}\n",
            entity_cnt.0
        ));
    }

    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        metrics,
    )
}
