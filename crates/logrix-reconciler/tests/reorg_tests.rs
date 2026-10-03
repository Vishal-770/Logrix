use alloy_primitives::{Address, Bytes, B256};
use logrix_core::domain::{BlockEnvelope, ChainId, Checkpoint, EventLog, QueueMessage, QueueType};
use logrix_core::ports::{QueuePort, StorePort};
use logrix_reconciler::{
    ContinuityStatus, GapReconciler, ReorgDetector, ReorgHandler, RollingBlockBuffer,
    WebhookDispatcher,
};
use logrix_testkit::{MockChainPort, MockQueuePort, MockStorePort};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_rolling_buffer_parent_continuity_and_rewind() {
    let mut buf = RollingBlockBuffer::new(5);

    let h1 = B256::repeat_byte(0x01);
    let h2 = B256::repeat_byte(0x02);
    let h3 = B256::repeat_byte(0x03);

    buf.push(logrix_core::domain::BlockRef::new(100, h1));
    buf.push(logrix_core::domain::BlockRef::new(101, h2));
    buf.push(logrix_core::domain::BlockRef::new(102, h3));

    assert_eq!(buf.len(), 3);
    assert_eq!(buf.latest().unwrap().number, 102);

    // Continuous block 103 with parent h3
    assert!(buf.is_parent_matching(103, h3));
    // Discontinuous block 103 with wrong parent
    assert!(!buf.is_parent_matching(103, h1));

    // Rewind buffer after a 1-block reorg to 101
    buf.rewind_to(101);
    assert_eq!(buf.len(), 2);
    assert_eq!(buf.latest().unwrap().number, 101);
}

#[tokio::test]
async fn test_reorg_detector_catches_fork_and_ancestor() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    let h100 = B256::repeat_byte(0x10);
    let h101 = B256::repeat_byte(0x11);
    let h102 = B256::repeat_byte(0x12);

    let detector = ReorgDetector::new(chain_id, mock_chain.clone(), 32);

    // Feed block 100, 101, 102
    let b100 = BlockEnvelope::new(chain_id, 100, h100, B256::ZERO, 1000, vec![]);
    let b101 = BlockEnvelope::new(chain_id, 101, h101, h100, 1010, vec![]);
    let b102 = BlockEnvelope::new(chain_id, 102, h102, h101, 1020, vec![]);

    assert_eq!(
        detector.check_envelope(&b100).await.unwrap(),
        ContinuityStatus::Continuous
    );
    assert_eq!(
        detector.check_envelope(&b101).await.unwrap(),
        ContinuityStatus::Continuous
    );
    assert_eq!(
        detector.check_envelope(&b102).await.unwrap(),
        ContinuityStatus::Continuous
    );

    // Now introduce a reorganization!
    // Block 102 was reorganized on chain. Incoming block 102-fork has parent h101 (fork point is 101).
    let h102_fork = B256::repeat_byte(0x99);
    let b102_fork = BlockEnvelope::new(chain_id, 102, h102_fork, h101, 1025, vec![]);

    let status = detector.check_envelope(&b102_fork).await.unwrap();
    match status {
        ContinuityStatus::ReorgDetected {
            fork_block,
            fork_hash,
            reorg_depth,
        } => {
            assert_eq!(fork_block, 101);
            assert_eq!(fork_hash, h101);
            assert_eq!(reorg_depth, 1);
        }
        other => panic!("Expected ReorgDetected, got {:?}", other),
    }
}

#[tokio::test]
async fn test_reorg_detector_gap_and_duplicate_handling() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    let h100 = B256::repeat_byte(0x10);
    let h101 = B256::repeat_byte(0x11);
    let h105 = B256::repeat_byte(0x15);

    let detector = ReorgDetector::new(chain_id, mock_chain.clone(), 32);

    let b100 = BlockEnvelope::new(chain_id, 100, h100, B256::ZERO, 1000, vec![]);
    let b101 = BlockEnvelope::new(chain_id, 101, h101, h100, 1010, vec![]);
    let b105 = BlockEnvelope::new(chain_id, 105, h105, B256::repeat_byte(0x14), 1050, vec![]);

    assert_eq!(
        detector.check_envelope(&b100).await.unwrap(),
        ContinuityStatus::Continuous
    );
    assert_eq!(
        detector.check_envelope(&b101).await.unwrap(),
        ContinuityStatus::Continuous
    );

    // Block 105 is pushed when tip is 101 -> gap detected (102..=104)
    let status = detector.check_envelope(&b105).await.unwrap();
    assert_eq!(
        status,
        ContinuityStatus::GapDetected {
            from_block: 102,
            to_block: 104
        }
    );

    // Duplicate of block 101 with same hash -> Continuous
    assert_eq!(
        detector.check_envelope(&b101).await.unwrap(),
        ContinuityStatus::Continuous
    );

    // Block 101 with DIFFERENT hash -> Reorg at height!
    let b101_divergent =
        BlockEnvelope::new(chain_id, 101, B256::repeat_byte(0x77), h100, 1011, vec![]);
    let status_reorg = detector.check_envelope(&b101_divergent).await.unwrap();
    match status_reorg {
        ContinuityStatus::ReorgDetected { fork_block, .. } => {
            assert_eq!(fork_block, 100);
        }
        other => panic!("Expected ReorgDetected, got {:?}", other),
    }
}

#[tokio::test]
async fn test_reorg_handler_executes_atomic_rollback_and_webhook_dispatch() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_store = Arc::new(MockStorePort::new());
    let mock_queue = Arc::new(MockQueuePort::new());
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    let detector = ReorgDetector::new(chain_id, mock_chain, 32);
    let handler = ReorgHandler::new(chain_id, mock_store.clone(), mock_queue.clone(), detector);

    // Initial state: checkpoint at block 105 with events
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

    // Execute reorg rollback to block 102
    let fork_hash = B256::repeat_byte(0x42);
    handler.execute_rollback(102, fork_hash, 3).await.unwrap();

    // Verify checkpoint is rewound to block 102
    let updated_cp = mock_store.get_checkpoint(chain_id).await.unwrap().unwrap();
    assert_eq!(updated_cp.last_indexed_block, 102);

    // Verify webhook notification was published to Webhook Queue
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

    // Checkpoint is at block 100
    let cp = Checkpoint::new(chain_id, 100, B256::ZERO, true);
    mock_store
        .write_events_and_checkpoint(&[], &cp)
        .await
        .unwrap();

    // Live chain head is at block 125 (25 blocks ahead > threshold of 10)
    *mock_chain.head_block.lock().await = 125;

    let reconciler = GapReconciler::new(
        chain_id,
        mock_store,
        mock_queue.clone(),
        mock_chain,
        Duration::from_secs(10),
    );

    let scheduled = reconciler.check_and_reconcile().await.unwrap();
    assert_eq!(scheduled, 24); // 101 to 124

    // Verify Backfill job was dispatched into Backfill queue
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

    // Message must be acknowledged
    let acked = mock_queue.acked.lock().await;
    assert_eq!(acked.len(), 1);
}
