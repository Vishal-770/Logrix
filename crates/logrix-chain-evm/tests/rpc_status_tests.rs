use logrix_chain_evm::{EvmChainClient, EvmClientConfig};
use logrix_core::{
    domain::ChainId,
    error::{ErrorClass, ErrorSource},
    ports::ChainPort,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn spawn_mock_server(status: u16, body: &'static str) -> (String, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_clone = hits.clone();

    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            hits_clone.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 1024];
            let n = socket.read(&mut buf).await.expect("socket read");
            if n == 0 {
                continue;
            }
            let response = format!(
                "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{}",
                status,
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("socket write_all");
        }
    });

    (format!("http://{}", addr), hits)
}

#[tokio::test]
async fn test_rpc_call_status_400_fails_permanently() {
    let (url, hits) = spawn_mock_server(400, "Bad Request").await;
    let client = EvmChainClient::new_with_config(ChainId::new(1), url, EvmClientConfig::default());

    let err = client.get_latest_block_number().await.unwrap_err();
    assert_eq!(err.class, ErrorClass::Permanent);
    assert!(err.message.contains("400"));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_rpc_call_status_401_fails_permanently_without_looping() {
    let (url, hits) = spawn_mock_server(401, "Unauthorized").await;
    let client = EvmChainClient::new_with_config(ChainId::new(1), url, EvmClientConfig::default());

    let err = client.get_latest_block_number().await.unwrap_err();
    assert_eq!(err.class, ErrorClass::Permanent);
    assert_eq!(err.source, ErrorSource::ChainRpc);
    assert!(err.message.contains("401"));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_rpc_call_status_403_fails_permanently() {
    let (url, hits) = spawn_mock_server(403, "Forbidden").await;
    let client = EvmChainClient::new_with_config(ChainId::new(1), url, EvmClientConfig::default());

    let err = client.get_latest_block_number().await.unwrap_err();
    assert_eq!(err.class, ErrorClass::Permanent);
    assert!(err.message.contains("403"));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_rpc_call_status_404_fails_permanently() {
    let (url, hits) = spawn_mock_server(404, "Not Found").await;
    let client = EvmChainClient::new_with_config(ChainId::new(1), url, EvmClientConfig::default());

    let err = client.get_latest_block_number().await.unwrap_err();
    assert_eq!(err.class, ErrorClass::Permanent);
    assert!(err.message.contains("404"));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_rpc_call_status_429_returns_rate_limited() {
    let (url, hits) = spawn_mock_server(429, "Too Many Requests").await;
    let client = EvmChainClient::new_with_config(ChainId::new(1), url, EvmClientConfig::default());

    let err = client.get_latest_block_number().await.unwrap_err();
    assert!(matches!(err.class, ErrorClass::RateLimited { .. }));
    assert!(hits.load(Ordering::SeqCst) >= 3);
}

#[tokio::test]
async fn test_rpc_call_status_503_retries_and_exhausts() {
    let (url, hits) = spawn_mock_server(503, "Service Unavailable").await;
    let client = EvmChainClient::new_with_config(ChainId::new(1), url, EvmClientConfig::default());

    let err = client.get_latest_block_number().await.unwrap_err();
    assert_eq!(err.class, ErrorClass::Transient);
    assert!(hits.load(Ordering::SeqCst) >= 3);
}

#[tokio::test]
async fn test_rpc_call_2xx_missing_result_and_error_is_protocol_failure() {
    let (url, hits) = spawn_mock_server(200, r#"{"jsonrpc":"2.0","id":1}"#).await;
    let config = EvmClientConfig::default().with_chunk_bounds(100, 10, 200);
    let client = EvmChainClient::new_with_config(ChainId::new(1), url, config);

    let err = client.get_latest_block_number().await.unwrap_err();
    assert_eq!(err.class, ErrorClass::Permanent);
    assert_eq!(err.source, ErrorSource::ChainRpc);
    assert!(err.message.contains("neither result nor error"));
    assert_eq!(client.chunker().chunk_size(), 50);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_rpc_call_range_limit_error_returns_shrinkable_and_halves_chunk() {
    let response_body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32005,"message":"query returned more than 10000 results"}}"#;
    let (url, hits) = spawn_mock_server(200, response_body).await;
    let config = EvmClientConfig::default().with_chunk_bounds(100, 10, 200);
    let client = EvmChainClient::new_with_config(ChainId::new(1), url, config);

    let err = client.get_latest_block_number().await.unwrap_err();
    assert_eq!(err.class, ErrorClass::Shrinkable);
    assert_eq!(err.source, ErrorSource::ChainRpc);
    assert!(err.message.contains("range limit exceeded"));
    assert_eq!(client.chunker().chunk_size(), 50);
    // Must immediately return Shrinkable without executing futile retries
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}
