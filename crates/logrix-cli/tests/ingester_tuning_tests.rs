use logrix_chain_evm::EvmClientConfig;
use logrix_cli::ingester::IngesterConfig;
use logrix_core::domain::ChainId;
use logrix_rpc_gateway::{GatewayConfig, ManagedProvider, ProviderPool, RpcGateway};
use std::time::Duration;

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
        start_block: None,
        chunk_size: 50,
        max_queue_depth: 1000,
        chain_head_poll_ms: 1500,
        manifest_path: Some(tmp_path.to_str().unwrap().to_string()),
    };

    let (chunk, max_q, poll_ms) = cfg.resolve_tuning();
    assert_eq!(chunk, 25);
    assert_eq!(max_q, 500);
    assert_eq!(poll_ms, 800);

    let _ = std::fs::remove_file(tmp_path);
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
