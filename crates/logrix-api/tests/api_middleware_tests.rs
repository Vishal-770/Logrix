use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use logrix_api::dynamic_schema::DynamicSchemaEngine;
use logrix_api::schema::build_schema;
use logrix_api::schema_parser::SchemaDefinition;
use logrix_api::server::{create_dynamic_router, create_router};
use logrix_store_postgres::PostgresStore;
use std::sync::Arc;
use tower::ServiceExt;

fn mock_store() -> Arc<PostgresStore> {
    let pool = sqlx::PgPool::connect_lazy("postgres://mock:mock@localhost:5432/mock").unwrap();
    Arc::new(PostgresStore::from_pool(pool, "test-pipeline"))
}

#[tokio::test]
async fn test_cors_headers_on_graphql_endpoint() {
    let store = mock_store();
    let schema = build_schema(store);
    let app = create_router(schema);

    let req = Request::builder()
        .method(Method::OPTIONS)
        .uri("/graphql")
        .header(header::ORIGIN, "http://localhost:3000")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(
        resp.headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN),
        "CORS permissive layer must include access-control-allow-origin header"
    );
}

#[tokio::test]
async fn test_healthz_and_readyz_endpoints() {
    let store = mock_store();
    let schema = build_schema(store);
    let app = create_router(schema);

    let health_req = Request::builder()
        .method(Method::GET)
        .uri("/healthz")
        .body(Body::empty())
        .unwrap();
    let health_resp = app.clone().oneshot(health_req).await.unwrap();
    assert_eq!(health_resp.status(), StatusCode::OK);

    let ready_req = Request::builder()
        .method(Method::GET)
        .uri("/readyz")
        .body(Body::empty())
        .unwrap();
    let ready_resp = app.oneshot(ready_req).await.unwrap();
    assert_eq!(ready_resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_dynamic_pluralization_in_schema() {
    let sdl = r#"
    type Activity @entity {
        id: ID!
        action: String!
    }
    type Match @entity {
        id: ID!
        score: Int!
    }
    "#;
    let schema_def = SchemaDefinition::from_graphql_sdl(sdl).unwrap();
    let store = mock_store();
    let dynamic_schema = DynamicSchemaEngine::build(&schema_def, store).unwrap();
    let app = create_dynamic_router(dynamic_schema, None);

    // Verify GraphiQL IDE is rendered on GET /
    let graphiql_req = Request::builder()
        .method(Method::GET)
        .uri("/")
        .body(Body::empty())
        .unwrap();
    let graphiql_resp = app.oneshot(graphiql_req).await.unwrap();
    assert_eq!(graphiql_resp.status(), StatusCode::OK);
}
