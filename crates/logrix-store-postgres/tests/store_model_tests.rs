use chrono::Utc;
use logrix_store_postgres::{TokenTransfer, TransferFilter};

#[test]
fn test_token_transfer_serialization() {
    let transfer = TokenTransfer {
        chain_id: 421614,
        block_number: 1000,
        block_hash: "0x123".to_string(),
        tx_hash: "0xabc".to_string(),
        log_index: 2,
        contract_address: "0xcontract".to_string(),
        from_address: "0xfrom".to_string(),
        to_address: "0xto".to_string(),
        amount: "5000000".to_string(),
        timestamp: Utc::now(),
    };

    let serialized = serde_json::to_string(&transfer).expect("serialize transfer");
    assert!(serialized.contains("421614"));
    assert!(serialized.contains("5000000"));

    let deserialized: TokenTransfer =
        serde_json::from_str(&serialized).expect("deserialize transfer");
    assert_eq!(transfer, deserialized);
}

#[test]
fn test_transfer_filter_defaults() {
    let filter = TransferFilter::default();
    assert!(filter.contract_address.is_none());
    assert!(filter.from_address.is_none());
    assert!(filter.to_address.is_none());
    assert!(filter.limit.is_none());
    assert!(filter.offset.is_none());
}

#[test]
fn test_token_transfer_amount_u256() {
    let transfer = TokenTransfer {
        chain_id: 1,
        block_number: 19_000_000,
        block_hash: "0xblock".to_string(),
        tx_hash: "0xtx".to_string(),
        log_index: 0,
        contract_address: "0xtoken".to_string(),
        from_address: "0xalice".to_string(),
        to_address: "0xbob".to_string(),
        amount: "1000000000000000000000000000".to_string(), // 1 billion tokens with 18 decimals
        timestamp: Utc::now(),
    };

    let u256_val = transfer.amount_u256().expect("valid U256 string");
    assert_eq!(
        u256_val,
        "1000000000000000000000000000"
            .parse::<alloy_primitives::U256>()
            .unwrap()
    );

    let invalid_transfer = TokenTransfer {
        amount: "not-a-number".to_string(),
        ..transfer
    };
    assert!(invalid_transfer.amount_u256().is_none());
}

#[test]
fn test_postgres_config_builder() {
    use logrix_store_postgres::PostgresConfig;
    use std::time::Duration;

    let config = PostgresConfig::new("postgres://user:pass@localhost:5432/db", "main-pipeline")
        .max_connections(50)
        .min_connections(10)
        .acquire_timeout(Duration::from_secs(25))
        .idle_timeout(Duration::from_secs(120));

    assert_eq!(
        config.database_url,
        "postgres://user:pass@localhost:5432/db"
    );
    assert_eq!(config.pipeline_id, "main-pipeline");
    assert_eq!(config.max_connections, 50);
    assert_eq!(config.min_connections, 10);
    assert_eq!(config.acquire_timeout, Duration::from_secs(25));
    assert_eq!(config.idle_timeout, Duration::from_secs(120));
}

#[test]
fn test_dynamic_entity_models_roundtrip() {
    use logrix_store_postgres::{DynamicEntityRecord, EntityInsert};

    let insert = EntityInsert {
        entity_type: "Account".to_string(),
        entity_id: "0x123".to_string(),
        data: serde_json::json!({"balance": "5000", "nonce": 42}),
    };
    let json = serde_json::to_string(&insert).unwrap();
    let decoded: EntityInsert = serde_json::from_str(&json).unwrap();
    assert_eq!(insert, decoded);

    let record = DynamicEntityRecord {
        chain_id: 42161,
        entity_type: "Account".to_string(),
        entity_id: "0x123".to_string(),
        data: serde_json::json!({"balance": "5000"}),
        block_number: 12345,
        is_reverted: false,
        updated_at: Utc::now(),
    };
    let record_json = serde_json::to_string(&record).unwrap();
    let decoded_record: DynamicEntityRecord = serde_json::from_str(&record_json).unwrap();
    assert_eq!(record, decoded_record);
}
