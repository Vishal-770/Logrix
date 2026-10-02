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

#[tokio::test]
async fn test_retry_budget_exhaustion() {
    let budget = RetryBudget::new(3, 0.0); // 3 tokens, 0 refill rate
    assert!(budget.try_acquire().await);
    assert!(budget.try_acquire().await);
    assert!(budget.try_acquire().await);
    // Exhausted
    assert!(!budget.try_acquire().await);
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
