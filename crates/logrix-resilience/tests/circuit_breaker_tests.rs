use logrix_resilience::{CircuitBreaker, CircuitConfig, CircuitState};
use std::time::Duration;

#[tokio::test]
async fn test_circuit_breaker_state_transitions() {
    let breaker = CircuitBreaker::new(2, 2, Duration::from_millis(100));
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());

    breaker.on_failure();
    assert_eq!(breaker.state(), CircuitState::Closed);

    breaker.on_failure();
    assert_eq!(breaker.state(), CircuitState::Open);
    assert!(!breaker.can_execute());

    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    breaker.on_success();
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    breaker.on_success();
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());
}

#[tokio::test]
async fn test_circuit_breaker_manual_overrides_and_probe_limiting() {
    let breaker = CircuitBreaker::new(2, 2, Duration::from_millis(100));

    breaker.trip();
    assert_eq!(breaker.state(), CircuitState::Open);
    assert!(!breaker.can_execute());

    breaker.reset();
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());
    assert_eq!(breaker.failure_count(), 0);

    breaker.trip();
    tokio::time::sleep(Duration::from_millis(110)).await;

    assert!(breaker.can_execute());
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // Second probe permitted
    assert!(breaker.can_execute());
    // Third probe rejected (success_threshold / probe limit is 2)
    assert!(!breaker.can_execute());

    breaker.on_success();
    breaker.on_success();
    assert_eq!(breaker.state(), CircuitState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_guard_raii_and_window_expiry() {
    let config = CircuitConfig::builder()
        .failure_threshold(2)
        .success_threshold(1)
        .reset_timeout(Duration::from_millis(50))
        .half_open_max_probes(1)
        .failure_window(Duration::from_millis(100))
        .build();

    let breaker = CircuitBreaker::with_config(config);
    assert_eq!(breaker.failure_count(), 0);

    let guard = breaker.try_acquire().expect("permit should be granted");
    guard.record_success();
    assert_eq!(breaker.failure_count(), 0);

    // Dropping a guard without explicit finish auto-records failure
    {
        let _guard = breaker.try_acquire().expect("permit should be granted");
    }
    assert_eq!(breaker.failure_count(), 1);

    // After failure_window elapses, stale failures expire back to 0
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(breaker.can_execute());
    assert_eq!(breaker.failure_count(), 0);
}
