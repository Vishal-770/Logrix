use alloy_primitives::{address, b256, Bytes};
use logrix_core::{
    domain::{BlockRangeJob, ChainId, Checkpoint, EventLog, LiveBlockJob, QueueMessage},
    error::{ErrorClass, ErrorSource, LogrixError},
};

#[test]
fn test_error_class_predicates() {
    assert!(ErrorClass::Transient.is_retryable());
    assert!(ErrorClass::RateLimited { retry_after: None }.is_retryable());
    assert!(ErrorClass::Shrinkable.is_retryable());
    assert!(!ErrorClass::Permanent.is_retryable());
    assert!(!ErrorClass::Fatal.is_retryable());

    assert!(ErrorClass::Permanent.is_permanent());
    assert!(ErrorClass::Fatal.is_fatal());
    assert!(ErrorClass::Integrity.is_integrity());
}

#[test]
fn test_logrix_error_context_and_display() {
    let err = LogrixError::transient(ErrorSource::ChainRpc, "connection reset by peer")
        .with_chain_id(421614)
        .with_block_number(123456)
        .with_job_id("job-abc-123");

    let msg = format!("{err}");
    assert!(msg.contains("ChainRpc"));
    assert!(msg.contains("Transient"));
    assert!(msg.contains("connection reset by peer"));
    assert!(msg.contains("chain: 421614"));
    assert!(msg.contains("block: 123456"));
    assert!(msg.contains("job: job-abc-123"));
}

#[test]
fn test_queue_message_versioned_serialization() {
    let live_job = LiveBlockJob::new(
        ChainId::ARBITRUM_SEPOLIA,
        5000000,
        b256!("1111111111111111111111111111111111111111111111111111111111111111"),
        b256!("0000000000000000000000000000000000000000000000000000000000000000"),
    );
    let msg = QueueMessage::LiveBlock(live_job);

    let json = serde_json::to_string(&msg).expect("serialize live block job");
    assert!(json.contains("\"schema_version\":\"v1:live_block\""));
    assert!(json.contains("5000000"));

    let deserialized: QueueMessage =
        serde_json::from_str(&json).expect("deserialize live block job");
    assert_eq!(msg, deserialized);
}

#[test]
fn test_block_range_split_half() {
    let range = BlockRangeJob::new(ChainId::ETHEREUM, 1000, 1999, "0xcontract");
    assert_eq!(range.block_count(), 1000);

    let (first, second) = range.split_half();
    assert_eq!(first.from_block, 1000);
    assert_eq!(first.to_block, 1499);
    assert_eq!(first.block_count(), 500);

    let second = second.expect("second half should exist");
    assert_eq!(second.from_block, 1500);
    assert_eq!(second.to_block, 1999);
    assert_eq!(second.block_count(), 500);

    // Single block range cannot split second half
    let single = BlockRangeJob::new(ChainId::ETHEREUM, 100, 100, "0xcontract");
    let (single_first, single_second) = single.split_half();
    assert_eq!(single_first.from_block, 100);
    assert_eq!(single_first.to_block, 100);
    assert!(single_second.is_none());
}

#[test]
fn test_event_log_idempotency_key() {
    let log = EventLog {
        address: address!("88e6a0c2ddd26feeb64f039a2c41296fcb3f5640"),
        topics: vec![b256!(
            "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
        )],
        data: Bytes::from(vec![0x01, 0x02]),
        tx_hash: b256!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        log_index: 42,
        tx_index: 3,
        block_number: 18000000,
        block_hash: b256!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    };

    let key = log.idempotency_key(ChainId::ETHEREUM);
    assert!(key.starts_with("1:0xbbbbbbbb"));
    assert!(key.ends_with(":42"));
}

#[test]
fn test_checkpoint_creation() {
    let cp = Checkpoint::new(
        ChainId::ARBITRUM_SEPOLIA,
        1500000,
        b256!("cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"),
        true,
    );
    assert_eq!(cp.chain_id, ChainId::ARBITRUM_SEPOLIA);
    assert_eq!(cp.last_indexed_block, 1500000);
    assert!(cp.is_finalized);
}

#[test]
fn test_block_envelope_ocp_extensibility() {
    use logrix_core::domain::{BlockEnvelope, EnvelopeKind};

    let envelope = BlockEnvelope::new(
        ChainId::ETHEREUM,
        19000000,
        b256!("1111111111111111111111111111111111111111111111111111111111111111"),
        b256!("0000000000000000000000000000000000000000000000000000000000000000"),
        1700000000,
        vec![],
    )
    .with_kind(EnvelopeKind::Trace)
    .with_extra("trace_count", serde_json::json!(150))
    .with_extra("gas_used", serde_json::json!("0x5208"));

    assert_eq!(envelope.kind, EnvelopeKind::Trace);
    assert_eq!(envelope.extra["trace_count"], 150);

    // Verify wire serialization
    let json = serde_json::to_string(&envelope).expect("serialize envelope");
    assert!(json.contains("\"kind\":\"trace\""));
    assert!(json.contains("\"trace_count\":150"));

    let deserialized: BlockEnvelope = serde_json::from_str(&json).expect("deserialize envelope");
    assert_eq!(deserialized.kind, EnvelopeKind::Trace);
    assert_eq!(deserialized.extra["gas_used"], "0x5208");
}

#[test]
fn test_custom_queue_message_ocp() {
    let custom_msg = QueueMessage::Custom {
        job_id: "plugin-job-99".to_string(),
        chain_id: ChainId::BASE,
        job_type: "parquet_export".to_string(),
        attempt: 1,
        payload: serde_json::json!({ "s3_bucket": "archive-bucket", "format": "parquet" }),
    };

    assert_eq!(custom_msg.job_id(), "plugin-job-99");
    assert_eq!(custom_msg.chain_id(), ChainId::BASE);
    assert_eq!(custom_msg.attempt(), 1);

    let json = serde_json::to_string(&custom_msg).expect("serialize custom queue msg");
    assert!(json.contains("\"schema_version\":\"v1:custom\""));
    assert!(json.contains("\"parquet_export\""));

    let deserialized: QueueMessage =
        serde_json::from_str(&json).expect("deserialize custom queue msg");
    assert_eq!(custom_msg, deserialized);
}
