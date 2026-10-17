use crate::circuit_breaker::config::CircuitConfig;
use crate::circuit_breaker::guard::CircuitGuard;
use crate::circuit_breaker::state::{CircuitState, InnerState};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Instant;

/// Three-state circuit breaker preventing calls to failing external dependencies.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    inner: Arc<Mutex<InnerState>>,
    config: CircuitConfig,
}

impl CircuitBreaker {
    pub fn new(
        failure_threshold: u32,
        success_threshold: u32,
        reset_timeout: std::time::Duration,
    ) -> Self {
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

    /// Read-only check whether the circuit is currently in a state that permits calls.
    /// Does not reserve probe slots or modify internal state.
    pub fn can_execute(&self) -> bool {
        let inner = self.inner.lock();
        match inner.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                Instant::now().duration_since(inner.last_state_change) >= self.config.reset_timeout
            }
            CircuitState::HalfOpen => {
                inner.active_probes < self.config.effective_half_open_probes()
            }
        }
    }

    /// Attempt to acquire an RAII execution permit guard.
    /// If in HalfOpen, reserves an active probe slot.
    pub fn try_acquire(&self) -> Option<CircuitGuard> {
        let mut inner = self.inner.lock();
        let (permitted, was_probe) = self.try_permit(&mut inner);
        if permitted {
            Some(CircuitGuard::new(
                Arc::clone(&self.inner),
                self.config.clone(),
                was_probe,
            ))
        } else {
            None
        }
    }

    fn try_permit(&self, inner: &mut InnerState) -> (bool, bool) {
        match inner.state {
            CircuitState::Closed => {
                inner.prune_stale_failures(&self.config);
                (true, false)
            }
            CircuitState::Open => {
                if Instant::now().duration_since(inner.last_state_change)
                    >= self.config.reset_timeout
                {
                    inner.state = CircuitState::HalfOpen;
                    inner.success_count = 0;
                    inner.active_probes = 1;
                    inner.last_state_change = Instant::now();
                    (true, true)
                } else {
                    (false, false)
                }
            }
            CircuitState::HalfOpen => {
                let max_probes = self.config.effective_half_open_probes();
                if inner.active_probes < max_probes {
                    inner.active_probes += 1;
                    (true, true)
                } else {
                    (false, false)
                }
            }
        }
    }

    /// Record a successful call.
    pub fn on_success(&self) {
        let mut inner = self.inner.lock();
        if inner.active_probes > 0 {
            inner.active_probes -= 1;
        }
        inner.record_success(&self.config);
    }

    /// Record a failed call.
    pub fn on_failure(&self) {
        let mut inner = self.inner.lock();
        if inner.active_probes > 0 {
            inner.active_probes -= 1;
        }
        inner.record_failure(&self.config);
    }

    /// Observe the current circuit state, lazily reconciling recovery timeout.
    pub fn state(&self) -> CircuitState {
        let mut inner = self.inner.lock();
        if inner.state == CircuitState::Open
            && Instant::now().duration_since(inner.last_state_change) >= self.config.reset_timeout
        {
            inner.state = CircuitState::HalfOpen;
            inner.success_count = 0;
            inner.active_probes = 0;
            inner.last_state_change = Instant::now();
        }
        inner.state
    }

    /// Read-only inspection of the current circuit state.
    pub fn current_state(&self) -> CircuitState {
        self.state()
    }

    pub fn failure_count(&self) -> u32 {
        self.inner.lock().failure_count(&self.config)
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
