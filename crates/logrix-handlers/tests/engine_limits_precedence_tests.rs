use alloy_primitives::Address;
use logrix_handlers::{Manifest, UserLogicEngine};

#[test]
fn test_wasm_limits_precedence_hierarchy() {
    let yaml = r#"
schema_version: "1"
chain_id: 1
indexing:
  wasm_fuel_limit: 200000
  wasm_max_memory_mb: 64
contracts:
  - name: ContractWithExplicitOverride
    address: "0x1111111111111111111111111111111111111111"
    start_block: 100
    fuel_limit: 50000
    max_memory_mb: 32
  - name: ContractInheritingManifestIndexing
    address: "0x2222222222222222222222222222222222222222"
    start_block: 100
"#;

    let manifest = Manifest::from_yaml_str(yaml).expect("parse manifest");
    let mut engine = UserLogicEngine::new(manifest)
        .expect("engine new")
        .with_global_limits(1_000_000, 128);

    let wat = r#"
    (module
      (memory (export "memory") 1)
      (func (export "handle_event") (param i32 i32) (result i32)
        (i32.const 0)
      )
    )
    "#;
    let wasm_bytes = wat::parse_str(wat).expect("compile wat");

    let addr1: Address = "0x1111111111111111111111111111111111111111"
        .parse()
        .unwrap();
    let addr2: Address = "0x2222222222222222222222222222222222222222"
        .parse()
        .unwrap();

    engine
        .load_wasm_handler(addr1, &wasm_bytes)
        .expect("load addr1");
    engine
        .load_wasm_handler(addr2, &wasm_bytes)
        .expect("load addr2");

    // Contract 1: explicit contract overrides win over manifest indexing & CLI
    let h1 = engine.wasm_handler(&addr1).expect("h1 present");
    assert_eq!(h1.fuel_limit(), 50_000);
    assert_eq!(h1.max_memory_bytes(), 32 * 1024 * 1024);

    // Contract 2: manifest indexing wins over CLI global
    let h2 = engine.wasm_handler(&addr2).expect("h2 present");
    assert_eq!(h2.fuel_limit(), 200_000);
    assert_eq!(h2.max_memory_bytes(), 64 * 1024 * 1024);
}

#[test]
fn test_wasm_limits_fallback_to_cli_globals() {
    let yaml = r#"
schema_version: "1"
chain_id: 1
contracts:
  - name: ContractWithNoOverrides
    address: "0x3333333333333333333333333333333333333333"
    start_block: 100
"#;

    let manifest = Manifest::from_yaml_str(yaml).expect("parse manifest");
    let mut engine = UserLogicEngine::new(manifest)
        .expect("engine new")
        .with_global_limits(750_000, 96);

    let wat = r#"
    (module
      (memory (export "memory") 1)
      (func (export "handle_event") (param i32 i32) (result i32)
        (i32.const 0)
      )
    )
    "#;
    let wasm_bytes = wat::parse_str(wat).expect("compile wat");

    let addr3: Address = "0x3333333333333333333333333333333333333333"
        .parse()
        .unwrap();

    engine
        .load_wasm_handler(addr3, &wasm_bytes)
        .expect("load addr3");

    let h3 = engine.wasm_handler(&addr3).expect("h3 present");
    assert_eq!(h3.fuel_limit(), 750_000);
    assert_eq!(h3.max_memory_bytes(), 96 * 1024 * 1024);
}
