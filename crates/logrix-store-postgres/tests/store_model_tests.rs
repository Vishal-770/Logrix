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
