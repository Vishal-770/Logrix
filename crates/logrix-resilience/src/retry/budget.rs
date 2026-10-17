use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug)]
struct BudgetState {
    tokens: f64,
    last_update: Instant,
}

impl BudgetState {
    fn refill(&mut self, capacity: f64, fill_rate_per_sec: f64) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();
        self.last_update = now;
        self.tokens = (self.tokens + elapsed * fill_rate_per_sec).min(capacity);
    }
}

/// Token-based budget to prevent retry storms from overwhelming downstream services.
///
/// Implements a sliding refill rate allowing at most `fill_rate_per_sec` retries.
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
    pub fn try_acquire(&self) -> bool {
        let mut state = self.state.lock();
        state.refill(self.capacity as f64, self.fill_rate_per_sec);

        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Count of currently available retry tokens.
    pub fn available_budget(&self) -> f64 {
        let mut state = self.state.lock();
        state.refill(self.capacity as f64, self.fill_rate_per_sec);
        state.tokens
    }

    /// Return total configured token capacity.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Return refill rate in tokens per second.
    pub fn fill_rate_per_sec(&self) -> f64 {
        self.fill_rate_per_sec
    }
}
