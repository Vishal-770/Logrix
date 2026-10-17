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
