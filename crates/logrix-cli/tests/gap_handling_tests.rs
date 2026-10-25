use alloy_primitives::B256;
use logrix_cli::{
    handlers::handle_live_block,
    processor::{ProcessorConfig, ProcessorContext},
};
use logrix_core::{
    domain::{ChainId, LiveBlockJob, MessageHandle, QueueMessage, QueueType},
    ports::QueuePort,
};
use logrix_reconciler::{ReorgDetector, ReorgHandler};
use logrix_rpc_gateway::{ManagedProvider, ProviderPool, RpcGateway};
use logrix_store_postgres::PostgresStore;
use logrix_testkit::{MockQueuePort, MockStorePort};
use sqlx::PgPool;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn start_mock_rpc_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req_str = String::from_utf8_lossy(&buf[..n]);
                let json_part = req_str.split("\r\n\r\n").nth(1).unwrap_or("{}");
                let parsed: serde_json::Value =
                    serde_json::from_str(json_part).unwrap_or(serde_json::json!({}));

                let block_res = serde_json::json!({
                    "number": "0xc8",
                    "hash": "0x00000000000000000000000000000000000000000000000000000000000000c8",
                    "parentHash": "0x00000000000000000000000000000000000000000000000000000000000000c7",
                    "timestamp": "0x100"
                });
                let resp_body = if let Some(arr) = parsed.as_array() {
                    let mut out = Vec::new();
                    for item in arr {
                        let id = item.get("id").cloned().unwrap_or(serde_json::json!(1));
                        let method = item.get("method").and_then(|m| m.as_str()).unwrap_or("");
                        let result = if method == "eth_getBlockByNumber" {
                            block_res.clone()
                        } else {
                            serde_json::json!([])
                        };
                        out.push(
                            serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                        );
                    }
                    serde_json::to_string(&out).unwrap()
                } else {
                    let id = parsed.get("id").cloned().unwrap_or(serde_json::json!(1));
                    serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": block_res })
                        .to_string()
                };

                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    resp_body.len(),
                    resp_body
                );
                let _ = socket.write_all(resp.as_bytes()).await;
            });
        }
    });
    format!("http://{}", addr)
}

#[tokio::test]
async fn test_gap_detected_acks_and_deduplicates_backfill() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let rpc_url = start_mock_rpc_server().await;
    let pool = ProviderPool::new(chain_id);
    pool.add_provider(ManagedProvider::new("test", &rpc_url, 1, chain_id))
        .await;
    let gateway = Arc::new(RpcGateway::new(chain_id, pool, None, None));
    let detector = ReorgDetector::new(chain_id, gateway.clone(), 8);

    // Seed detector buffer with block 100 so block 200 triggers GapDetected
    let tip_block = logrix_core::domain::BlockRef::new(100, B256::repeat_byte(1));
    detector.buffer().write().await.push(tip_block);

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
    let last_dispatched_gap = Arc::new(AtomicU64::new(100));

    let ctx = ProcessorContext {
        config: &config,
        store: &store,
        blob_store: None,
        detector: &detector,
        handler: &handler,
        gateway: &gateway,
        queue: &queue,
        engine: None,
        last_dispatched_gap: last_dispatched_gap.clone(),
    };

    let msg = QueueMessage::LiveBlock(LiveBlockJob::new(
        chain_id,
        200,
        B256::repeat_byte(2),
        B256::repeat_byte(1),
    ));
    let handle1 = MessageHandle::new("msg-200", msg.clone(), QueueType::Live);

    // First live block 200 triggers GapDetected from 101 to 200
    let res = handle_live_block(&ctx, 200, &handle1).await;
    assert!(res.is_ok());

    // Verified: Message was ACKed, NEVER nacked
    assert_eq!(*mock_queue.acked.lock().await, vec!["msg-200".to_string()]);
    assert!(mock_queue.nacked.lock().await.is_empty());

    // Verified: Backfill job was published spanning 101..=200
    assert_eq!(mock_queue.depth(QueueType::Backfill).await.unwrap(), 1);
    let backfill_msg = mock_queue
        .consume(QueueType::Backfill)
        .await
        .unwrap()
        .unwrap();
    if let QueueMessage::BackfillRange(range) = backfill_msg.message {
        assert_eq!(range.from_block, 101);
        assert_eq!(range.to_block, 200);
    } else {
        panic!("Expected BackfillRange message");
    }
    assert_eq!(
        last_dispatched_gap.load(std::sync::atomic::Ordering::Relaxed),
        200
    );

    // Second live block 200 arrives again (re-delivery or duplicate)
    let handle2 = MessageHandle::new("msg-200-dup", msg, QueueType::Live);
    let res2 = handle_live_block(&ctx, 200, &handle2).await;
    assert!(res2.is_ok());

    // Verified: Second message is ACKed without publishing duplicate backfill
    assert_eq!(mock_queue.depth(QueueType::Backfill).await.unwrap(), 0);
    assert!(mock_queue.nacked.lock().await.is_empty());
    assert_eq!(mock_queue.acked.lock().await.len(), 2);
}
