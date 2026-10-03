use alloy_primitives::{Address, Bytes, B256};
use logrix_core::domain::EventLog;
use logrix_handlers::{
    DeclarativeMapper, DeclarativeRule, Manifest, UserLogicEngine, WasmHandlerInstance, WasmRuntime,
};
use std::collections::HashMap;

fn create_sample_log() -> EventLog {
    let mut data = vec![0u8; 64];
    data[31] = 100; // First 32 bytes: uint256 = 100
    data[63] = 200; // Second 32 bytes: uint256 = 200

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

#[test]
fn test_wasm_state_mutation_and_entity_emission() {
    let runtime = WasmRuntime::new().unwrap();

    // WAT WebAssembly module calling logrix host functions
    let wat = r#"
    (module
      (import "logrix" "logrix_db_set" (func $db_set (param i32 i32 i32 i32) (result i32)))
      (import "logrix" "logrix_emit" (func $emit (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)

      ;; Static strings placed in memory:
      ;; 100: "account:0x1" (len 11)
      ;; 120: "1500"        (len 4)
      ;; 130: "AccountState"(len 12)
      ;; 150: "{\"balance\":1500}" (len 16)
      (data (i32.const 100) "account:0x1")
      (data (i32.const 120) "1500")
      (data (i32.const 130) "AccountState")
      (data (i32.const 150) "{\"balance\":1500}")

      (func (export "handle_event") (param i32 i32) (result i32)
        ;; call db_set("account:0x1", "1500")
        (drop (call $db_set (i32.const 100) (i32.const 11) (i32.const 120) (i32.const 4)))
        ;; call emit("AccountState", "{\"balance\":1500}")
        (drop (call $emit (i32.const 130) (i32.const 12) (i32.const 150) (i32.const 16)))
        (i32.const 0)
      )
    )
    "#;

    let wasm_bytes = wat::parse_str(wat).expect("Compile WAT");
    let module = runtime.compile_module(&wasm_bytes).unwrap();
    let instance = WasmHandlerInstance::new(runtime, module);

    let staging = instance
        .execute_event("{}", HashMap::new())
        .expect("Execution succeeds");

    assert_eq!(staging.get_state("account:0x1").unwrap(), "1500");
    assert_eq!(staging.emitted_entities().len(), 1);
    assert_eq!(staging.emitted_entities()[0].entity_type, "AccountState");
    assert_eq!(staging.emitted_entities()[0].payload["balance"], 1500);
}

#[test]
fn test_wasm_infinite_loop_fuel_exhaustion_traps() {
    let runtime = WasmRuntime::new().unwrap();

    // Module with an infinite loop
    let wat = r#"
    (module
      (memory (export "memory") 1)
      (func (export "handle_event") (param i32 i32) (result i32)
        (loop (br 0))
        (i32.const 0)
      )
    )
    "#;

    let wasm_bytes = wat::parse_str(wat).expect("Compile WAT");
    let module = runtime.compile_module(&wasm_bytes).unwrap();
    let instance = WasmHandlerInstance::new(runtime, module).with_fuel_limit(10_000);

    let result = instance.execute_event("{}", HashMap::new());
    assert!(result.is_err());
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("fuel") || err_msg.contains("trapped"),
        "Expected fuel exhaustion trap, got: {err_msg}"
    );
}

#[test]
fn test_wasm_memory_ceiling_limit() {
    let runtime = WasmRuntime::new().unwrap();

    // Module requesting 2000 pages of memory (2000 * 64KB = 128MB > 64MB limit)
    let wat = r#"
    (module
      (memory (export "memory") 2000)
      (func (export "handle_event") (param i32 i32) (result i32)
        (i32.const 0)
      )
    )
    "#;

    let wasm_bytes = wat::parse_str(wat).expect("Compile WAT");
    let module = runtime.compile_module(&wasm_bytes).unwrap();
    let instance = WasmHandlerInstance::new(runtime, module);

    let result = instance.execute_event("{}", HashMap::new());
    assert!(result.is_err());
    let err_msg = result.err().unwrap().to_string();
    assert!(
        err_msg.contains("memory") || err_msg.contains("exceeds limit"),
        "Expected memory limit violation, got: {err_msg}"
    );
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

    // WASM module that increments an execution counter in state
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

    // Both declarative entity and WASM state mutation should be staged
    assert_eq!(staging.emitted_entities().len(), 1);
    assert_eq!(
        staging.emitted_entities()[0].entity_type,
        "DeclarativeTransfer"
    );
    assert_eq!(staging.get_state("counter").unwrap(), "1");

    // Commit staging to engine
    let (mutations, emitted) = engine.commit_staging(staging).await;
    assert_eq!(mutations.len(), 1);
    assert_eq!(emitted.len(), 1);

    // State is now persisted in engine's store
    assert_eq!(engine.get_state("counter").await.unwrap(), "1");
}

#[tokio::test]
async fn test_wasm_transactional_rollback_on_panic() {
    let runtime = WasmRuntime::new().unwrap();

    // Module that writes state and then executes 'unreachable'
    let wat = r#"
    (module
      (import "logrix" "logrix_db_set" (func $db_set (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)

      (data (i32.const 50) "dirty_key")
      (data (i32.const 70) "corrupted_val")

      (func (export "handle_event") (param i32 i32) (result i32)
        ;; Stage a write
        (drop (call $db_set (i32.const 50) (i32.const 9) (i32.const 70) (i32.const 13)))
        ;; Trap!
        (unreachable)
        (i32.const 0)
      )
    )
    "#;

    let wasm_bytes = wat::parse_str(wat).unwrap();
    let module = runtime.compile_module(&wasm_bytes).unwrap();
    let instance = WasmHandlerInstance::new(runtime, module);

    let mut base_state = HashMap::new();
    base_state.insert("clean_key".to_string(), "original_val".to_string());

    let result = instance.execute_event("{}", base_state.clone());
    assert!(result.is_err());

    // Verify error trapped cleanly
    let err = result.err().unwrap();
    assert!(err.to_string().contains("trapped") || err.to_string().contains("unreachable"));

    // Base state was never contaminated
    assert_eq!(base_state.get("clean_key").unwrap(), "original_val");
    assert!(!base_state.contains_key("dirty_key"));
}
