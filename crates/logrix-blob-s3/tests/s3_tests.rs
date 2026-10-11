use aws_sdk_s3::Client as S3Client;
use logrix_blob_s3::{compress_zstd, decompress_zstd, S3Config, DEFAULT_ZSTD_LEVEL};

#[tokio::test]
async fn test_s3_key_resolution() {
    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;
    let client = S3Client::new(&sdk_config);

    let config_with_prefix = S3Config::new(
        client.clone(),
        "logrix-blocks",
        Some("mainnet/raw-blocks".into()),
    );
    assert_eq!(
        config_with_prefix.resolve_key("19000000.json.zst"),
        "mainnet/raw-blocks/19000000.json.zst"
    );

    let config_no_prefix = S3Config::new(client, "logrix-blocks", None);
    assert_eq!(
        config_no_prefix.resolve_key("19000000.json.zst"),
        "19000000.json.zst"
    );
}

#[test]
fn test_zstd_compression_ratio() {
    // Generate repetitive JSON block simulating Ethereum transactions
    let mut fake_block = String::from("{\"block\": 19500000, \"transactions\": [");
    for i in 0..100 {
        fake_block.push_str(&format!(
            "{{\"hash\": \"0x{:064x}\", \"from\": \"0x{:040x}\"}},",
            i, i
        ));
    }
    fake_block.push_str("]}");

    let original_bytes = fake_block.as_bytes();
    let compressed = compress_zstd(original_bytes, DEFAULT_ZSTD_LEVEL).expect("compression");
    assert!(
        compressed.len() < original_bytes.len(),
        "compressed size {} should be smaller than original {}",
        compressed.len(),
        original_bytes.len()
    );

    let decompressed = decompress_zstd(&compressed).expect("decompression");
    assert_eq!(decompressed, original_bytes);
}

#[tokio::test]
async fn test_s3_config_from_env_override() {
    let config = S3Config::from_env(
        "my-test-bucket",
        Some("chain-1".into()),
        Some("http://127.0.0.1:4566"),
    )
    .await;

    assert!(config.is_ok());
    let cfg = config.unwrap();
    assert_eq!(cfg.bucket, "my-test-bucket");
    assert_eq!(cfg.resolve_key("block.json"), "chain-1/block.json");
}

#[tokio::test]
async fn test_s3_client_options_builder() {
    let builder_res = S3Config::builder("production-archive")
        .key_prefix("eth/mainnet")
        .endpoint_url("http://127.0.0.1:9000")
        .region("us-east-1")
        .force_path_style(true)
        .build()
        .await;

    assert!(builder_res.is_ok());
    let cfg = builder_res.unwrap();
    assert_eq!(cfg.bucket, "production-archive");
    assert_eq!(cfg.resolve_key("100.json.zst"), "eth/mainnet/100.json.zst");
}

#[test]
fn test_zstd_empty_and_corrupt_data() {
    // Empty data roundtrip
    let empty = b"";
    let compressed_empty = compress_zstd(empty, 0).expect("compress empty");
    let decompressed_empty = decompress_zstd(&compressed_empty).expect("decompress empty");
    assert_eq!(decompressed_empty, empty);

    // Corrupted payload fails cleanly
    let garbage = b"this is not a valid zstd payload";
    let result = decompress_zstd(garbage);
    assert!(result.is_err());
}
