use logrix_store_postgres::EntityInsert;
use std::collections::HashMap;

#[test]
fn test_entity_deduplication_preserves_latest_state() {
    let entities = vec![
        EntityInsert {
            entity_type: "Protocol".to_string(),
            entity_id: "aave-v3".to_string(),
            data: serde_json::json!({"totalSuppliedCount": "0"}),
        },
        EntityInsert {
            entity_type: "User".to_string(),
            entity_id: "0xuser1".to_string(),
            data: serde_json::json!({"supplyCount": "1"}),
        },
        EntityInsert {
            entity_type: "Protocol".to_string(),
            entity_id: "aave-v3".to_string(),
            data: serde_json::json!({"totalSuppliedCount": "1"}),
        },
        EntityInsert {
            entity_type: "User".to_string(),
            entity_id: "0xuser1".to_string(),
            data: serde_json::json!({"supplyCount": "2"}),
        },
    ];

    let mut deduped: HashMap<(&str, &str), &EntityInsert> = HashMap::with_capacity(entities.len());
    for e in &entities {
        deduped.insert((e.entity_type.as_str(), e.entity_id.as_str()), e);
    }

    assert_eq!(deduped.len(), 2);
    let protocol = deduped.get(&("Protocol", "aave-v3")).unwrap();
    assert_eq!(protocol.data["totalSuppliedCount"], "1");

    let user = deduped.get(&("User", "0xuser1")).unwrap();
    assert_eq!(user.data["supplyCount"], "2");
}

#[test]
fn test_entity_deduplication_different_types_same_id() {
    let entities = vec![
        EntityInsert {
            entity_type: "Protocol".to_string(),
            entity_id: "id-1".to_string(),
            data: serde_json::json!({"metric": "proto"}),
        },
        EntityInsert {
            entity_type: "User".to_string(),
            entity_id: "id-1".to_string(),
            data: serde_json::json!({"metric": "user"}),
        },
    ];

    let mut deduped: HashMap<(&str, &str), &EntityInsert> = HashMap::with_capacity(entities.len());
    for e in &entities {
        deduped.insert((e.entity_type.as_str(), e.entity_id.as_str()), e);
    }

    assert_eq!(deduped.len(), 2);
    assert_eq!(
        deduped.get(&("Protocol", "id-1")).unwrap().data["metric"],
        "proto"
    );
    assert_eq!(
        deduped.get(&("User", "id-1")).unwrap().data["metric"],
        "user"
    );
}
