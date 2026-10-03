use alloy_primitives::{Address, Bytes, B256};
use logrix_core::domain::EventLog;
use logrix_handlers::{DeclarativeMapper, DeclarativeRule, Manifest, UserLogicEngine};
use std::collections::HashMap;

fn create_sample_log() -> EventLog {
    let mut data = vec![0u8; 64];
    data[31] = 100;
    data[63] = 200;

    EventLog {
        address: Address::repeat_byte(0xaa),
        topics: vec![
            B256::repeat_byte(0x01),
            B256::repeat_byte(0x02),
            B256::repeat_byte(0x03),
        ],
        data: Bytes::from(data),
        tx_hash: B256::repeat_byte(0x99),
        log_index: 5,
        tx_index: 2,
        block_number: 12345,
        block_hash: B256::repeat_byte(0x55),
    }
}

#[test]
fn test_declarative_mapping_field_extraction() {
    let mapper = DeclarativeMapper::new();
    let log = create_sample_log();

    let mut fields = HashMap::new();
    fields.insert("contract".to_string(), "log.address".to_string());
    fields.insert("block".to_string(), "log.block_number".to_string());
    fields.insert("from".to_string(), "log.topics[1]".to_string());
    fields.insert("to".to_string(), "log.topics[2]".to_string());
    fields.insert("val_chunk".to_string(), "log.data[0..32]".to_string());

    let rule = DeclarativeRule {
        event: "Transfer".to_string(),
        entity: "TokenTransferEntity".to_string(),
        fields,
    };

    let entities = mapper.apply_rules(&[rule], &log);
    assert_eq!(entities.len(), 1);

    let entity = &entities[0];
    assert_eq!(entity.entity_type, "TokenTransferEntity");

    let payload = &entity.payload;
    assert_eq!(payload["block"], 12345);
    assert_eq!(
        payload["contract"],
        format!("{:#x}", Address::repeat_byte(0xaa))
    );
    assert_eq!(payload["from"], format!("{:#x}", B256::repeat_byte(0x02)));
    assert_eq!(payload["to"], format!("{:#x}", B256::repeat_byte(0x03)));
    assert!(payload["val_chunk"].as_str().unwrap().starts_with("0x"));
}

#[test]
fn test_manifest_yaml_parsing() {
    let yaml = r#"
schema_version: "1.0"
chain_id: 421614
contracts:
  - name: "USDC"
    address: "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d"
    events:
      - "Transfer(address,address,uint256)"
    declarative:
      - event: "Transfer"
        entity: "TransferRecord"
        fields:
          from: "log.topics[1]"
          to: "log.topics[2]"
          block: "log.block_number"
    wasm_handler: null
"#;

    let manifest = Manifest::from_yaml_str(yaml).expect("Valid manifest YAML");
    assert_eq!(manifest.chain_id, 421614);
    assert_eq!(manifest.contracts.len(), 1);
    assert_eq!(manifest.contracts[0].name, "USDC");
    assert_eq!(manifest.contracts[0].declarative.len(), 1);
}

#[tokio::test]
async fn test_user_logic_engine_orchestration_and_state_persistence() {
    let yaml = r#"
schema_version: "1.0"
chain_id: 421614
contracts:
  - name: "TestToken"
    address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    events:
      - "Transfer(address,address,uint256)"
    declarative:
      - event: "Transfer"
        entity: "DeclarativeTransfer"
        fields:
          block: "log.block_number"
    wasm_handler: null
"#;

    let manifest = Manifest::from_yaml_str(yaml).unwrap();
    let mut engine = UserLogicEngine::new(manifest).unwrap();

    let wat = r#"
    (module
      (import "logrix" "logrix_db_set" (func $db_set (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)

      (data (i32.const 50) "counter")
      (data (i32.const 60) "1")

      (func (export "handle_event") (param i32 i32) (result i32)
        (drop (call $db_set (i32.const 50) (i32.const 7) (i32.const 60) (i32.const 1)))
        (i32.const 0)
      )
    )
    "#;

    let wasm_bytes = wat::parse_str(wat).unwrap();
    let contract_addr = Address::repeat_byte(0xaa);
    engine
        .load_wasm_handler(contract_addr, &wasm_bytes)
        .unwrap();

    let log = create_sample_log();
    let staging = engine.process_log(&log).await.unwrap();

    assert_eq!(staging.emitted_entities().len(), 1);
    assert_eq!(
        staging.emitted_entities()[0].entity_type,
        "DeclarativeTransfer"
    );
    assert_eq!(staging.get_state("counter").unwrap(), "1");

    let (mutations, emitted) = engine.commit_staging(staging).await;
    assert_eq!(mutations.len(), 1);
    assert_eq!(emitted.len(), 1);
    assert_eq!(engine.get_state("counter").await.unwrap(), "1");
}
