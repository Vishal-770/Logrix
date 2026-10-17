use logrix_core::leader::LocalLeaderElector;
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
async fn test_local_leader_elector_simulates_failover() {
    let elector = LocalLeaderElector::with_initial_state(false);
    assert!(!elector.is_leader().await);

    elector.set_leader(true);
    assert!(elector.try_acquire_or_renew().await.expect("state read"));

    elector.simulate_leadership_loss();
    assert!(!elector.is_leader().await);
    assert!(elector.try_acquire_or_renew().await.expect("re-acquire"));
}
