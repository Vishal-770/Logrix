use alloy_primitives::Address;
use logrix_cli::{
    handlers::handle_backfill_range,
    processor::{ProcessorConfig, ProcessorContext},
};
use logrix_core::{
    domain::{BlockRangeJob, ChainId, QueueMessage, QueueType},
    ports::QueuePort,
};
use logrix_reconciler::{ReorgDetector, ReorgHandler};
use logrix_rpc_gateway::{ManagedProvider, ProviderPool, RpcGateway};
use logrix_store_postgres::PostgresStore;
use logrix_testkit::{MockQueuePort, MockStorePort};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn run_backfill(block_status: u16, block_body: &'static str) -> (bool, Arc<MockQueuePort>) {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let rpc_url = spawn_rpc_server(block_status, block_body).await;
    let pool = ProviderPool::new(chain_id);
    pool.add_provider(ManagedProvider::new("test", rpc_url, 1, chain_id))
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
    let config = test_config(chain_id);
    let ctx = ProcessorContext {
        config: &config,
        store: &store,
        blob_store: None,
        detector: &detector,
        handler: &handler,
        gateway: &gateway,
        queue: &queue,
        engine: None,
    };
    let job = BlockRangeJob::new(chain_id, 100, 100, "test:100-100");
    queue
        .publish(
            QueueType::Backfill,
            &QueueMessage::BackfillRange(job.clone()),
        )
        .await
        .expect("publish backfill");
    let handle = queue
        .consume(QueueType::Backfill)
        .await
        .expect("consume backfill")
        .expect("backfill handle");

    (
        handle_backfill_range(&ctx, &job, &handle).await.is_ok(),
        mock_queue,
    )
}

fn test_config(chain_id: ChainId) -> ProcessorConfig {
    ProcessorConfig {
        chain_id,
        database_url: "postgres://unused@localhost/logrix".to_string(),
        database_max_connections: 1,
        database_min_connections: 0,
        target_contract: Address::ZERO,
        ring_buffer_depth: 8,
        webhook_url: None,
        webhook_secret: String::new(),
        reconciler_interval_secs: 30,
        reconciler_chunk_size: None,
        reconciler_lag_threshold: 50,
        manifest_path: None,
        enable_webhooks: false,
        s3_bucket: None,
        s3_endpoint: None,
        s3_prefix: None,
        wasm_fuel_limit: 1_000_000,
        wasm_max_memory_mb: 128,
        webhook_timeout_secs: 5,
        webhook_max_retries: 1,
        webhook_worker_idle_ms: 100,
        webhook_worker_backoff_ms: 100,
    }
}

async fn spawn_rpc_server(block_status: u16, block_body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind RPC server");
    let address = listener.local_addr().expect("RPC server address");
    tokio::spawn(async move {
        for request_number in 0..2 {
            let (mut socket, _) = listener.accept().await.expect("accept RPC request");
            let mut request = [0_u8; 4096];
            let _ = socket.read(&mut request).await;
            let (status, body) = if request_number == 0 {
                (200, r#"{"jsonrpc":"2.0","id":1,"result":[]}"#)
            } else {
                (block_status, block_body)
            };
            let response = format!(
                "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("write RPC response");
        }
    });
    format!("http://{address}")
}

#[tokio::test]
async fn missing_canonical_block_is_requeued_without_error() {
    let (succeeded, queue) = run_backfill(200, r#"{"jsonrpc":"2.0","id":2,"result":null}"#).await;

    assert!(succeeded);
    assert_eq!(queue.acked.lock().await.len(), 0);
    let nacked = queue.nacked.lock().await;
    assert_eq!(nacked.len(), 1);
    assert!(nacked[0].1);
    assert_eq!(queue.depth(QueueType::Backfill).await.unwrap(), 1);
}

#[tokio::test]
async fn canonical_block_lookup_failure_is_requeued_without_error() {
    let (succeeded, queue) = run_backfill(400, "canonical lookup failed").await;

    assert!(succeeded);
    assert_eq!(queue.acked.lock().await.len(), 0);
    let nacked = queue.nacked.lock().await;
    assert_eq!(nacked.len(), 1);
    assert!(nacked[0].1);
    assert_eq!(queue.depth(QueueType::Backfill).await.unwrap(), 1);
}
