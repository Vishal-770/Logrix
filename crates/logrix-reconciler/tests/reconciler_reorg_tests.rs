use alloy_primitives::B256;
use logrix_core::domain::{ChainId, Checkpoint, QueueMessage, QueueType};
use logrix_core::ports::{QueuePort, StorePort};
use logrix_reconciler::GapReconciler;
use logrix_testkit::{MockChainPort, MockQueuePort, MockStorePort};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_gap_reconciler_resets_inflight_marker_on_reorg_rollback() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_store = Arc::new(MockStorePort::new());
    let mock_queue = Arc::new(MockQueuePort::new());
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    // Initial checkpoint at 120
    let cp = Checkpoint::try_new(chain_id, 120, B256::repeat_byte(1), true).expect("valid hash");
    mock_store
        .write_events_and_checkpoint(&[], &cp)
        .await
        .unwrap();

    *mock_chain.head_block.lock().await = 130;

    let reconciler = GapReconciler::new(
        chain_id,
        mock_store.clone(),
        mock_queue.clone(),
        mock_chain.clone(),
        Duration::from_secs(10),
    )
    .with_lag_threshold(5);

    // Initial reconcile schedules 121..=129
    let scheduled = reconciler.check_and_reconcile().await.unwrap();
    assert_eq!(scheduled, 9);
    assert_eq!(mock_queue.depth(QueueType::Backfill).await.unwrap(), 1);

    // Now simulate a reorg rollback rewinding checkpoint to 110
    let rolled_back_cp =
        Checkpoint::try_new(chain_id, 110, B256::repeat_byte(2), true).expect("valid hash");
    mock_store
        .write_events_and_checkpoint(&[], &rolled_back_cp)
        .await
        .unwrap();

    // Reconciler should detect backward jump and re-dispatch from 111..=129
    let rescheduled = reconciler.check_and_reconcile().await.unwrap();
    assert_eq!(rescheduled, 19);
    assert_eq!(mock_queue.depth(QueueType::Backfill).await.unwrap(), 2);

    let _ = mock_queue.consume(QueueType::Backfill).await.unwrap();
    let second_job = mock_queue
        .consume(QueueType::Backfill)
        .await
        .unwrap()
        .unwrap();
    if let QueueMessage::BackfillRange(job) = second_job.message {
        assert_eq!(job.from_block, 111);
        assert_eq!(job.to_block, 129);
    } else {
        panic!("Expected BackfillRange job");
    }
}
