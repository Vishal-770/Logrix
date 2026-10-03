use aws_sdk_sqs::Client as SqsClient;
use logrix_core::domain::{
    BlockRangeJob, ChainId, LiveBlockJob, MessageHandle, QueueMessage, QueueType,
};
use logrix_queue_sqs::{
    batch::{build_delete_batch_entry, build_send_batch_entry},
    SqsClientConfig,
};
use std::collections::HashMap;

#[test]
fn test_sqs_message_json_roundtrip() {
    let job = BlockRangeJob::new(ChainId::new(421614), 1000, 1500, "421614:1000-1500");
    let msg = QueueMessage::BackfillRange(job);

    let serialized = serde_json::to_string(&msg).expect("serialize SQS message");
    assert!(serialized.contains("v1:backfill_range"));
    assert!(serialized.contains("1000"));

    let deserialized: QueueMessage =
        serde_json::from_str(&serialized).expect("deserialize SQS message");
    assert_eq!(msg, deserialized);
}

#[test]
fn test_message_handle_source_queue_scoping() {
    let job = LiveBlockJob::new(
        ChainId::new(1),
        100,
        alloy_primitives::B256::ZERO,
        alloy_primitives::B256::ZERO,
    );
    let msg = QueueMessage::LiveBlock(job);

    let handle_unscoped = MessageHandle::new("rcpt_1", msg.clone());
    assert_eq!(handle_unscoped.source_queue, None);

    let handle_scoped = MessageHandle::with_queue("rcpt_2", msg, QueueType::Backfill);
    assert_eq!(handle_scoped.source_queue, Some(QueueType::Backfill));
    assert_eq!(handle_scoped.receipt_id, "rcpt_2");
}

#[test]
fn test_sqs_batch_entry_builders() {
    let job = BlockRangeJob::new(ChainId::new(1), 200, 300, "range-1");
    let msg = QueueMessage::BackfillRange(job);

    let send_entry = build_send_batch_entry("msg_1", &msg).expect("build send entry");
    assert_eq!(send_entry.id(), "msg_1");

    let del_entry = build_delete_batch_entry("del_1", "rcpt_xyz").expect("build del entry");
    assert_eq!(del_entry.id(), "del_1");
    assert_eq!(del_entry.receipt_handle(), "rcpt_xyz");
}

#[tokio::test]
async fn test_sqs_client_config_url_resolution() {
    let mut map = HashMap::new();
    map.insert(
        QueueType::Live,
        "https://sqs.us-east-1.amazonaws.com/123/live".into(),
    );

    let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .load()
        .await;
    let client = SqsClient::new(&sdk_config);
    let config = SqsClientConfig::new(client, map);

    assert!(config.resolve_url(QueueType::Live).is_ok());
    assert!(config.resolve_url(QueueType::Backfill).is_err());
}
