use logrix_api::query_builder::{
    validate_identifier, DynamicQueryParams, FieldFilter, FilterOperator,
};

#[test]
fn test_query_builder_equality_and_comparison_filters() {
    let params = DynamicQueryParams {
        chain_id: 1,
        entity_type: "Account".to_string(),
        filters: vec![
            FieldFilter {
                field: "balance".to_string(),
                op: FilterOperator::Gt,
                value: "1000.5".to_string(),
            },
            FieldFilter {
                field: "isActive".to_string(),
                op: FilterOperator::Eq,
                value: "true".to_string(),
            },
        ],
        order_by: Some("balance".to_string()),
        order_direction: Some("desc".to_string()),
        first: Some(50),
        skip: Some(10),
        after: Some("0xabc".to_string()),
    };

    let builder = params.build_query().expect("Valid query builder");
    let sql = builder.into_sql();

    assert!(sql.contains("SELECT entity_id, data, block_number, updated_at FROM logrix_entities"));
    assert!(sql.contains("WHERE chain_id ="));
    assert!(sql.contains("AND entity_type ="));
    assert!(sql.contains("AND is_reverted = FALSE"));
    assert!(sql.contains("AND entity_id >"));
    assert!(sql.contains("(data->>'balance')::numeric >"));
    assert!(sql.contains("(data->>'isActive') ="));
    assert!(sql.contains("ORDER BY (data->>'balance') DESC"));
    assert!(sql.contains("LIMIT"));
    assert!(sql.contains("OFFSET"));
}

#[test]
fn test_query_builder_string_operators() {
    let params = DynamicQueryParams {
        chain_id: 1,
        entity_type: "TokenHolder".to_string(),
        filters: vec![
            FieldFilter {
                field: "name".to_string(),
                op: FilterOperator::Contains,
                value: "dao".to_string(),
            },
            FieldFilter {
                field: "prefix".to_string(),
                op: FilterOperator::StartsWith,
                value: "0x".to_string(),
            },
        ],
        order_by: None,
        order_direction: None,
        first: None,
        skip: None,
        after: None,
    };

    let builder = params.build_query().expect("Valid query builder");
    let sql = builder.into_sql();

    assert!(sql.contains("(data->>'name') ILIKE"));
    assert!(sql.contains("(data->>'prefix') ILIKE"));
    assert!(sql.contains("ORDER BY block_number DESC, entity_id ASC"));
    assert!(sql.contains("LIMIT "));
}

#[test]
fn test_query_builder_injection_protection() {
    assert!(validate_identifier("valid_field_123").is_ok());
    assert!(validate_identifier("balance").is_ok());
    assert!(validate_identifier("Robert'); DROP TABLE logrix_entities;--").is_err());
    assert!(validate_identifier("field$name").is_err());
    assert!(validate_identifier("").is_err());
}
