use crate::circuit_breaker::config::CircuitConfig;
use crate::circuit_breaker::state::{CircuitState, InnerState};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Instant;

/// RAII execution guard that tracks an in-flight circuit permit.
///
/// If dropped without calling `record_success()` or `finish()`,
/// it safely records a failure to avoid probe starvation.
#[derive(Debug)]
pub struct CircuitGuard {
    inner: Arc<Mutex<InnerState>>,
    config: CircuitConfig,
    finished: bool,
}

impl CircuitGuard {
    pub(crate) fn new(inner: Arc<Mutex<InnerState>>, config: CircuitConfig) -> Self {
        Self {
            inner,
            config,
            finished: false,
        }
    }

    /// Record that the execution was successful.
    pub fn record_success(mut self) {
        self.finish(true);
    }

    /// Record that the execution failed.
    pub fn record_failure(mut self) {
        self.finish(false);
    }

    /// Explicitly finish the execution permit with success or failure.
    pub fn finish(&mut self, success: bool) {
        if self.finished {
            return;
        }
        self.finished = true;

        let mut inner = self.inner.lock();
        if success {
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
        } else {
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
    }
}

impl Drop for CircuitGuard {
    fn drop(&mut self) {
        if !self.finished {
            // Guard dropped prematurely (cancellation, error, or panic)
            self.finish(false);
        }
    }
}
