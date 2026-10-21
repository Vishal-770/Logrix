use logrix_resilience::{RetryBudget, RetryPolicy};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_retry_policy_delays() {
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
fn test_retry_policy_with_jitter_stays_within_bounds() {
    let policy = RetryPolicy::default()
        .with_multiplier(2.0)
        .with_jitter(true);

    for attempt in 1..=5 {
        let delay = policy.delay_for_attempt(attempt);
        let max_possible = policy.max_delay;
        assert!(delay <= max_possible);
    }
}

#[test]
fn test_retry_policy_should_retry_bounds() {
    let policy = RetryPolicy::new(3, Duration::from_millis(10), Duration::from_millis(100));
    // For max_attempts = 3: attempts 0, 1, 2 return true (< 3), attempt 3 and above return false
    assert!(policy.should_retry(0));
    assert!(policy.should_retry(1));
    assert!(policy.should_retry(2));
    assert!(!policy.should_retry(3));
    assert!(!policy.should_retry(4));
}

#[test]
fn test_retry_budget_exhaustion_and_refill() {
    let budget = RetryBudget::new(2, 10.0); // 10 tokens/sec
    assert!(budget.try_acquire());
    assert!(budget.try_acquire());
    assert!(!budget.try_acquire());

    // Sleep for 150ms -> accumulates ~1.5 tokens
    std::thread::sleep(Duration::from_millis(150));
    assert!(budget.try_acquire());
    assert_eq!(budget.capacity(), 2);
    assert_eq!(budget.fill_rate_per_sec(), 10.0);
}

#[test]
fn test_retry_budget_concurrent_contention() {
    let budget = Arc::new(RetryBudget::new(50, 0.0));
    let mut handles = Vec::new();

    for _ in 0..10 {
        let b = Arc::clone(&budget);
        handles.push(thread::spawn(move || {
            let mut acquired = 0;
            for _ in 0..10 {
                if b.try_acquire() {
                    acquired += 1;
                }
            }
            acquired
        }));
    }

    let total_acquired: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    assert_eq!(total_acquired, 50);
    assert_eq!(budget.available_budget(), 0.0);
}

#[tokio::test]
async fn test_retry_policy_multiplier_and_jitter_floor() {
    let policy = RetryPolicy::new(3, Duration::from_millis(100), Duration::from_secs(1))
        .with_multiplier(0.0) // Invalid multiplier must fall back to 1.0
        .with_jitter(true);

    assert_eq!(policy.multiplier, 1.0);

    // With equal jitter, delay for attempt 1 must have a guaranteed floor (>= 50ms)
    for _ in 0..10 {
        let delay = policy.delay_for_attempt(1);
        assert!(delay >= Duration::from_millis(50));
        assert!(delay <= Duration::from_millis(100));
    }
}

#[tokio::test]
async fn test_retry_budget_acquire_and_time_until_available() {
    let budget = RetryBudget::new(1, 50.0);
    assert_eq!(budget.time_until_available(), Duration::ZERO);
    assert!(budget.try_acquire());

    let wait = budget.time_until_available();
    assert!(wait > Duration::ZERO);

    // acquire should wait and succeed
    budget.acquire().await;
    assert_eq!(budget.capacity(), 1);
}
