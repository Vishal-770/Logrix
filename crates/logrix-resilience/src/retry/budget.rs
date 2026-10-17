use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

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
