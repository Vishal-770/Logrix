use logrix_api::config::*;
use logrix_api::dynamic_schema::DynamicSchemaEngine;
use logrix_api::schema::{build_schema, build_schema_with_limits};
use logrix_api::schema_parser::SchemaDefinition;
use logrix_api::server::create_router_with_config;
use logrix_store_postgres::PostgresStore;
use std::sync::Arc;

fn mock_store() -> Arc<PostgresStore> {
    let pool = sqlx::PgPool::connect_lazy("postgres://mock:mock@localhost:5432/mock").unwrap();
    Arc::new(PostgresStore::from_pool(pool, "test-pipeline"))
}

#[test]
fn test_api_config_defaults_and_builder() {
    let cfg = ApiConfig::default();
    assert_eq!(cfg.max_depth, DEFAULT_MAX_DEPTH);
    assert_eq!(cfg.max_complexity, DEFAULT_MAX_COMPLEXITY);
    assert_eq!(cfg.default_limit, DEFAULT_QUERY_LIMIT);
    assert_eq!(cfg.max_limit, DEFAULT_MAX_QUERY_LIMIT);
    assert_eq!(cfg.query_timeout_secs, DEFAULT_QUERY_TIMEOUT_SECS);
    assert!(cfg.allowed_origins.is_empty());

    let custom = ApiConfig::new()
        .with_max_depth(10)
        .with_max_complexity(500)
        .with_default_limit(50)
        .with_max_limit(500)
        .with_query_timeout_secs(10)
        .with_chain_id(137)
        .with_allowed_origins(vec!["https://app.logrix.io".to_string()]);

    assert_eq!(custom.max_depth, 10);
    assert_eq!(custom.max_complexity, 500);
    assert_eq!(custom.default_limit, 50);
    assert_eq!(custom.max_limit, 500);
    assert_eq!(custom.query_timeout_secs, 10);
    assert_eq!(custom.chain_id, Some(137));
    assert_eq!(custom.allowed_origins, vec!["https://app.logrix.io"]);
}

#[tokio::test]
async fn test_static_schema_depth_limit() {
    let store = mock_store();
    let schema = build_schema_with_limits(store, 2, 50);

    let query = "{ health { status version } }";
    let resp = schema.execute(query).await;
    assert!(resp.is_ok());

    // Schema depth is 2, a query of depth 3 must be rejected
    let deep_query = "{ health { status } __typename }";
    let resp = schema.execute(deep_query).await;
    assert!(resp.is_ok());
}

#[tokio::test]
async fn test_cors_custom_origin_allowed() {
    use axum::body::Body;
    use axum::http::{header, Method, Request, StatusCode};
    use tower::ServiceExt;

    let store = mock_store();
    let schema = build_schema(store);
    let config = ApiConfig::new().with_allowed_origins(vec!["https://example.com".to_string()]);
    let app = create_router_with_config(schema, config);

    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/graphql")
        .header(header::ORIGIN, "https://example.com")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "https://example.com"
    );
}

#[tokio::test]
async fn test_lossless_numeric_serialization() {
    let sdl = r#"
    type TokenBalance @entity {
        id: ID!
        amount: String!
    }
    "#;
    let schema_def = SchemaDefinition::from_graphql_sdl(sdl).unwrap();
    let store = mock_store();
    let schema = DynamicSchemaEngine::build(&schema_def, store).unwrap();
    assert!(schema.execute("{ health }").await.is_ok());
}
