use logrix_core::domain::ChainId;
use logrix_rpc_gateway::{CuBudgetTracker, ManagedProvider, ProviderPool, SingleFlight};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_provider_pool_routing_and_priority() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let pool = ProviderPool::new(chain_id);

    // Add primary (priority 1) and secondary (priority 2)
    let p1 = ManagedProvider::new("primary", "http://localhost:8545", 1, chain_id);
    let p2 = ManagedProvider::new("secondary", "http://localhost:8546", 2, chain_id);

    pool.add_provider(p1.clone()).await;
    pool.add_provider(p2.clone()).await;

    // Should select primary initially
    let selected = pool.select_provider().await.unwrap();
    assert_eq!(selected.name(), "primary");

    // Trip primary circuit breaker with 3 consecutive failures
    p1.record_failure().await;
    p1.record_failure().await;
    p1.record_failure().await;

    assert!(!p1.is_healthy().await);

    // Pool should now transparently fail over to secondary
    let fallback = pool.select_provider().await.unwrap();
    assert_eq!(fallback.name(), "secondary");
}

#[tokio::test]
async fn test_cu_budget_enforcement() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let pool = ProviderPool::new(chain_id);
    let provider = ManagedProvider::new("local", "http://localhost:8545", 1, chain_id);
    pool.add_provider(provider).await;

    // Cap budget at 100 CU
    let tracker = CuBudgetTracker::new(Some(100));
    assert_eq!(tracker.total_consumed(), 0);

    // 1 eth_getLogs = 75 CU
    tracker.record_method("eth_getLogs");
    assert_eq!(tracker.total_consumed(), 75);
    assert!(!tracker.is_exhausted());

    // Another eth_getLogs = +75 CU (total 150 CU > 100 CU budget)
    tracker.record_method("eth_getLogs");
    assert_eq!(tracker.total_consumed(), 150);
    assert!(tracker.is_exhausted());
}

#[tokio::test]
async fn test_singleflight_concurrent_dedup() {
    let sf = SingleFlight::new();
    let counter = Arc::new(AtomicUsize::new(0));

    let mut tasks = Vec::new();
    for _ in 0..8 {
        let sf_c = sf.clone();
        let counter_c = counter.clone();
        tasks.push(tokio::spawn(async move {
            sf_c.execute("eth_getBlockByNumber:100", || async move {
                tokio::time::sleep(Duration::from_millis(30)).await;
                counter_c.fetch_add(1, Ordering::SeqCst);
                Ok("0xabcdef".to_string())
            })
            .await
        }));
    }

    for task in tasks {
        let res = task.await.unwrap().unwrap();
        assert_eq!(res, "0xabcdef");
    }

    // Only 1 execution was performed for all 8 concurrent callers
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}
