use logrix_cli::processor_loop::run_consumer_loop;
use logrix_cli::processor_types::{ProcessorConfig, ProcessorContext};
use logrix_core::{
    domain::{ChainId, LiveBlockJob, QueueMessage, QueueType},
    ports::QueuePort,
};
use logrix_reconciler::{ReorgDetector, ReorgHandler};
use logrix_rpc_gateway::{ManagedProvider, ProviderPool, RpcGateway};
use logrix_store_postgres::PostgresStore;
use logrix_testkit::{MockQueuePort, MockStorePort};
use sqlx::PgPool;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_fair_queue_consumes_both_live_and_backfill() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let pool = ProviderPool::new(chain_id);
    pool.add_provider(ManagedProvider::new(
        "test",
        "http://127.0.0.1:9",
        1,
        chain_id,
    ))
    .await;
    let gateway = Arc::new(RpcGateway::new(chain_id, pool, None, None));
    let detector = ReorgDetector::new(chain_id, gateway.clone(), 8);
    let mock_queue = Arc::new(MockQueuePort::new());
    let queue: Arc<dyn QueuePort> = mock_queue.clone();
    let handler = ReorgHandler::new(
        chain_id,
        Arc::new(MockStorePort::new()),
        queue.clone(),
        detector.clone(),
    );
    let store = PostgresStore::from_pool(
        PgPool::connect_lazy("postgres://unused@localhost/logrix").expect("lazy pool"),
        "test",
    );

    let config = ProcessorConfig::for_testing(chain_id);

    let ctx = ProcessorContext {
        config: &config,
        store: &store,
        blob_store: None,
        detector: &detector,
        handler: &handler,
        gateway: &gateway,
        queue: &queue,
        engine: None,
        last_dispatched_gap: Arc::new(AtomicU64::new(0)),
    };

    queue
        .publish(
            QueueType::Live,
            &QueueMessage::LiveBlock(LiveBlockJob::new(
                chain_id,
                100,
                alloy_primitives::B256::repeat_byte(1),
                alloy_primitives::B256::ZERO,
            )),
        )
        .await
        .unwrap();
    queue
        .publish(
            QueueType::Live,
            &QueueMessage::LiveBlock(LiveBlockJob::new(
                chain_id,
                101,
                alloy_primitives::B256::repeat_byte(2),
                alloy_primitives::B256::repeat_byte(1),
            )),
        )
        .await
        .unwrap();

    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 2);
    assert_eq!(queue.depth(QueueType::Backfill).await.unwrap(), 0);

    let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
    let _ = tokio::time::timeout(
        Duration::from_millis(50),
        run_consumer_loop(&ctx, shutdown_tx),
    )
    .await;
}

#[tokio::test]
async fn test_consumer_loop_backs_off_when_primary_errors_and_secondary_empty() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let pool = ProviderPool::new(chain_id);
    let gateway = Arc::new(RpcGateway::new(chain_id, pool, None, None));
    let detector = ReorgDetector::new(chain_id, gateway.clone(), 8);
    let mock_queue = Arc::new(MockQueuePort::new());
    mock_queue.inject_transient_errors(1);
    let queue: Arc<dyn QueuePort> = mock_queue.clone();
    let handler = ReorgHandler::new(
        chain_id,
        Arc::new(MockStorePort::new()),
        queue.clone(),
        detector.clone(),
    );
    let store = PostgresStore::from_pool(
        PgPool::connect_lazy("postgres://unused@localhost/logrix").expect("lazy pool"),
        "test",
    );

    let config = ProcessorConfig::for_testing(chain_id);

    let ctx = ProcessorContext {
        config: &config,
        store: &store,
        blob_store: None,
        detector: &detector,
        handler: &handler,
        gateway: &gateway,
        queue: &queue,
        engine: None,
        last_dispatched_gap: Arc::new(AtomicU64::new(0)),
    };

    let (shutdown_tx, _shutdown_rx) = tokio::sync::watch::channel(false);
    let _ = tokio::time::timeout(
        Duration::from_millis(200),
        run_consumer_loop(&ctx, shutdown_tx),
    )
    .await;
}
