use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Three-state circuit breaker preventing calls to failing external dependencies.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: Arc<Mutex<CircuitState>>,
    failure_count: Arc<Mutex<u32>>,
    success_count: Arc<Mutex<u32>>,
    failure_threshold: u32,
    success_threshold: u32,
    reset_timeout: Duration,
    last_state_change: Arc<Mutex<Instant>>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, success_threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            state: Arc::new(Mutex::new(CircuitState::Closed)),
            failure_count: Arc::new(Mutex::new(0)),
            success_count: Arc::new(Mutex::new(0)),
            failure_threshold,
            success_threshold,
            reset_timeout,
            last_state_change: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Check if call is permitted under current circuit state.
    pub async fn can_execute(&self) -> bool {
        let mut state = self.state.lock().await;
        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let last = self.last_state_change.lock().await;
                if Instant::now().duration_since(*last) >= self.reset_timeout {
                    drop(last);
                    *state = CircuitState::HalfOpen;
                    *self.success_count.lock().await = 0;
                    *self.last_state_change.lock().await = Instant::now();
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful call.
    pub async fn on_success(&self) {
        let mut state = self.state.lock().await;
        match *state {
            CircuitState::Closed => {
                *self.failure_count.lock().await = 0;
            }
            CircuitState::HalfOpen => {
                let mut success = self.success_count.lock().await;
                *success += 1;
                if *success >= self.success_threshold {
                    *state = CircuitState::Closed;
                    *self.failure_count.lock().await = 0;
                    *success = 0;
                    *self.last_state_change.lock().await = Instant::now();
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed call.
    pub async fn on_failure(&self) {
        let mut state = self.state.lock().await;
        match *state {
            CircuitState::Closed => {
                let mut failures = self.failure_count.lock().await;
                *failures += 1;
                if *failures >= self.failure_threshold {
                    *state = CircuitState::Open;
                    *failures = 0;
                    *self.last_state_change.lock().await = Instant::now();
                }
            }
            CircuitState::HalfOpen => {
                *state = CircuitState::Open;
                *self.success_count.lock().await = 0;
                *self.last_state_change.lock().await = Instant::now();
            }
            CircuitState::Open => {}
        }
    }

    pub async fn state(&self) -> CircuitState {
        *self.state.lock().await
    }
}
