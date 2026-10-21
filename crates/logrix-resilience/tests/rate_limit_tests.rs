use logrix_resilience::RateLimiter;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_rate_limiter_tokens_and_try_acquire() {
    let limiter = RateLimiter::new(5, 10.0);
    assert_eq!(limiter.capacity(), 5.0);
    assert_eq!(limiter.refill_rate(), 10.0);
    assert!(limiter.try_acquire(3.0));
    assert!(limiter.try_acquire(2.0));
    assert!(!limiter.try_acquire(1.0));
    assert!(!limiter.try_acquire(100.0));
}

#[tokio::test]
async fn test_rate_limiter_acquire_computed_wait() {
    let limiter = RateLimiter::new(2, 100.0);
    assert!(limiter.try_acquire(2.0));

    // Computes sleep duration for 1 token (~10ms) without spinning
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

    // Cost 20.0 > capacity 10.0 is clamped safely without deadlock
    limiter.acquire(20.0).await;
    assert!(limiter.available_tokens() >= 0.0);
}

#[tokio::test]
async fn test_rate_limiter_concurrent_acquisition() {
    let limiter = Arc::new(RateLimiter::new(20, 50.0));
    let mut tasks = Vec::new();

    for _ in 0..5 {
        let l = Arc::clone(&limiter);
        tasks.push(tokio::spawn(async move {
            for _ in 0..4 {
                l.acquire(1.0).await;
            }
        }));
    }

    for task in tasks {
        task.await.unwrap();
    }
}

#[tokio::test]
async fn test_rate_limiter_edge_cases_negative_and_nan() {
    let limiter = RateLimiter::new(10, 10.0);

    // Negative and NaN cost must be rejected and not inject tokens
    assert!(!limiter.try_acquire(-5.0));
    assert!(!limiter.try_acquire(0.0));
    assert!(!limiter.try_acquire(f64::NAN));
    assert!(!limiter.try_acquire(f64::INFINITY));
    assert!(!limiter.try_acquire(f64::NEG_INFINITY));

    // Tokens must remain at 10.0, not increased
    assert!(limiter.available_tokens() <= 10.0);

    // acquire with negative or NaN should be safe no-op
    limiter.acquire(-10.0).await;
    limiter.acquire(f64::NAN).await;

    // zero or negative refill rate in constructor must not panic
    let zero_limiter = RateLimiter::new(5, 0.0);
    assert_eq!(zero_limiter.refill_rate(), 1.0); // defaulted safely

    let nan_limiter = RateLimiter::new(5, f64::NAN);
    assert_eq!(nan_limiter.refill_rate(), 1.0);
}
