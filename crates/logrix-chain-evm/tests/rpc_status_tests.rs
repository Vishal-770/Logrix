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
        loop {
            if let Ok((mut socket, _)) = listener.accept().await {
                hits_clone.fetch_add(1, Ordering::SeqCst);
                let mut buf = [0u8; 1024];
                let _ = socket.read(&mut buf).await;
                let response = format!(
                    "HTTP/1.1 {} OK\r\nContent-Length: {}\r\nContent-Type: text/plain\r\n\r\n{}",
                    status,
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        }
    });

    (format!("http://{}", addr), hits)
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
