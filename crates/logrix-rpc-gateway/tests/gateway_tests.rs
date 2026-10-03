use logrix_core::domain::ChainId;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use logrix_rpc_gateway::{
    CuBudgetTracker, ManagedProvider, ProviderPool, RpcGateway, SingleFlight,
};
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

#[tokio::test]
async fn test_gateway_fault_tolerance_automatic_failover_and_recovery() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let pool = ProviderPool::new(chain_id);

    let p1 = ManagedProvider::new("flakey-primary", "http://localhost:8545", 1, chain_id);
    let p2 = ManagedProvider::new("reliable-fallback", "http://localhost:8546", 2, chain_id);

    pool.add_provider(p1.clone()).await;
    pool.add_provider(p2.clone()).await;

    let gateway = RpcGateway::new(chain_id, pool, None, None);

    // Call 1: Primary fails with HTTP 429 RateLimit; Gateway transparently falls back to p2
    let res: LogrixResult<String> = gateway
        .execute_with_fallback("eth_getLogs", |prov| async move {
            if prov.name() == "flakey-primary" {
                Err(LogrixError::rate_limited(
                    ErrorSource::ChainRpc,
                    "Simulated 429 Too Many Requests",
                    None,
                ))
            } else {
                Ok("fallback-logs-success".to_string())
            }
        })
        .await;

    assert!(res.is_ok());
    assert_eq!(res.unwrap(), "fallback-logs-success");

    // Fail primary two more times to trip its circuit breaker (threshold: 3)
    p1.record_failure().await;
    p1.record_failure().await;
    assert!(!p1.is_healthy().await);

    // Subsequent calls bypass p1 entirely without waiting and route directly to p2
    let res2: LogrixResult<String> = gateway
        .execute_with_fallback("eth_blockNumber", |prov| async move {
            assert_eq!(prov.name(), "reliable-fallback");
            Ok("1000".to_string())
        })
        .await;

    assert!(res2.is_ok());
    assert_eq!(res2.unwrap(), "1000");

    // Recovery test: once p1 recovers and records successes, cooldown resets to initial 10s
    p1.record_success(Duration::from_millis(25)).await;
    let cooldown = p1.cooldown_duration();
    // Due to jitter [base/2, base], 10s base yields between 5s and 10s
    assert!(cooldown >= Duration::from_millis(5000));
    assert!(cooldown <= Duration::from_millis(10000));
}

#[tokio::test]
async fn test_gateway_all_providers_failing_error_propagation() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let pool = ProviderPool::new(chain_id);

    let p1 = ManagedProvider::new("bad-node-1", "http://localhost:8545", 1, chain_id);
    let p2 = ManagedProvider::new("bad-node-2", "http://localhost:8546", 2, chain_id);

    pool.add_provider(p1).await;
    pool.add_provider(p2).await;

    let gateway = RpcGateway::new(chain_id, pool, None, None);

    // Both nodes fail
    let res: LogrixResult<u64> = gateway
        .execute_with_fallback("eth_blockNumber", |_| async move {
            Err(LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::ChainRpc,
                "Simulated connection reset",
            ))
        })
        .await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(err.class, ErrorClass::Transient);
    assert_eq!(err.source, ErrorSource::ChainRpc);
}

#[tokio::test]
async fn test_gateway_budget_exhaustion_blocks_further_network_calls() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let pool = ProviderPool::new(chain_id);
    let p1 = ManagedProvider::new("node", "http://localhost:8545", 1, chain_id);
    pool.add_provider(p1).await;

    // Hard ceiling: 70 CU (less than 1 eth_getLogs = 75 CU)
    let gateway = RpcGateway::new(chain_id, pool, Some(70), None);
    assert!(!gateway.budget().is_exhausted());

    // Consume 75 CU
    gateway.budget().record_method("eth_getLogs");
    assert!(gateway.budget().is_exhausted());

    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

    // Any new call through gateway must be instantly rejected without hitting provider
    let res: LogrixResult<String> = gateway
        .execute_with_fallback("eth_getLogs", |_| {
            let c_clone = c.clone();
            async move {
                c_clone.fetch_add(1, Ordering::SeqCst);
                Ok("data".to_string())
            }
        })
        .await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(matches!(err.class, ErrorClass::RateLimited { .. }));
    // Counter remains 0: the network closure was never invoked!
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_provider_latency_moving_average_and_jitter() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let p = ManagedProvider::new("node", "http://localhost:8545", 1, chain_id);

    // Initial seed latency is 50ms
    assert_eq!(p.avg_latency_ms(), 50);

    // Record a 100ms response (EMA: (50*4 + 100)/5 = 300/5 = 60ms)
    p.record_success(Duration::from_millis(100)).await;
    assert_eq!(p.avg_latency_ms(), 60);

    // Jitter test: ensure cooldown with jitter stays within [base/2, base] bounds
    for _ in 0..20 {
        let dur = p.cooldown_duration();
        assert!(dur >= Duration::from_millis(5000));
        assert!(dur <= Duration::from_millis(10000));
    }
}
