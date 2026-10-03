use alloy_primitives::{Address, Bytes, B256};
use logrix_core::domain::{ChainId, Checkpoint, EventLog, QueueMessage, QueueType};
use logrix_core::ports::{QueuePort, StorePort};
use logrix_reconciler::{GapReconciler, ReorgDetector, ReorgHandler, WebhookDispatcher};
use logrix_testkit::{MockChainPort, MockQueuePort, MockStorePort};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_reorg_handler_executes_atomic_rollback_and_webhook_dispatch() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_store = Arc::new(MockStorePort::new());
    let mock_queue = Arc::new(MockQueuePort::new());
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    let detector = ReorgDetector::new(chain_id, mock_chain, 32);
    let handler = ReorgHandler::new(chain_id, mock_store.clone(), mock_queue.clone(), detector);

    let dummy_event = EventLog {
        address: Address::ZERO,
        topics: vec![],
        data: Bytes::new(),
        tx_hash: B256::ZERO,
        log_index: 0,
        tx_index: 0,
        block_number: 104,
        block_hash: B256::ZERO,
    };
    let cp = Checkpoint::new(chain_id, 105, B256::ZERO, true);
    mock_store
        .write_events_and_checkpoint(&[dummy_event], &cp)
        .await
        .unwrap();

    let fork_hash = B256::repeat_byte(0x42);
    handler.execute_rollback(102, fork_hash, 3).await.unwrap();

    let updated_cp = mock_store.get_checkpoint(chain_id).await.unwrap().unwrap();
    assert_eq!(updated_cp.last_indexed_block, 102);

    assert_eq!(mock_queue.depth(QueueType::Webhook).await.unwrap(), 1);
    let handle = mock_queue
        .consume(QueueType::Webhook)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(handle.message.chain_id(), chain_id);
}

#[tokio::test]
async fn test_gap_reconciler_detects_and_refills_missing_ranges() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_store = Arc::new(MockStorePort::new());
    let mock_queue = Arc::new(MockQueuePort::new());
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    let cp = Checkpoint::new(chain_id, 100, B256::ZERO, true);
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
    );

    let scheduled = reconciler.check_and_reconcile().await.unwrap();
    assert_eq!(scheduled, 24);

    assert_eq!(mock_queue.depth(QueueType::Backfill).await.unwrap(), 1);
    let handle = mock_queue
        .consume(QueueType::Backfill)
        .await
        .unwrap()
        .unwrap();
    if let QueueMessage::BackfillRange(job) = handle.message {
        assert_eq!(job.from_block, 101);
        assert_eq!(job.to_block, 124);
    } else {
        panic!("Expected BackfillRange job in queue");
    }
}

#[tokio::test]
async fn test_webhook_dispatcher_no_target_acks_immediately() {
    let mock_queue = Arc::new(MockQueuePort::new());
    let dispatcher = WebhookDispatcher::new(mock_queue.clone(), None);

    let msg = QueueMessage::Custom {
        job_id: "test-reorg-job-1".to_string(),
        chain_id: ChainId::ARBITRUM_SEPOLIA,
        job_type: "event.reverted".to_string(),
        attempt: 1,
        payload: serde_json::json!({ "fork_block": 100 }),
    };
    mock_queue.publish(QueueType::Webhook, &msg).await.unwrap();

    let processed = dispatcher.process_one().await.unwrap();
    assert!(processed);

    let acked = mock_queue.acked.lock().await;
    assert_eq!(acked.len(), 1);
}
