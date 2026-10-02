use alloy_primitives::{address, b256, Bytes};
use logrix_core::{
    domain::{
        BlockRangeJob, ChainId, Checkpoint, EventLog, LiveBlockJob, QueueMessage,
    },
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

    let deserialized: QueueMessage = serde_json::from_str(&json).expect("deserialize live block job");
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
        topics: vec![b256!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")],
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
