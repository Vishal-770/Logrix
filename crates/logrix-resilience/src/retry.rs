use rand::Rng;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// Configurable exponential backoff retry policy with jitter.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f64,
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(10),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    pub fn new(max_attempts: u32, initial_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_attempts,
            initial_delay,
            max_delay,
            multiplier: 2.0,
            jitter: true,
        }
    }

    /// Compute delay for given attempt (1-indexed).
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let exp_delay = self.initial_delay.as_millis() as f64
            * self.multiplier.powi((attempt - 1).min(10) as i32);
        let capped_millis = exp_delay.min(self.max_delay.as_millis() as f64);

        if self.jitter {
            let mut rng = rand::thread_rng();
            let jittered = rng.gen_range(0.0..=capped_millis);
            Duration::from_millis(jittered as u64)
        } else {
            Duration::from_millis(capped_millis as u64)
        }
    }

    #[must_use]
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }
}

/// Token-based budget to prevent retry storms from overwhelming downstream services.
///
/// Implements a sliding refill rate allowing at most `max_retries_per_sec` retries.
#[derive(Debug, Clone)]
pub struct RetryBudget {
    capacity: u32,
    available_tokens: Arc<Mutex<f64>>,
    fill_rate_per_sec: f64,
    last_update: Arc<Mutex<Instant>>,
}

impl RetryBudget {
    pub fn new(capacity: u32, fill_rate_per_sec: f64) -> Self {
        Self {
            capacity,
            available_tokens: Arc::new(Mutex::new(capacity as f64)),
            fill_rate_per_sec,
            last_update: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Try to acquire a retry token. Returns true if budget allows, false if exhausted.
    pub async fn try_acquire(&self) -> bool {
        let mut tokens = self.available_tokens.lock().await;
        let mut last = self.last_update.lock().await;

        let now = Instant::now();
        let elapsed = now.duration_since(*last).as_secs_f64();
        *last = now;

        // Refill tokens
        *tokens = (*tokens + elapsed * self.fill_rate_per_sec).min(self.capacity as f64);

        if *tokens >= 1.0 {
            *tokens -= 1.0;
            true
        } else {
            false
        }
    }
}
