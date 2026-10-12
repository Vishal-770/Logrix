use logrix_handlers::{WasmHandlerInstance, WasmRuntime};
use std::collections::HashMap;

#[test]
fn test_wasm_state_mutation_and_entity_emission() {
    let runtime = WasmRuntime::new().unwrap();

    let wat = r#"
    (module
      (import "logrix" "logrix_db_set" (func $db_set (param i32 i32 i32 i32) (result i32)))
      (import "logrix" "logrix_emit" (func $emit (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)

      (data (i32.const 100) "account:0x1")
      (data (i32.const 120) "1500")
      (data (i32.const 130) "AccountState")
      (data (i32.const 150) "{\"balance\":1500}")

      (func (export "handle_event") (param i32 i32) (result i32)
        (drop (call $db_set (i32.const 100) (i32.const 11) (i32.const 120) (i32.const 4)))
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
async fn test_wasm_transactional_rollback_on_panic() {
    let runtime = WasmRuntime::new().unwrap();

    let wat = r#"
    (module
      (import "logrix" "logrix_db_set" (func $db_set (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)

      (data (i32.const 50) "dirty_key")
      (data (i32.const 70) "corrupted_val")

      (func (export "handle_event") (param i32 i32) (result i32)
        (drop (call $db_set (i32.const 50) (i32.const 9) (i32.const 70) (i32.const 13)))
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

    let err = result.err().unwrap();
    assert!(err.to_string().contains("trapped") || err.to_string().contains("unreachable"));
    assert_eq!(base_state.get("clean_key").unwrap(), "original_val");
    assert!(!base_state.contains_key("dirty_key"));
}

#[test]
fn test_live_mapping_wasm_execution() {
    let wasm_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../arbitrum-uniswap-indexer/build/mapping.wasm"
    );
    let wasm_bytes = match std::fs::read(wasm_path) {
        Ok(b) => b,
        Err(_) => {
            eprintln!("Skipping live mapping test: mapping.wasm not found");
            return;
        }
    };
    let runtime = WasmRuntime::new().expect("WasmRuntime::new");
    let module = runtime
        .compile_module(&wasm_bytes)
        .expect("compile mapping.wasm");
    let instance = WasmHandlerInstance::new(runtime, module).with_fuel_limit(10_000_000);

    let log = logrix_core::domain::EventLog {
        address: "0xc6962004f452be9203591991d15f6b388e09e8d0"
            .parse::<alloy_primitives::Address>()
            .unwrap(),
        topics: vec![
            "0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67"
                .parse::<alloy_primitives::B256>()
                .unwrap(),
            "0x000000000000000000000000e592427a0aece92de3edee1f18e0157c05861564"
                .parse::<alloy_primitives::B256>()
                .unwrap(),
        ],
        data: alloy_primitives::Bytes::from(vec![0u8; 160]),
        tx_hash: "0x63b823ac0ec1af4ca3da5c48516dc9a0551c0fca69324cf6e08e754579c06c99"
            .parse::<alloy_primitives::B256>()
            .unwrap(),
        log_index: 0,
        tx_index: 0,
        block_number: 514111951,
        block_hash: alloy_primitives::B256::ZERO,
    };

    let log_json = serde_json::to_string(&log).unwrap();
    let staging = instance
        .execute_event(&log_json, HashMap::new())
        .expect("Execution succeeds");
    println!("Entities emitted: {}", staging.emitted_entities().len());
    for e in staging.emitted_entities() {
        println!("Emitted: {} -> {}", e.entity_type, e.payload);
    }
    assert_eq!(staging.emitted_entities().len(), 3);
}
