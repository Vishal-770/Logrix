use crate::circuit_breaker::config::CircuitConfig;
use crate::circuit_breaker::guard::CircuitGuard;
use crate::circuit_breaker::state::{CircuitState, InnerState};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Three-state circuit breaker preventing calls to failing external dependencies.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    inner: Arc<Mutex<InnerState>>,
    config: CircuitConfig,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, success_threshold: u32, reset_timeout: Duration) -> Self {
        Self::with_config(CircuitConfig::new(
            failure_threshold,
            success_threshold,
            reset_timeout,
        ))
    }

    pub fn with_config(config: CircuitConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(InnerState::new())),
            config,
        }
    }

    pub fn config(&self) -> &CircuitConfig {
        &self.config
    }

    /// Check if call is permitted under current circuit state.
    pub fn can_execute(&self) -> bool {
        let mut inner = self.inner.lock();
        self.check_can_execute(&mut inner)
    }

    /// Attempt to acquire an execution permit guard.
    pub fn try_acquire(&self) -> Option<CircuitGuard> {
        let mut inner = self.inner.lock();
        if self.check_can_execute(&mut inner) {
            Some(CircuitGuard::new(
                Arc::clone(&self.inner),
                self.config.clone(),
            ))
        } else {
            None
        }
    }

    fn check_can_execute(&self, inner: &mut InnerState) -> bool {
        match inner.state {
            CircuitState::Closed => {
                if let Some(window) = self.config.failure_window {
                    if let Some(last_fail) = inner.last_failure_time {
                        if Instant::now().duration_since(last_fail) >= window {
                            inner.failure_count = 0;
                            inner.last_failure_time = None;
                        }
                    }
                }
                true
            }
            CircuitState::Open => {
                if Instant::now().duration_since(inner.last_state_change)
                    >= self.config.reset_timeout
                {
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
                let max_probes = self.config.effective_half_open_probes();
                if inner.half_open_probes < max_probes {
                    inner.half_open_probes += 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record a successful call.
    pub fn on_success(&self) {
        let mut inner = self.inner.lock();
        match inner.state {
            CircuitState::Closed => {
                inner.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                inner.success_count += 1;
                if inner.success_count >= self.config.success_threshold {
                    inner.reset();
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed call.
    pub fn on_failure(&self) {
        let mut inner = self.inner.lock();
        match inner.state {
            CircuitState::Closed => {
                inner.failure_count += 1;
                inner.last_failure_time = Some(Instant::now());
                if inner.failure_count >= self.config.failure_threshold {
                    inner.trip();
                }
            }
            CircuitState::HalfOpen => {
                inner.trip();
            }
            CircuitState::Open => {}
        }
    }

    pub fn state(&self) -> CircuitState {
        let mut inner = self.inner.lock();
        if inner.state == CircuitState::Open
            && Instant::now().duration_since(inner.last_state_change) >= self.config.reset_timeout
        {
            inner.state = CircuitState::HalfOpen;
            inner.success_count = 0;
            inner.half_open_probes = 0;
            inner.last_state_change = Instant::now();
        }
        inner.state
    }

    pub fn failure_count(&self) -> u32 {
        self.inner.lock().failure_count
    }

    /// Manually reset circuit to Closed state.
    pub fn reset(&self) {
        self.inner.lock().reset();
    }

    /// Manually trip circuit to Open state.
    pub fn trip(&self) {
        self.inner.lock().trip();
    }
}
