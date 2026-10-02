use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Token-bucket rate limiter for managing RPC and endpoint throughput limits.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    capacity: f64,
    refill_rate_per_sec: f64,
    tokens: Arc<Mutex<f64>>,
    last_refill: Arc<Mutex<Instant>>,
}

impl RateLimiter {
    pub fn new(capacity: u32, refill_rate_per_sec: f64) -> Self {
        Self {
            capacity: capacity as f64,
            refill_rate_per_sec,
            tokens: Arc::new(Mutex::new(capacity as f64)),
            last_refill: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Try to acquire `cost` tokens immediately.
    pub async fn try_acquire(&self, cost: f64) -> bool {
        let mut tokens = self.tokens.lock().await;
        let mut last = self.last_refill.lock().await;

        let now = Instant::now();
        let elapsed = now.duration_since(*last).as_secs_f64();
        *last = now;

        *tokens = (*tokens + elapsed * self.refill_rate_per_sec).min(self.capacity);

        if *tokens >= cost {
            *tokens -= cost;
            true
        } else {
            false
        }
    }

    /// Acquire tokens, sleeping asynchronously if rate limit is reached.
    pub async fn acquire(&self, cost: f64) {
        loop {
            if self.try_acquire(cost).await {
                return;
            }
            // Sleep short duration before retrying acquisition
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
}
