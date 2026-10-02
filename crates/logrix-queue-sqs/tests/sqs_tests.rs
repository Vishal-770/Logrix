use logrix_core::domain::{BlockRangeJob, ChainId, QueueMessage};

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
