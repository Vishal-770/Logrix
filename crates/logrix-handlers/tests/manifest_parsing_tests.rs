use logrix_core::domain::ChainId;
use logrix_handlers::Manifest;

#[test]
fn test_sdk_manifest_with_webhooks_and_camel_case_indexing() {
    let yaml = r#"
specVersion: "0.1.0"
network:
  chainId: 42161
dataSources:
  - name: "UniswapV3Pool"
    source:
      address: "0x1111111111111111111111111111111111111111"
    mapping:
      file: "./mapping.wasm"
      eventHandlers:
        - event: "Swap"
      fuelLimit: 5000000
      maxMemoryMb: 256
      enabled: true
webhooks:
  - url: "https://api.example.com/events"
    events: ["reorg", "Swap"]
    maxRetries: 4
    secret: "my_secret"
indexing:
  reconcilerChunkSize: 50
  reconcilerLagThreshold: 100
  chainHeadPollMs: 2000
  ringBufferDepth: 256
"#;

    let manifest = Manifest::from_yaml_str(yaml).expect("Valid SDK manifest");
    assert_eq!(manifest.chain_id, ChainId::new(42161));
    assert_eq!(manifest.contracts.len(), 1);
    assert_eq!(manifest.contracts[0].fuel_limit, Some(5000000));
    assert_eq!(manifest.contracts[0].max_memory_mb, Some(256));

    // Verify webhooks are parsed
    assert_eq!(manifest.webhooks.len(), 1);
    assert_eq!(manifest.webhooks[0].url, "https://api.example.com/events");
    assert_eq!(manifest.webhooks[0].max_retries, Some(4));
    assert_eq!(manifest.webhooks[0].secret.as_deref(), Some("my_secret"));
    assert_eq!(manifest.webhooks[0].events, vec!["reorg", "Swap"]);

    // Verify camelCase indexing is parsed
    let idx = manifest.indexing.expect("Indexing block present");
    assert_eq!(idx.reconciler_chunk_size, Some(50));
    assert_eq!(idx.reconciler_lag_threshold, Some(100));
    assert_eq!(idx.chain_head_poll_ms, Some(2000));
    assert_eq!(idx.ring_buffer_depth, Some(256));
}

#[test]
fn test_malformed_indexing_type_fails_cleanly_without_swallowing() {
    let yaml = r#"
specVersion: "0.1.0"
network:
  chainId: 1
dataSources: []
indexing:
  reconcilerChunkSize: "invalid_not_a_number"
"#;

    let err = Manifest::from_yaml_str(yaml).unwrap_err();
    assert!(
        err.message.contains("Failed to deserialize SDK project manifest")
            || err.message.contains("invalid")
            || err.message.contains("reconcilerChunkSize")
    );
}

#[test]
fn test_unsupported_manifest_format_fails() {
    let yaml = r#"
random_key: 123
other_key: "abc"
"#;

    let err = Manifest::from_yaml_str(yaml).unwrap_err();
    assert!(err.message.contains("Unsupported manifest format") || err.message.contains("Failed"));
}
