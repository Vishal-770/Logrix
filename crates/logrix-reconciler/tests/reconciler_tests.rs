use alloy_primitives::{Address, Bytes, B256};
use logrix_core::domain::{ChainId, Checkpoint, EventLog, QueueMessage, QueueType};
use logrix_core::ports::{QueuePort, StorePort};
use logrix_reconciler::{GapReconciler, ReorgDetector, ReorgHandler};
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
        block_timestamp: 0,
    };
    let cp = Checkpoint::try_new(chain_id, 105, B256::repeat_byte(1), true).expect("valid hash");
    mock_store
        .write_events_and_checkpoint(&[dummy_event], &cp)
        .await
        .unwrap();

    let fork_hash = B256::repeat_byte(0x42);
    handler.execute_rollback(102, fork_hash, 3).await.unwrap();

    let updated_cp = mock_store.get_checkpoint(chain_id).await.unwrap().unwrap();
    assert_eq!(updated_cp.last_indexed_block, 102);
    assert_eq!(updated_cp.last_indexed_hash, fork_hash);

    assert_eq!(
        *mock_store.rollbacks.lock().await,
        vec![(chain_id, 102, fork_hash)]
    );

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
    .with_lag_threshold(10);

    let scheduled = reconciler.check_and_reconcile().await.unwrap();
    assert_eq!(scheduled, 24);

    assert_eq!(mock_queue.depth(QueueType::Backfill).await.unwrap(), 3);
    let handle = mock_queue
        .consume(QueueType::Backfill)
        .await
        .unwrap()
        .unwrap();
    if let QueueMessage::BackfillRange(job) = handle.message {
        assert_eq!(job.from_block, 101);
        assert_eq!(job.to_block, 110);
    } else {
        panic!("Expected BackfillRange job in queue");
    }
}

#[tokio::test]
async fn test_gap_reconciler_deduplicates_in_flight_ranges() {
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
        mock_store.clone(),
        mock_queue.clone(),
        mock_chain.clone(),
        Duration::from_secs(10),
    )
    .with_lag_threshold(10);

    let scheduled = reconciler.check_and_reconcile().await.unwrap();
    assert_eq!(scheduled, 24);
    assert_eq!(mock_queue.depth(QueueType::Backfill).await.unwrap(), 3);

    let scheduled_again = reconciler.check_and_reconcile().await.unwrap();
    assert_eq!(scheduled_again, 0);
    assert_eq!(mock_queue.depth(QueueType::Backfill).await.unwrap(), 3);

    *mock_chain.head_block.lock().await = 135;
    let scheduled_delta = reconciler.check_and_reconcile().await.unwrap();
    assert_eq!(scheduled_delta, 10);
    assert_eq!(mock_queue.depth(QueueType::Backfill).await.unwrap(), 4);
}

#[tokio::test]
async fn test_gap_reconciler_graceful_shutdown() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_store = Arc::new(MockStorePort::new());
    let mock_queue = Arc::new(MockQueuePort::new());
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    let reconciler = GapReconciler::new(
        chain_id,
        mock_store,
        mock_queue,
        mock_chain,
        Duration::from_millis(50),
    )
    .with_lag_threshold(5);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let join_handle = tokio::spawn(async move {
        reconciler
            .run_loop_with_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = shutdown_tx.send(());

    tokio::time::timeout(Duration::from_secs(1), join_handle)
        .await
        .expect("Shutdown cleanly")
        .unwrap();
}

#[test]
fn test_reconciler_default_constants() {
    assert_eq!(logrix_reconciler::DEFAULT_LAG_THRESHOLD, 50);
    assert_eq!(logrix_reconciler::DEFAULT_HEAD_OFFSET, 1);
    assert_eq!(logrix_reconciler::DEFAULT_RECONCILER_CHUNK_SIZE, 10);
    assert_eq!(logrix_reconciler::DEFAULT_RING_BUFFER_CAPACITY, 128);
    assert_eq!(logrix_reconciler::DEFAULT_MAX_REORG_DEPTH, 128);
}
