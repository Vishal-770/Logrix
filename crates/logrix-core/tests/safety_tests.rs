use alloy_primitives::B256;
use logrix_core::domain::{
    BlockRangeJob, ChainId, Checkpoint, MessageHandle, QueueMessage, QueueType,
};
use logrix_core::error::ErrorClass;

#[test]
fn test_block_range_rejects_inverted_input() {
    let error = BlockRangeJob::try_new(ChainId::ETHEREUM, 2, 1, "invalid")
        .expect_err("inverted block range must fail");
    assert_eq!(error.class, ErrorClass::Validation);
}

#[test]
fn test_checkpoint_rejects_zero_hash() {
    let error = Checkpoint::try_new(ChainId::ETHEREUM, 10, B256::ZERO, false)
        .expect_err("zero block hash must fail");
    assert_eq!(error.class, ErrorClass::Validation);
}

#[test]
fn test_message_handle_requires_queue_lineage() {
    let message = QueueMessage::Custom {
        job_id: "job-1".to_string(),
        chain_id: ChainId::BASE,
        job_type: "test".to_string(),
        attempt: 1,
        payload: serde_json::json!({}),
    };
    let handle = MessageHandle::new("receipt-1", message, QueueType::Webhook);
    assert_eq!(handle.source_queue, QueueType::Webhook);
}
