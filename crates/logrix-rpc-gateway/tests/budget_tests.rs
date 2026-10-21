use logrix_core::domain::ChainId;
use logrix_core::error::ErrorClass;
use logrix_rpc_gateway::{ManagedProvider, ProviderPool, RpcGateway};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_concurrent_cu_budget_reservation_prevents_overspend() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let pool = ProviderPool::new(chain_id);
    let p = ManagedProvider::new("node", "http://localhost:8545", 1, chain_id);
    pool.add_provider(p).await;

    // Budget of 100 CUs allows only ONE eth_getLogs (75 CUs)
    let gateway = Arc::new(RpcGateway::new(chain_id, pool, Some(100), None));
    let mut tasks = Vec::new();

    for _ in 0..10 {
        let g = Arc::clone(&gateway);
        tasks.push(tokio::spawn(async move {
            g.execute_with_fallback("eth_getLogs", |_| async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                Ok("logs".to_string())
            })
            .await
        }));
    }

    let mut successes = 0;
    let mut rate_limited = 0;
    for t in tasks {
        match t.await.unwrap() {
            Ok(_) => successes += 1,
            Err(e) if matches!(e.class, ErrorClass::RateLimited { .. }) => rate_limited += 1,
            Err(e) => panic!("Unexpected error: {e:?}"),
        }
    }

    assert_eq!(successes, 1);
    assert_eq!(rate_limited, 9);
    assert_eq!(gateway.budget().total_consumed(), 75);
}
