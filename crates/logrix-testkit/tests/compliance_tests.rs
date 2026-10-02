use alloy_primitives::{address, b256, Bytes};
use logrix_core::{
    domain::{ChainId, Checkpoint, EventLog, LiveBlockJob, QueueMessage, QueueType},
    ports::{QueuePort, StorePort},
};
use logrix_testkit::{MockQueuePort, MockStorePort};

#[tokio::test]
async fn test_queue_port_lifecycle() {
    let queue = MockQueuePort::new();

    let msg = QueueMessage::LiveBlock(LiveBlockJob::new(
        ChainId::ARBITRUM_SEPOLIA,
        100,
        b256!("1111111111111111111111111111111111111111111111111111111111111111"),
        b256!("0000000000000000000000000000000000000000000000000000000000000000"),
    ));

    // Initially empty
    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 0);

    // Publish
    queue.publish(QueueType::Live, &msg).await.unwrap();
    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 1);

    // Consume
    let handle = queue.consume(QueueType::Live).await.unwrap().expect("handle");
    assert_eq!(handle.message, msg);
    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 0);

    // Nack with requeue
    queue.nack(&handle, true).await.unwrap();
    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 1);

    // Consume again and Dead Letter
    let handle2 = queue.consume(QueueType::Live).await.unwrap().expect("handle");
    queue.dead_letter(&handle2, "poison block").await.unwrap();
    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 0);
    assert_eq!(queue.depth(QueueType::DeadLetter).await.unwrap(), 1);
}

#[tokio::test]
async fn test_store_port_atomic_writes_and_reorg_rollback() {
    let store = MockStorePort::new();
    let chain_id = ChainId::ARBITRUM_SEPOLIA;

    let log1 = EventLog {
        address: address!("1111111111111111111111111111111111111111"),
        topics: vec![b256!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")],
        data: Bytes::from(vec![0x01]),
        tx_hash: b256!("1111111111111111111111111111111111111111111111111111111111111111"),
        log_index: 0,
        tx_index: 0,
        block_number: 100,
        block_hash: b256!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    };

    let log2 = EventLog {
        address: address!("2222222222222222222222222222222222222222"),
        topics: vec![b256!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")],
        data: Bytes::from(vec![0x02]),
        tx_hash: b256!("2222222222222222222222222222222222222222222222222222222222222222"),
        log_index: 0,
        tx_index: 0,
        block_number: 101,
        block_hash: b256!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    };

    // Atomic write block 100
    let cp1 = Checkpoint::new(chain_id, 100, log1.block_hash, false);
    store.write_events_and_checkpoint(&[log1.clone()], &cp1).await.unwrap();

    // Atomic write block 101
    let cp2 = Checkpoint::new(chain_id, 101, log2.block_hash, false);
    store.write_events_and_checkpoint(&[log2.clone()], &cp2).await.unwrap();

    assert_eq!(store.total_events().await, 2);
    let cp = store.get_checkpoint(chain_id).await.unwrap().expect("checkpoint");
    assert_eq!(cp.last_indexed_block, 101);

    // Rollback to block 100 (e.g. block 101 was reorged)
    store.rollback_to_block(chain_id, 100).await.unwrap();

    // Event 101 must be deleted, event 100 remains
    assert_eq!(store.total_events().await, 1);
    let events = store.get_events_for_chain(chain_id).await;
    assert_eq!(events[0].block_number, 100);

    let cp_after_reorg = store.get_checkpoint(chain_id).await.unwrap().expect("checkpoint");
    assert_eq!(cp_after_reorg.last_indexed_block, 100);
}

#[tokio::test]
async fn test_mock_chain_port() {
    use logrix_core::domain::BlockEnvelope;
    use logrix_core::ports::ChainPort;
    use logrix_testkit::MockChainPort;

    let chain = MockChainPort::new(ChainId::ARBITRUM_SEPOLIA);
    let target_addr = address!("1111111111111111111111111111111111111111");

    let log = EventLog {
        address: target_addr,
        topics: vec![b256!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")],
        data: Bytes::from(vec![0x01]),
        tx_hash: b256!("1111111111111111111111111111111111111111111111111111111111111111"),
        log_index: 0,
        tx_index: 0,
        block_number: 100,
        block_hash: b256!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    };

    let envelope = BlockEnvelope {
        chain_id: ChainId::ARBITRUM_SEPOLIA,
        block_number: 100,
        block_hash: log.block_hash,
        parent_hash: b256!("0000000000000000000000000000000000000000000000000000000000000000"),
        timestamp: 1700000000,
        logs: vec![log.clone()],
    };

    chain.add_block(envelope).await;

    assert_eq!(chain.get_latest_block_number().await.unwrap(), 100);
    let block_ref = chain.get_block_by_number(100).await.unwrap().expect("block ref");
    assert_eq!(block_ref.number, 100);

    let logs = chain.fetch_logs(100, 100, &[target_addr]).await.unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].address, target_addr);
}
