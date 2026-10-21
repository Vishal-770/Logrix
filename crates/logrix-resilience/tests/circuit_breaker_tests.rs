use logrix_resilience::{CircuitBreaker, CircuitConfig, CircuitState};
use std::time::Duration;

#[tokio::test]
async fn test_circuit_breaker_state_transitions() {
    let breaker = CircuitBreaker::new(2, 2, Duration::from_millis(100));
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert_eq!(breaker.current_state(), CircuitState::Closed);
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
async fn test_circuit_breaker_half_open_probe_release_and_custom_max() {
    // 2 successes needed, but only 1 concurrent probe allowed
    let config = CircuitConfig::builder()
        .failure_threshold(2)
        .success_threshold(2)
        .reset_timeout(Duration::from_millis(50))
        .half_open_max_probes(1)
        .build();

    let breaker = CircuitBreaker::with_config(config);
    breaker.trip();
    tokio::time::sleep(Duration::from_millis(70)).await;

    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // Acquire first probe permit
    let guard1 = breaker
        .try_acquire()
        .expect("first probe should be granted");
    // Second concurrent probe should be blocked because half_open_max_probes is 1
    assert!(breaker.try_acquire().is_none());
    assert!(!breaker.can_execute());

    // First probe succeeds and releases its slot
    guard1.record_success();
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // Now second probe should be allowed to run!
    let guard2 = breaker
        .try_acquire()
        .expect("second probe should be granted after first released");
    guard2.record_success();

    // Circuit is now closed after 2 successes
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());
}

#[tokio::test]
async fn test_circuit_breaker_guard_edge_and_error_paths() {
    let config = CircuitConfig::builder()
        .failure_threshold(2)
        .success_threshold(2)
        .reset_timeout(Duration::from_millis(50))
        .half_open_max_probes(1)
        .build();

    let breaker = CircuitBreaker::with_config(config);

    // 1. Explicit finish(false)
    let mut guard = breaker.try_acquire().unwrap();
    guard.finish(false);
    assert_eq!(breaker.failure_count(), 1);

    // 2. Repeated finish calls are idempotent
    guard.finish(true);
    guard.finish(false);
    assert_eq!(breaker.failure_count(), 1);

    // Trip circuit to Open
    breaker.trip();
    tokio::time::sleep(Duration::from_millis(70)).await;
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // 3. record_failure() while HalfOpen trips back to Open immediately
    let probe_guard = breaker.try_acquire().unwrap();
    probe_guard.record_failure();
    assert_eq!(breaker.state(), CircuitState::Open);

    // 4. Guard completing after breaker has transitioned to Open
    let closed_guard = {
        breaker.reset();
        breaker.try_acquire().unwrap()
    };
    breaker.trip(); // State changed to Open externally
    assert_eq!(breaker.state(), CircuitState::Open);
    closed_guard.record_success(); // Should be a no-op and not panic or corrupt state
    assert_eq!(breaker.state(), CircuitState::Open);
}

#[tokio::test]
async fn test_circuit_breaker_sliding_failure_window() {
    let config = CircuitConfig::builder()
        .failure_threshold(3)
        .success_threshold(1)
        .reset_timeout(Duration::from_millis(50))
        .failure_window(Duration::from_millis(80))
        .build();

    let breaker = CircuitBreaker::with_config(config);

    // Two failures at t = 0
    breaker.on_failure();
    breaker.on_failure();
    assert_eq!(breaker.failure_count(), 2);

    // Sleep past window (100ms > 80ms)
    tokio::time::sleep(Duration::from_millis(100)).await;

    // A single new failure should NOT trip the breaker, because the first two expired!
    breaker.on_failure();
    assert_eq!(breaker.failure_count(), 1);
    assert_eq!(breaker.state(), CircuitState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_manual_overrides() {
    let breaker = CircuitBreaker::new(2, 2, Duration::from_millis(100));

    breaker.trip();
    assert_eq!(breaker.state(), CircuitState::Open);
    assert!(!breaker.can_execute());

    breaker.reset();
    assert_eq!(breaker.state(), CircuitState::Closed);
    assert!(breaker.can_execute());
    assert_eq!(breaker.failure_count(), 0);
}

#[tokio::test]
async fn test_circuit_breaker_probe_epoch_safety_across_flips() {
    let breaker = CircuitBreaker::new(2, 2, Duration::from_millis(50));
    breaker.trip();

    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // Acquire probe 1 in epoch 1
    let probe1 = breaker.try_acquire().expect("should permit probe 1");

    // Manually trip or fail, changing epoch
    breaker.trip();
    assert_eq!(breaker.state(), CircuitState::Open);

    // Sleep to enter HalfOpen in epoch 2
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(breaker.state(), CircuitState::HalfOpen);

    // Acquire probe in new epoch
    let probe2 = breaker.try_acquire().expect("should permit probe 2");

    // Drop stale probe 1 from prior epoch - must NOT decrement active_probes of epoch 2
    drop(probe1);

    // Probe 2 finishes cleanly
    probe2.record_success();
}
