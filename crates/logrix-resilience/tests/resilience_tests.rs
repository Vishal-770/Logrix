use logrix_resilience::{
    CircuitBreaker, CircuitConfig, CircuitState, RateLimiter, RetryBudget, RetryPolicy,
};
use std::time::Duration;

#[tokio::test]
async fn test_retry_policy_delays() {
    let policy = RetryPolicy {
        max_attempts: 4,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_millis(1000),
        multiplier: 2.0,
        jitter: false,
    };

    assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(100));
    assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
    assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
    assert_eq!(policy.delay_for_attempt(4), Duration::from_millis(800));
    assert_eq!(policy.delay_for_attempt(5), Duration::from_millis(1000));
}

#[test]
fn test_retry_budget_exhaustion() {
    let budget = RetryBudget::new(3, 0.0);
    assert!(budget.try_acquire());
    assert!(budget.try_acquire());
    assert!(budget.try_acquire());
    assert!(!budget.try_acquire());
}

#[test]
fn test_rate_limiter_tokens() {
    let limiter = RateLimiter::new(5, 10.0);
    assert!(limiter.try_acquire(3.0));
    assert!(limiter.try_acquire(2.0));
    assert!(!limiter.try_acquire(1.0));
    assert!(!limiter.try_acquire(100.0));
}

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

    assert!(breaker.can_execute());
    assert!(!breaker.can_execute());

    breaker.on_success();
    breaker.on_success();
    assert_eq!(breaker.state(), CircuitState::Closed);
}

#[tokio::test]
async fn test_circuit_breaker_guard_raii_and_config() {
    let config = CircuitConfig::builder()
        .failure_threshold(2)
        .success_threshold(1)
        .reset_timeout(Duration::from_millis(50))
        .half_open_max_probes(1)
        .failure_window(Duration::from_millis(100))
        .build();

    let breaker = CircuitBreaker::with_config(config);
    assert_eq!(breaker.failure_count(), 0);

    // Permit acquisition and success
    let guard = breaker.try_acquire().expect("permit should be granted");
    guard.record_success();
    assert_eq!(breaker.failure_count(), 0);

    // Dropping a guard without explicit finish counts as a failure
    {
        let _guard = breaker.try_acquire().expect("permit should be granted");
    }
    assert_eq!(breaker.failure_count(), 1);

    // Expire failure window
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(breaker.can_execute());
    assert_eq!(breaker.failure_count(), 0);
}

#[tokio::test]
async fn test_rate_limiter_acquire_computed_wait() {
    let limiter = RateLimiter::new(2, 100.0);
    assert!(limiter.try_acquire(2.0));

    limiter.acquire(1.0).await;
    assert!(limiter.available_tokens() >= 0.0);
}

#[tokio::test]
async fn test_rate_limiter_builder_and_oversized_cost() {
    let limiter = RateLimiter::new(10, 100.0)
        .with_initial_tokens(0.0)
        .with_min_sleep_floor(Duration::from_micros(500));

    assert!(limiter.available_tokens() < 0.5);
    assert!(!limiter.try_acquire(1.0));

    limiter.acquire(20.0).await;
    assert!(limiter.available_tokens() >= 0.0);
}

#[tokio::test]
async fn test_retry_policy_builder_and_budget_inspector() {
    let policy = RetryPolicy::default()
        .with_multiplier(1.5)
        .with_jitter(false);

    assert_eq!(policy.multiplier, 1.5);
    assert!(!policy.jitter);
    assert!(policy.should_retry(4));
    assert!(!policy.should_retry(5));

    let budget = RetryBudget::new(5, 0.0);
    assert_eq!(budget.available_budget(), 5.0);
    assert!(budget.try_acquire());
    assert_eq!(budget.available_budget(), 4.0);
}
