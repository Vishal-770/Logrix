use logrix_api::dynamic_schema::DynamicSchemaEngine;
use logrix_api::schema_parser::SchemaDefinition;
use logrix_api::subscriptions::{EntityMutationEvent, SubscriptionBroadcaster};
use logrix_store_postgres::PostgresStore;
use std::sync::Arc;

#[tokio::test]
async fn test_dynamic_graphql_schema_execution_and_health() {
    let sdl = r#"
    type Account @entity {
        id: ID!
        balance: String!
    }
    "#;
    let schema_def = SchemaDefinition::from_graphql_sdl(sdl).unwrap();

    let pool = sqlx::PgPool::connect_lazy("postgres://mock:mock@localhost:5432/mock").unwrap();
    let store = Arc::new(PostgresStore::from_pool(pool, "test-pipeline"));

    let schema = DynamicSchemaEngine::build(&schema_def, store).expect("Schema builds cleanly");

    // Execute health query
    let req = async_graphql::Request::new("{ health }");
    let resp = schema.execute(req).await;

    assert!(resp.is_ok(), "Response should be ok: {:?}", resp.errors);
    let data = resp.data.into_json().unwrap();
    assert_eq!(data["health"], "OK");
}

#[tokio::test]
async fn test_dynamic_graphql_query_depth_guardrail() {
    let sdl = r#"
    type Node @entity {
        id: ID!
        child: Node
    }
    "#;
    let schema_def = SchemaDefinition::from_graphql_sdl(sdl).unwrap();
    let pool = sqlx::PgPool::connect_lazy("postgres://mock:mock@localhost:5432/mock").unwrap();
    let store = Arc::new(PostgresStore::from_pool(pool, "test-pipeline"));

    let schema = DynamicSchemaEngine::build(&schema_def, store).expect("Schema builds cleanly");

    // Query with depth > 7 should be rejected by depth limit guardrail
    let deep_query = "{ node(id: \"1\") { child { child { child { child { child { child { child { id } } } } } } } } }";
    let req = async_graphql::Request::new(deep_query);
    let resp = schema.execute(req).await;

    assert!(
        !resp.errors.is_empty(),
        "Deep query must trigger validation errors"
    );
    assert!(
        resp.errors[0].message.to_lowercase().contains("deep")
            || resp.errors[0].message.to_lowercase().contains("depth"),
        "Error message should mention depth limit: {}",
        resp.errors[0].message
    );
}

#[tokio::test]
async fn test_subscription_broadcaster_pub_sub() {
    let broadcaster = SubscriptionBroadcaster::new(16);
    let mut rx = broadcaster.subscribe();

    let event = EntityMutationEvent {
        chain_id: 1,
        entity_type: "Account".to_string(),
        entity_id: "0x123".to_string(),
        block_number: 100,
        data: serde_json::json!({ "balance": "5000" }),
    };

    broadcaster.broadcast(event.clone());

    let received = rx.recv().await.expect("Receive broadcast event");
    assert_eq!(received.chain_id, 1);
    assert_eq!(received.entity_type, "Account");
    assert_eq!(received.entity_id, "0x123");
    assert_eq!(received.data["balance"], "5000");
}

#[tokio::test]
async fn test_dynamic_graphql_custom_configurable_limits() {
    let sdl = r#"
    type Node @entity {
        id: ID!
        child: Node
    }
    "#;
    let schema_def = SchemaDefinition::from_graphql_sdl(sdl).unwrap();
    let pool = sqlx::PgPool::connect_lazy("postgres://mock:mock@localhost:5432/mock").unwrap();
    let store = Arc::new(PostgresStore::from_pool(pool, "test-pipeline"));

    // Build with tight custom limit of depth 3
    let schema = DynamicSchemaEngine::build_with_limits(&schema_def, store, 3, 50)
        .expect("Schema builds cleanly");

    // Depth 4 query must be rejected under limit of 3
    let query = "{ node(id: \"1\") { child { child { child { id } } } } }";
    let resp = schema.execute(async_graphql::Request::new(query)).await;

    assert!(!resp.errors.is_empty());
    assert!(
        resp.errors[0].message.to_lowercase().contains("deep")
            || resp.errors[0].message.to_lowercase().contains("depth")
    );
}

#[tokio::test]
async fn test_subscription_chain_id_filtering() {
    let sdl = r#"
    type Account @entity {
        id: ID!
        balance: String!
    }
    "#;
    let schema_def = SchemaDefinition::from_graphql_sdl(sdl).unwrap();
    let pool = sqlx::PgPool::connect_lazy("postgres://mock:mock@localhost:5432/mock").unwrap();
    let store = Arc::new(PostgresStore::from_pool(pool, "test-pipeline"));
    let broadcaster = Arc::new(SubscriptionBroadcaster::new(16));

    let schema = DynamicSchemaEngine::build_with_config(
        &schema_def,
        store,
        Some(broadcaster.clone()),
        15,
        1000,
    )
    .expect("Schema builds cleanly");

    let sub_req =
        async_graphql::Request::new("subscription { onAccount(chainId: 1) { id balance } }");
    let mut stream = schema.execute_stream(sub_req);

    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let bc = broadcaster.clone();
    tokio::spawn(async move {
        let _ = ready_rx.await;
        for _ in 0..100 {
            if bc.receiver_count() > 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        // Broadcast for chain 42161 (should be filtered out)
        bc.broadcast(EntityMutationEvent {
            chain_id: 42161,
            entity_type: "Account".to_string(),
            entity_id: "0xother".to_string(),
            block_number: 10,
            data: serde_json::json!({ "id": "0xother", "balance": "100" }),
        });

        // Broadcast for chain 1 (should match)
        bc.broadcast(EntityMutationEvent {
            chain_id: 1,
            entity_type: "Account".to_string(),
            entity_id: "0xmatch".to_string(),
            block_number: 11,
            data: serde_json::json!({ "id": "0xmatch", "balance": "200" }),
        });
    });

    let _ = ready_tx.send(());
    use futures::StreamExt;
    let item = tokio::time::timeout(std::time::Duration::from_secs(2), stream.next())
        .await
        .expect("Stream item received within timeout")
        .expect("Stream has item");

    assert!(item.is_ok(), "Response error: {:?}", item.errors);
    let json = item.data.into_json().unwrap();
    assert_eq!(json["onAccount"]["id"], "0xmatch");
    assert_eq!(json["onAccount"]["balance"], "200");
}
