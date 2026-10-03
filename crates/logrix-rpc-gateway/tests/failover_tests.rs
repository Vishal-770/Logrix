use logrix_core::domain::ChainId;
use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};
use logrix_rpc_gateway::{ManagedProvider, ProviderPool, RpcGateway};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_gateway_fault_tolerance_automatic_failover_and_recovery() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let pool = ProviderPool::new(chain_id);

    let p1 = ManagedProvider::new("flakey-primary", "http://localhost:8545", 1, chain_id);
    let p2 = ManagedProvider::new("reliable-fallback", "http://localhost:8546", 2, chain_id);

    pool.add_provider(p1.clone()).await;
    pool.add_provider(p2.clone()).await;

    let gateway = RpcGateway::new(chain_id, pool, None, None);

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

    p1.record_failure().await;
    p1.record_failure().await;
    assert!(!p1.is_healthy().await);

    let res2: LogrixResult<String> = gateway
        .execute_with_fallback("eth_blockNumber", |prov| async move {
            assert_eq!(prov.name(), "reliable-fallback");
            Ok("1000".to_string())
        })
        .await;

    assert!(res2.is_ok());
    assert_eq!(res2.unwrap(), "1000");

    p1.record_success(Duration::from_millis(25)).await;
    let cooldown = p1.cooldown_duration();
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

    let gateway = RpcGateway::new(chain_id, pool, Some(70), None);
    assert!(!gateway.budget().is_exhausted());

    gateway.budget().record_method("eth_getLogs");
    assert!(gateway.budget().is_exhausted());

    let counter = Arc::new(AtomicUsize::new(0));
    let c = counter.clone();

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
    assert_eq!(counter.load(Ordering::SeqCst), 0);
}
