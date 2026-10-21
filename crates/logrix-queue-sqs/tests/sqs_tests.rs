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
    let job = BlockRangeJob::try_new(ChainId::new(421614), 1000, 1500, "421614:1000-1500")
        .expect("valid range");
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

    let handle = MessageHandle::new("rcpt_1", msg, QueueType::Backfill);
    assert_eq!(handle.source_queue, QueueType::Backfill);
    assert_eq!(handle.receipt_id, "rcpt_1");
}

#[test]
fn test_sqs_batch_entry_builders() {
    let job = BlockRangeJob::try_new(ChainId::new(1), 200, 300, "range-1").expect("valid range");
    let msg = QueueMessage::BackfillRange(job);

    let send_entry = build_send_batch_entry("msg_1", &msg).expect("build send entry");
    assert_eq!(send_entry.id(), "msg_1");

    let del_entry = build_delete_batch_entry("del_1", "rcpt_xyz").expect("build del entry");
    assert_eq!(del_entry.id(), "del_1");
    assert_eq!(del_entry.receipt_handle(), "rcpt_xyz");
}

#[test]
fn test_sqs_visibility_constants() {
    use logrix_queue_sqs::{
        DEFAULT_MAX_MESSAGES, DEFAULT_WAIT_TIME_SECONDS, SQS_BUFFERED_VISIBILITY_TIMEOUT,
        SQS_MAX_VISIBILITY_TIMEOUT,
    };
    assert_eq!(SQS_MAX_VISIBILITY_TIMEOUT, 43_200);
    assert_eq!(SQS_BUFFERED_VISIBILITY_TIMEOUT, 120);
    assert_eq!(DEFAULT_WAIT_TIME_SECONDS, 2);
    assert_eq!(DEFAULT_MAX_MESSAGES, 1);
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

#[tokio::test]
async fn test_sqs_config_builder() {
    let builder_res = SqsClientConfig::builder()
        .live_queue_url("https://sqs.us-east-1.amazonaws.com/123/live")
        .backfill_queue_url("https://sqs.us-east-1.amazonaws.com/123/backfill")
        .webhook_queue_url("https://sqs.us-east-1.amazonaws.com/123/webhook")
        .dlq_url("https://sqs.us-east-1.amazonaws.com/123/dlq")
        .endpoint_override("http://localhost:4566")
        .region("us-east-1")
        .build()
        .await;

    assert!(builder_res.is_ok());
    let cfg = builder_res.unwrap();
    assert_eq!(
        cfg.resolve_url(QueueType::Live).unwrap(),
        "https://sqs.us-east-1.amazonaws.com/123/live"
    );
    assert_eq!(
        cfg.resolve_url(QueueType::Backfill).unwrap(),
        "https://sqs.us-east-1.amazonaws.com/123/backfill"
    );
    assert_eq!(
        cfg.resolve_url(QueueType::Webhook).unwrap(),
        "https://sqs.us-east-1.amazonaws.com/123/webhook"
    );
    assert_eq!(
        cfg.resolve_url(QueueType::DeadLetter).unwrap(),
        "https://sqs.us-east-1.amazonaws.com/123/dlq"
    );
}
