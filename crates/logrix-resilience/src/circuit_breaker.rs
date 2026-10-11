use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
struct InnerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    half_open_probes: u32,
    last_state_change: Instant,
}

/// Three-state circuit breaker preventing calls to failing external dependencies.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    inner: Arc<Mutex<InnerState>>,
    failure_threshold: u32,
    success_threshold: u32,
    reset_timeout: Duration,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, success_threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerState {
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                half_open_probes: 0,
                last_state_change: Instant::now(),
            })),
            failure_threshold,
            success_threshold,
            reset_timeout,
        }
    }

    /// Check if call is permitted under current circuit state.
    pub async fn can_execute(&self) -> bool {
        let mut inner = self.inner.lock().await;
        match inner.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if Instant::now().duration_since(inner.last_state_change) >= self.reset_timeout {
                    inner.state = CircuitState::HalfOpen;
                    inner.success_count = 0;
                    inner.half_open_probes = 1;
                    inner.last_state_change = Instant::now();
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                if inner.half_open_probes < self.success_threshold {
                    inner.half_open_probes += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record a successful call.
    pub async fn on_success(&self) {
        let mut inner = self.inner.lock().await;
        match inner.state {
            CircuitState::Closed => {
                inner.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                inner.success_count += 1;
                if inner.success_count >= self.success_threshold {
                    inner.state = CircuitState::Closed;
                    inner.failure_count = 0;
                    inner.success_count = 0;
                    inner.half_open_probes = 0;
                    inner.last_state_change = Instant::now();
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed call.
    pub async fn on_failure(&self) {
        let mut inner = self.inner.lock().await;
        match inner.state {
            CircuitState::Closed => {
                inner.failure_count += 1;
                if inner.failure_count >= self.failure_threshold {
                    inner.state = CircuitState::Open;
                    inner.failure_count = 0;
                    inner.last_state_change = Instant::now();
                }
            }
            CircuitState::HalfOpen => {
                inner.state = CircuitState::Open;
                inner.success_count = 0;
                inner.half_open_probes = 0;
                inner.last_state_change = Instant::now();
            }
            CircuitState::Open => {}
        }
    }

    pub async fn state(&self) -> CircuitState {
        self.inner.lock().await.state
    }

    pub async fn failure_count(&self) -> u32 {
        self.inner.lock().await.failure_count
    }

    /// Manually reset circuit to Closed state.
    pub async fn reset(&self) {
        let mut inner = self.inner.lock().await;
        inner.state = CircuitState::Closed;
        inner.failure_count = 0;
        inner.success_count = 0;
        inner.half_open_probes = 0;
        inner.last_state_change = Instant::now();
    }

    /// Manually trip circuit to Open state.
    pub async fn trip(&self) {
        let mut inner = self.inner.lock().await;
        inner.state = CircuitState::Open;
        inner.failure_count = 0;
        inner.success_count = 0;
        inner.half_open_probes = 0;
        inner.last_state_change = Instant::now();
    }
}
