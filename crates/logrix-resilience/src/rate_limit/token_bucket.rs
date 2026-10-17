use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::warn;

#[derive(Debug)]
struct BucketState {
    tokens: f64,
    last_refill: Instant,
}

/// Token-bucket rate limiter for managing RPC and endpoint throughput limits.
#[derive(Debug, Clone)]
pub struct TokenBucketRateLimiter {
    capacity: f64,
    refill_rate_per_sec: f64,
    min_sleep_floor: Duration,
    state: Arc<Mutex<BucketState>>,
}

/// Backward-compatible alias for the standard token bucket rate limiter.
pub type RateLimiter = TokenBucketRateLimiter;

impl TokenBucketRateLimiter {
    pub fn new(capacity: u32, refill_rate_per_sec: f64) -> Self {
        let cap = capacity as f64;
        Self {
            capacity: cap,
            refill_rate_per_sec,
            min_sleep_floor: Duration::from_millis(1),
            state: Arc::new(Mutex::new(BucketState {
                tokens: cap,
                last_refill: Instant::now(),
            })),
        }
    }

    /// Set a custom starting token balance (e.g. 0.0 for cold-start warmup).
    #[must_use]
    pub fn with_initial_tokens(self, tokens: f64) -> Self {
        let clamped = tokens.clamp(0.0, self.capacity);
        let mut state = self.state.lock();
        state.tokens = clamped;
        state.last_refill = Instant::now();
        drop(state);
        self
    }

    /// Set a custom minimum sleep duration floor (default: 1ms).
    #[must_use]
    pub fn with_min_sleep_floor(mut self, floor: Duration) -> Self {
        self.min_sleep_floor = floor;
        self
    }

    /// Try to acquire `cost` tokens immediately (synchronous, non-blocking).
    pub fn try_acquire(&self, cost: f64) -> bool {
        if cost > self.capacity {
            return false;
        }

        let mut state = self.state.lock();
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
    ///
    /// If requested cost exceeds total capacity, it is safely clamped to capacity
    /// to prevent infinite loop deadlocks.
    pub async fn acquire(&self, cost: f64) {
        let effective_cost = if cost > self.capacity {
            warn!(
                cost,
                capacity = self.capacity,
                "Requested rate limit cost exceeds capacity; clamping to capacity"
            );
            self.capacity
        } else {
            cost
        };

        loop {
            let wait_duration = {
                let mut state = self.state.lock();
                let now = Instant::now();
                let elapsed = now.duration_since(state.last_refill).as_secs_f64();
                state.last_refill = now;
                state.tokens =
                    (state.tokens + elapsed * self.refill_rate_per_sec).min(self.capacity);

                if state.tokens >= effective_cost {
                    state.tokens -= effective_cost;
                    None
                } else if self.refill_rate_per_sec > 0.0 {
                    let missing = effective_cost - state.tokens;
                    let secs = missing / self.refill_rate_per_sec;
                    Some(Duration::from_secs_f64(secs).max(self.min_sleep_floor))
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

    /// Approximate count of currently available tokens (synchronous inspector).
    pub fn available_tokens(&self) -> f64 {
        let mut state = self.state.lock();
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        state.last_refill = now;
        state.tokens = (state.tokens + elapsed * self.refill_rate_per_sec).min(self.capacity);
        state.tokens
    }

    /// Configured maximum capacity of the rate limiter.
    #[must_use]
    pub fn capacity(&self) -> f64 {
        self.capacity
    }

    /// Configured refill rate per second.
    #[must_use]
    pub fn refill_rate(&self) -> f64 {
        self.refill_rate_per_sec
    }
}
