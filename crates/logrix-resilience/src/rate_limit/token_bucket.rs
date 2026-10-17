use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct BucketState {
    tokens: f64,
    last_refill: Instant,
}

impl BucketState {
    fn refill(&mut self, capacity: f64, refill_rate_per_sec: f64) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.last_refill = now;
        self.tokens = (self.tokens + elapsed * refill_rate_per_sec).min(capacity);
    }
}

/// Token bucket rate limiter supporting fractional refill bursts and single-mutex state encapsulation.
#[derive(Debug, Clone)]
pub struct TokenBucketRateLimiter {
    capacity: f64,
    refill_rate_per_sec: f64,
    min_sleep_floor: Duration,
    state: Arc<Mutex<BucketState>>,
}

impl TokenBucketRateLimiter {
    pub fn new(capacity: u32, refill_rate_per_sec: f64) -> Self {
        Self {
            capacity: capacity as f64,
            refill_rate_per_sec,
            min_sleep_floor: Duration::from_millis(1),
            state: Arc::new(Mutex::new(BucketState {
                tokens: capacity as f64,
                last_refill: Instant::now(),
            })),
        }
    }

    /// Set a custom initial token balance (e.g. 0.0 for cold-start warmup).
    #[must_use]
    pub fn with_initial_tokens(self, tokens: f64) -> Self {
        let mut state = self.state.lock();
        state.tokens = tokens.clamp(0.0, self.capacity);
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
        state.refill(self.capacity, self.refill_rate_per_sec);

        if state.tokens >= cost {
            state.tokens -= cost;
            true
        } else {
            false
        }
    }

    /// Asynchronously acquire `cost` tokens, computing the exact wait without busy-spinning.
    pub async fn acquire(&self, cost: f64) {
        let effective_cost = if cost > self.capacity {
            tracing::warn!(
                requested = cost,
                capacity = self.capacity,
                "Requested rate limiter cost exceeds capacity; clamping to capacity to prevent deadlock"
            );
            self.capacity
        } else {
            cost
        };

        loop {
            let wait_duration = {
                let mut state = self.state.lock();
                state.refill(self.capacity, self.refill_rate_per_sec);

                if state.tokens >= effective_cost {
                    state.tokens -= effective_cost;
                    return;
                }

                let missing = effective_cost - state.tokens;
                let needed_secs = missing / self.refill_rate_per_sec;
                Duration::from_secs_f64(needed_secs).max(self.min_sleep_floor)
            };

            tokio::time::sleep(wait_duration).await;
        }
    }

    pub fn available_tokens(&self) -> f64 {
        let mut state = self.state.lock();
        state.refill(self.capacity, self.refill_rate_per_sec);
        state.tokens
    }

    pub fn capacity(&self) -> f64 {
        self.capacity
    }

    pub fn refill_rate(&self) -> f64 {
        self.refill_rate_per_sec
    }
}

pub type RateLimiter = TokenBucketRateLimiter;
