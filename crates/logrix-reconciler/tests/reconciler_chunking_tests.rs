use alloy_primitives::B256;
use logrix_core::domain::{ChainId, Checkpoint, QueueMessage, QueueType};
use logrix_core::ports::{QueuePort, StorePort};
use logrix_reconciler::GapReconciler;
use logrix_testkit::{MockChainPort, MockQueuePort, MockStorePort};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_gap_reconciler_respects_configured_chunk_size() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_store = Arc::new(MockStorePort::new());
    let mock_queue = Arc::new(MockQueuePort::new());
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    let cp = Checkpoint::try_new(chain_id, 100, B256::repeat_byte(1), true).expect("valid hash");
    mock_store
        .write_events_and_checkpoint(&[], &cp)
        .await
        .unwrap();

    *mock_chain.head_block.lock().await = 125;

    let reconciler = GapReconciler::new(
        chain_id,
        mock_store,
        mock_queue.clone(),
        mock_chain,
        Duration::from_secs(10),
    )
    .with_chunk_size(10);

    let scheduled = reconciler.check_and_reconcile().await.unwrap();
    assert_eq!(scheduled, 24);

    // 24 blocks (101..=124) split into chunks of 10:
    // [101..=110, 111..=120, 121..=124] => 3 distinct backfill range jobs
    assert_eq!(mock_queue.depth(QueueType::Backfill).await.unwrap(), 3);

    let h1 = mock_queue
        .consume(QueueType::Backfill)
        .await
        .unwrap()
        .unwrap();
    if let QueueMessage::BackfillRange(job) = h1.message {
        assert_eq!(job.from_block, 101);
        assert_eq!(job.to_block, 110);
    } else {
        panic!("Expected BackfillRange job");
    }

    let h2 = mock_queue
        .consume(QueueType::Backfill)
        .await
        .unwrap()
        .unwrap();
    if let QueueMessage::BackfillRange(job) = h2.message {
        assert_eq!(job.from_block, 111);
        assert_eq!(job.to_block, 120);
    } else {
        panic!("Expected BackfillRange job");
    }

    let h3 = mock_queue
        .consume(QueueType::Backfill)
        .await
        .unwrap()
        .unwrap();
    if let QueueMessage::BackfillRange(job) = h3.message {
        assert_eq!(job.from_block, 121);
        assert_eq!(job.to_block, 124);
    } else {
        panic!("Expected BackfillRange job");
    }
}
