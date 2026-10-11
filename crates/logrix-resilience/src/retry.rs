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

    #[must_use]
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier;
        self
    }

    #[must_use]
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }
}

#[derive(Debug)]
struct BudgetState {
    tokens: f64,
    last_update: Instant,
}

/// Token-based budget to prevent retry storms from overwhelming downstream services.
///
/// Implements a sliding refill rate allowing at most `max_retries_per_sec` retries.
#[derive(Debug, Clone)]
pub struct RetryBudget {
    capacity: u32,
    fill_rate_per_sec: f64,
    state: Arc<Mutex<BudgetState>>,
}

impl RetryBudget {
    pub fn new(capacity: u32, fill_rate_per_sec: f64) -> Self {
        Self {
            capacity,
            fill_rate_per_sec,
            state: Arc::new(Mutex::new(BudgetState {
                tokens: capacity as f64,
                last_update: Instant::now(),
            })),
        }
    }

    /// Try to acquire a retry token. Returns true if budget allows, false if exhausted.
    pub async fn try_acquire(&self) -> bool {
        let mut state = self.state.lock().await;
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_update).as_secs_f64();
        state.last_update = now;

        state.tokens = (state.tokens + elapsed * self.fill_rate_per_sec).min(self.capacity as f64);

        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Count of currently available retry tokens.
    pub async fn available_budget(&self) -> f64 {
        let mut state = self.state.lock().await;
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_update).as_secs_f64();
        state.last_update = now;
        state.tokens = (state.tokens + elapsed * self.fill_rate_per_sec).min(self.capacity as f64);
        state.tokens
    }
}
