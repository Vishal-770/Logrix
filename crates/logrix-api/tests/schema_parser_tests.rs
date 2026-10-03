use logrix_api::schema_parser::{FieldType, SchemaDefinition};

#[test]
fn test_parse_graphql_sdl_entities() {
    let sdl = r#"
    type Account @entity {
        id: ID!
        balance: BigInt!
        transfersCount: Int!
        isActive: Boolean
    }

    type LiquidityPool @entity {
        id: ID!
        token0: String!
        token1: String!
        reserve0: BigInt!
        reserve1: BigInt!
    }
    "#;

    let schema = SchemaDefinition::from_graphql_sdl(sdl).expect("Valid GraphQL SDL");
    assert_eq!(schema.entities.len(), 2);

    let account = &schema.entities[0];
    assert_eq!(account.name, "Account");
    assert_eq!(account.fields.len(), 4);
    assert_eq!(account.fields[0].name, "id");
    assert_eq!(account.fields[0].field_type, FieldType::Id);
    assert!(!account.fields[0].is_nullable);

    assert_eq!(account.fields[1].name, "balance");
    assert_eq!(account.fields[1].field_type, FieldType::BigInt);
    assert!(!account.fields[1].is_nullable);

    assert_eq!(account.fields[2].name, "transfersCount");
    assert_eq!(account.fields[2].field_type, FieldType::Int);

    assert_eq!(account.fields[3].name, "isActive");
    assert_eq!(account.fields[3].field_type, FieldType::Boolean);
    assert!(account.fields[3].is_nullable);

    let pool = &schema.entities[1];
    assert_eq!(pool.name, "LiquidityPool");
    assert_eq!(pool.fields.len(), 5);
}

#[test]
fn test_parse_yaml_schema_entities() {
    let yaml = r#"
    entities:
      - name: TokenHolder
        fields:
          - name: id
            type: ID!
          - name: holderAddress
            type: String!
          - name: totalBalance
            type: BigInt!
          - name: isWhitelisted
            type: Boolean
    "#;

    let schema = SchemaDefinition::from_yaml(yaml).expect("Valid YAML schema");
    assert_eq!(schema.entities.len(), 1);

    let entity = &schema.entities[0];
    assert_eq!(entity.name, "TokenHolder");
    assert_eq!(entity.fields.len(), 4);
    assert_eq!(entity.fields[0].name, "id");
    assert_eq!(entity.fields[0].field_type, FieldType::Id);
    assert_eq!(entity.fields[1].name, "holderAddress");
    assert_eq!(entity.fields[1].field_type, FieldType::String);
    assert_eq!(entity.fields[2].name, "totalBalance");
    assert_eq!(entity.fields[2].field_type, FieldType::BigInt);
    assert_eq!(entity.fields[3].name, "isWhitelisted");
    assert_eq!(entity.fields[3].field_type, FieldType::Boolean);
    assert!(entity.fields[3].is_nullable);
}
