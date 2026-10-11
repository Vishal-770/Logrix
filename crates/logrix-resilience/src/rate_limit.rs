use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug)]
struct BucketState {
    tokens: f64,
    last_refill: Instant,
}

/// Token-bucket rate limiter for managing RPC and endpoint throughput limits.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    capacity: f64,
    refill_rate_per_sec: f64,
    state: Arc<Mutex<BucketState>>,
}

impl RateLimiter {
    pub fn new(capacity: u32, refill_rate_per_sec: f64) -> Self {
        let cap = capacity as f64;
        Self {
            capacity: cap,
            refill_rate_per_sec,
            state: Arc::new(Mutex::new(BucketState {
                tokens: cap,
                last_refill: Instant::now(),
            })),
        }
    }

    /// Try to acquire `cost` tokens immediately.
    pub async fn try_acquire(&self, cost: f64) -> bool {
        let mut state = self.state.lock().await;
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        state.last_refill = now;
        state.tokens = (state.tokens + elapsed * self.refill_rate_per_sec).min(self.capacity);

        if state.tokens >= cost {
            state.tokens -= cost;
            true
        } else {
            false
        }
    }

    /// Acquire tokens, sleeping asynchronously if rate limit is reached.
    pub async fn acquire(&self, cost: f64) {
        loop {
            let wait_duration = {
                let mut state = self.state.lock().await;
                let now = Instant::now();
                let elapsed = now.duration_since(state.last_refill).as_secs_f64();
                state.last_refill = now;
                state.tokens =
                    (state.tokens + elapsed * self.refill_rate_per_sec).min(self.capacity);

                if state.tokens >= cost {
                    state.tokens -= cost;
                    None
                } else if self.refill_rate_per_sec > 0.0 {
                    let missing = cost - state.tokens;
                    let secs = missing / self.refill_rate_per_sec;
                    Some(Duration::from_secs_f64(secs.max(0.001)))
                } else {
                    Some(Duration::from_millis(10))
                }
            };

            match wait_duration {
                None => return,
                Some(delay) => tokio::time::sleep(delay).await,
            }
        }
    }

    /// Approximate count of currently available tokens.
    pub async fn available_tokens(&self) -> f64 {
        let mut state = self.state.lock().await;
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        state.last_refill = now;
        state.tokens = (state.tokens + elapsed * self.refill_rate_per_sec).min(self.capacity);
        state.tokens
    }
}
