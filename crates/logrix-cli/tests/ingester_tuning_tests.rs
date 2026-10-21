use logrix_chain_evm::EvmClientConfig;
use logrix_cli::{env::expand_env_placeholders, ingester::IngesterConfig};
use logrix_core::domain::ChainId;
use logrix_rpc_gateway::{GatewayConfig, ManagedProvider, ProviderPool, RpcGateway};
use std::time::Duration;

#[test]
fn test_runtime_environment_placeholder_expansion() {
    let name = "LOGRIX_CLI_TEST_RPC_TOKEN";
    let previous = std::env::var(name).ok();
    std::env::set_var(name, "runtime-token");

    let expanded = expand_env_placeholders(
        "https://rpc.example/${LOGRIX_CLI_TEST_RPC_TOKEN}",
        "RPC_URL",
    )
    .expect("runtime placeholder should expand");
    assert_eq!(expanded, "https://rpc.example/runtime-token");

    match previous {
        Some(value) => std::env::set_var(name, value),
        None => std::env::remove_var(name),
    }
}

#[test]
fn test_runtime_environment_placeholder_missing_is_an_error() {
    let name = "LOGRIX_CLI_TEST_MISSING_RPC_TOKEN";
    std::env::remove_var(name);
    let error = expand_env_placeholders(
        "https://rpc.example/${LOGRIX_CLI_TEST_MISSING_RPC_TOKEN}",
        "RPC_URL",
    )
    .expect_err("missing runtime placeholder should fail");
    assert!(error.message.contains(name));
}

#[tokio::test]
async fn test_ingester_config_tuning_resolution() {
    let tmp_path = std::env::temp_dir().join("logrix_test_ingester_manifest.yaml");
    let manifest_yaml = r#"
schema_version: "1"
chain_id: 1
indexing:
  chunk_size: 25
  max_queue_depth: 500
  chain_head_poll_ms: 800
contracts: []
"#;
    std::fs::write(&tmp_path, manifest_yaml).expect("write temp manifest");

    let cfg = IngesterConfig {
        chain_id: ChainId::new(1),
        database_url: "postgres://localhost/test".to_string(),
        database_max_connections: 10,
        database_min_connections: 2,
        database_statement_timeout_secs: 30,
        start_block: None,
        chunk_size: 50,
        max_queue_depth: 1000,
        chain_head_poll_ms: 1500,
        manifest_path: Some(tmp_path.to_str().unwrap().to_string()),
    };

    let (chunk, max_q, poll_ms) = cfg.resolve_tuning().expect("resolve valid manifest");
    assert_eq!(chunk, 25);
    assert_eq!(max_q, 500);
    assert_eq!(poll_ms, 800);

    let _ = std::fs::remove_file(tmp_path);
}

#[test]
fn test_ingester_config_tuning_propagates_manifest_errors() {
    let temp_dir = std::env::temp_dir();
    let invalid_path = temp_dir.join(format!(
        "logrix_test_invalid_ingester_manifest_{}",
        std::process::id()
    ));
    std::fs::write(&invalid_path, "indexing: [not valid").expect("write invalid manifest");

    let invalid_cfg = IngesterConfig {
        chain_id: ChainId::new(1),
        database_url: "postgres://localhost/test".to_string(),
        database_max_connections: 10,
        database_min_connections: 2,
        database_statement_timeout_secs: 30,
        start_block: None,
        chunk_size: 50,
        max_queue_depth: 1000,
        chain_head_poll_ms: 1500,
        manifest_path: Some(invalid_path.to_string_lossy().into_owned()),
    };
    assert!(
        invalid_cfg.resolve_tuning().is_err(),
        "invalid manifest errors must be returned"
    );

    let unreadable_cfg = IngesterConfig {
        manifest_path: Some(
            temp_dir
                .join(format!(
                    "logrix_test_missing_ingester_manifest_{}",
                    std::process::id()
                ))
                .to_string_lossy()
                .into_owned(),
        ),
        ..invalid_cfg
    };
    assert!(
        unreadable_cfg.resolve_tuning().is_err(),
        "unreadable manifest errors must be returned"
    );

    let _ = std::fs::remove_file(invalid_path);
}

#[tokio::test]
async fn test_rpc_gateway_effective_chunk_size_aimd() {
    let chain_id = ChainId::new(1);
    let pool = ProviderPool::new(chain_id);
    let gateway = RpcGateway::new(chain_id, pool.clone(), None, None);

    assert_eq!(gateway.effective_chunk_size().await, None);

    let evm_cfg = EvmClientConfig {
        chunk_initial: 100,
        chunk_min: 10,
        chunk_max: 500,
        chunk_additive_step: 25,
        chunk_success_threshold: 2,
        ..Default::default()
    };

    let provider = ManagedProvider::new_with_config(
        "test-provider",
        "http://127.0.0.1:18545",
        1,
        chain_id,
        &GatewayConfig::default(),
        Some(evm_cfg),
    );

    pool.add_provider(provider.clone()).await;

    assert_eq!(gateway.effective_chunk_size().await, Some(100));

    // Multiplicative decrease upon failure
    provider.client().chunker().record_failure();
    assert_eq!(gateway.effective_chunk_size().await, Some(50));

    provider.client().chunker().record_failure();
    assert_eq!(gateway.effective_chunk_size().await, Some(25));

    // Additive increase upon repeated success (success threshold = 2)
    provider
        .client()
        .chunker()
        .record_success(Duration::from_millis(10));
    assert_eq!(gateway.effective_chunk_size().await, Some(25));

    provider
        .client()
        .chunker()
        .record_success(Duration::from_millis(10));
    assert_eq!(gateway.effective_chunk_size().await, Some(50)); // 25 + 25 = 50
}
