use logrix_core::leader::{KubernetesLeaseElector, LocalLeaderElector};
use logrix_core::ports::LeaderElectionPort;

#[tokio::test]
async fn test_local_leader_elector_lifecycle() {
    let elector = LocalLeaderElector::new();
    assert!(elector.is_leader().await);

    // Step down
    elector.step_down().await.expect("Step down succeeded");
    assert!(!elector.is_leader().await);

    // Re-acquire
    let acquired = elector
        .try_acquire_or_renew()
        .await
        .expect("Acquire succeeded");
    assert!(acquired);
    assert!(elector.is_leader().await);
}

#[tokio::test]
async fn test_kubernetes_lease_elector_unreachable_fallback() {
    // When pointing to unreachable mock URL, elector should fail gracefully without panic
    let elector = KubernetesLeaseElector::new(
        "logrix-ingester-lease",
        "default",
        "pod-1",
        Some("http://127.0.0.1:59999".into()),
        Some("mock-token".into()),
    );

    assert!(!elector.is_leader().await);
    let acquired = elector
        .try_acquire_or_renew()
        .await
        .expect("Call succeeds with false");
    assert!(!acquired);
    assert!(!elector.is_leader().await);
}
