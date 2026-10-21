use crate::circuit_breaker::config::CircuitConfig;
use crate::circuit_breaker::state::InnerState;
use parking_lot::Mutex;
use std::sync::Arc;

/// RAII execution permit that tracks active in-flight executions.
///
/// If dropped without calling `record_success()` or `finish()`,
/// it safely decrements `active_probes` and records a failure.
#[derive(Debug)]
pub struct CircuitGuard {
    inner: Arc<Mutex<InnerState>>,
    config: CircuitConfig,
    finished: bool,
    was_probe: bool,
    epoch: u64,
}

impl CircuitGuard {
    pub(crate) fn new(
        inner: Arc<Mutex<InnerState>>,
        config: CircuitConfig,
        was_probe: bool,
        epoch: u64,
    ) -> Self {
        Self {
            inner,
            config,
            finished: false,
            was_probe,
            epoch,
        }
    }

    /// Record that the execution was successful and release active probe.
    pub fn record_success(mut self) {
        self.finish(true);
    }

    /// Record that the execution failed and release active probe.
    pub fn record_failure(mut self) {
        self.finish(false);
    }

    /// Explicitly finish the execution permit with success or failure.
    /// Subsequent calls are idempotent no-ops.
    pub fn finish(&mut self, success: bool) {
        if self.finished {
            return;
        }
        self.finished = true;

        let mut inner = self.inner.lock();
        // Only decrement active_probes if the circuit is still in the same epoch
        if self.was_probe && inner.epoch == self.epoch && inner.active_probes > 0 {
            inner.active_probes -= 1;
        }

        if success {
            inner.record_success(&self.config);
        } else {
            inner.record_failure(&self.config);
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
