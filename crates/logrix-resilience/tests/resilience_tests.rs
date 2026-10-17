use logrix_resilience::{CircuitBreaker, CircuitState, RateLimiter, RetryBudget, RetryPolicy};
use std::time::Duration;

#[tokio::test]
async fn test_retry_policy_delays() {
    let policy = RetryPolicy {
        max_attempts: 4,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_millis(1000),
        multiplier: 2.0,
        jitter: false, // disable jitter for deterministic math
    };

    assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(100));
    assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(200));
    assert_eq!(policy.delay_for_attempt(3), Duration::from_millis(400));
    assert_eq!(policy.delay_for_attempt(4), Duration::from_millis(800));
    // Caps at max_delay
    assert_eq!(policy.delay_for_attempt(5), Duration::from_millis(1000));
}

#[test]
fn test_retry_budget_exhaustion() {
    let budget = RetryBudget::new(3, 0.0); // 3 tokens, 0 refill rate
    assert!(budget.try_acquire());
    assert!(budget.try_acquire());
    assert!(budget.try_acquire());
    // Exhausted
    assert!(!budget.try_acquire());
}

#[tokio::test]
async fn test_rate_limiter_tokens() {
    let limiter = RateLimiter::new(5, 10.0);
    assert!(limiter.try_acquire(3.0).await);
    assert!(limiter.try_acquire(2.0).await);
    // Not enough tokens left
    assert!(!limiter.try_acquire(1.0).await);
}

#[tokio::test]
async fn test_circuit_breaker_state_transitions() {
    let breaker = CircuitBreaker::new(2, 2, Duration::from_millis(100));
    assert_eq!(breaker.state().await, CircuitState::Closed);
    assert!(breaker.can_execute().await);

    // First failure
    breaker.on_failure().await;
    assert_eq!(breaker.state().await, CircuitState::Closed);

    // Second failure -> trips Open
    breaker.on_failure().await;
    assert_eq!(breaker.state().await, CircuitState::Open);
    assert!(!breaker.can_execute().await);

    // Sleep past reset timeout -> HalfOpen
    tokio::time::sleep(Duration::from_millis(120)).await;
    assert!(breaker.can_execute().await);
    assert_eq!(breaker.state().await, CircuitState::HalfOpen);

    // First success in HalfOpen
    breaker.on_success().await;
    assert_eq!(breaker.state().await, CircuitState::HalfOpen);

    // Second success in HalfOpen -> transitions back to Closed
    breaker.on_success().await;
    assert_eq!(breaker.state().await, CircuitState::Closed);
    assert!(breaker.can_execute().await);
}

#[tokio::test]
async fn test_circuit_breaker_manual_overrides_and_probe_limiting() {
    let breaker = CircuitBreaker::new(2, 2, Duration::from_millis(100));

    // Manual trip
    breaker.trip().await;
    assert_eq!(breaker.state().await, CircuitState::Open);
    assert!(!breaker.can_execute().await);

    // Manual reset
    breaker.reset().await;
    assert_eq!(breaker.state().await, CircuitState::Closed);
    assert!(breaker.can_execute().await);
    assert_eq!(breaker.failure_count().await, 0);

    // Trip again and test HalfOpen probe limiting
    breaker.trip().await;
    tokio::time::sleep(Duration::from_millis(110)).await;

    // First request discovers timeout and becomes probe 1
    assert!(breaker.can_execute().await);
    assert_eq!(breaker.state().await, CircuitState::HalfOpen);

    // Second request is allowed as probe 2 (success_threshold is 2)
    assert!(breaker.can_execute().await);

    // Third request exceeds success_threshold probes -> rejected to prevent thundering herd!
    assert!(!breaker.can_execute().await);

    // Successful probes restore the circuit
    breaker.on_success().await;
    breaker.on_success().await;
    assert_eq!(breaker.state().await, CircuitState::Closed);
}

#[tokio::test]
async fn test_rate_limiter_acquire_computed_wait() {
    let limiter = RateLimiter::new(2, 100.0); // 100 tokens/sec
    assert!(limiter.try_acquire(2.0).await);

    // Acquire will calculate needed time (approx 10ms for 1 token) and succeed without spinning
    limiter.acquire(1.0).await;
    assert!(limiter.available_tokens().await >= 0.0);
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
