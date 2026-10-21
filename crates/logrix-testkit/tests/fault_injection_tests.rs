use alloy_primitives::B256;
use logrix_core::{
    domain::{ChainId, Checkpoint, QueueMessage, QueueType},
    ports::{ChainPort, QueuePort, StorePort},
};
use logrix_testkit::{MockChainPort, MockQueuePort, MockStorePort};
use std::time::Duration;

#[tokio::test]
async fn test_mock_chain_fault_injection_and_latency() {
    let chain = MockChainPort::new(ChainId::new(1));
    assert_eq!(chain.get_latest_block_number().await.unwrap(), 0);

    // Latency
    chain.set_simulated_latency(Duration::from_millis(10)).await;
    let start = std::time::Instant::now();
    assert_eq!(chain.get_latest_block_number().await.unwrap(), 0);
    assert!(start.elapsed() >= Duration::from_millis(9));

    // Inject 2 transient RPC errors
    chain.inject_transient_errors(2);
    assert!(chain.get_latest_block_number().await.is_err());
    assert!(chain.get_latest_block_number().await.is_err());
    assert_eq!(chain.get_latest_block_number().await.unwrap(), 0);
}

#[tokio::test]
async fn test_mock_queue_fault_injection_and_bounded_capacity() {
    let queue = MockQueuePort::new();
    let msg = QueueMessage::LiveBlock(logrix_core::domain::LiveBlockJob::new(
        ChainId::new(1),
        100,
        B256::ZERO,
        B256::ZERO,
    ));

    // Capacity check
    queue.set_max_capacity(1).await;
    assert!(queue.publish(QueueType::Live, &msg).await.is_ok());
    assert!(queue.publish(QueueType::Live, &msg).await.is_err()); // rejected by backpressure

    // Drain
    let handle = queue.consume(QueueType::Live).await.unwrap().unwrap();
    queue.ack(&handle).await.unwrap();

    // Fault injection
    queue.inject_transient_errors(1);
    assert!(queue.publish(QueueType::Live, &msg).await.is_err());
    assert!(queue.publish(QueueType::Live, &msg).await.is_ok());
    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 1);
}

#[tokio::test]
async fn test_mock_queue_visibility_timeout() {
    let queue = MockQueuePort::new();
    let msg = QueueMessage::LiveBlock(logrix_core::domain::LiveBlockJob::new(
        ChainId::new(1),
        100,
        B256::ZERO,
        B256::ZERO,
    ));

    queue
        .set_visibility_timeout(Duration::from_millis(20))
        .await;
    queue.publish(QueueType::Live, &msg).await.unwrap();

    // Consume without acking
    let handle = queue.consume(QueueType::Live).await.unwrap().unwrap();
    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 0);

    // Before timeout: still 0 visible
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 0);

    // After timeout: message returns to visible queue
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 1);

    // Consume again and ack
    let handle2 = queue.consume(QueueType::Live).await.unwrap().unwrap();
    assert_eq!(handle2.message, handle.message);
    queue.ack(&handle2).await.unwrap();
    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 0);
}

#[tokio::test]
async fn test_mock_store_fault_injection_and_latency() {
    let store = MockStorePort::new();
    let hash = B256::repeat_byte(1);
    let cp = Checkpoint::try_new(ChainId::new(1), 100, hash, true).unwrap();

    store.set_simulated_latency(Duration::from_millis(10)).await;
    let start = std::time::Instant::now();
    assert!(store
        .get_checkpoint(ChainId::new(1))
        .await
        .unwrap()
        .is_none());
    assert!(start.elapsed() >= Duration::from_millis(9));

    store.inject_transient_errors(1);
    assert!(store.write_events_and_checkpoint(&[], &cp).await.is_err());
    assert!(store.write_events_and_checkpoint(&[], &cp).await.is_ok());
    assert_eq!(
        store
            .get_checkpoint(ChainId::new(1))
            .await
            .unwrap()
            .unwrap()
            .last_indexed_block,
        100
    );
}
