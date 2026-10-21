use crate::circuit_breaker::config::CircuitConfig;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
pub(crate) struct InnerState {
    pub(crate) state: CircuitState,
    pub(crate) success_count: u32,
    pub(crate) active_probes: u32,
    pub(crate) epoch: u64,
    pub(crate) last_state_change: Instant,
    pub(crate) failure_timestamps: VecDeque<Instant>,
}

impl InnerState {
    pub(crate) fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            success_count: 0,
            active_probes: 0,
            epoch: 0,
            last_state_change: Instant::now(),
            failure_timestamps: VecDeque::new(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.success_count = 0;
        self.active_probes = 0;
        self.epoch = self.epoch.wrapping_add(1);
        self.last_state_change = Instant::now();
        self.failure_timestamps.clear();
    }

    pub(crate) fn trip(&mut self) {
        self.state = CircuitState::Open;
        self.success_count = 0;
        self.active_probes = 0;
        self.epoch = self.epoch.wrapping_add(1);
        self.last_state_change = Instant::now();
        self.failure_timestamps.clear();
    }

    pub(crate) fn prune_stale_failures(&mut self, config: &CircuitConfig) {
        if let Some(window) = config.failure_window {
            let cutoff = Instant::now()
                .checked_sub(window)
                .unwrap_or_else(Instant::now);
            while let Some(&front) = self.failure_timestamps.front() {
                if front < cutoff {
                    self.failure_timestamps.pop_front();
                } else {
                    break;
                }
            }
        }
    }

    pub(crate) fn failure_count(&mut self, config: &CircuitConfig) -> u32 {
        self.prune_stale_failures(config);
        self.failure_timestamps.len() as u32
    }

    pub(crate) fn record_success(&mut self, config: &CircuitConfig) {
        match self.state {
            CircuitState::Closed => {
                self.failure_timestamps.clear();
            }
            CircuitState::HalfOpen => {
                self.success_count += 1;
                if self.success_count >= config.success_threshold {
                    self.reset();
                }
            }
            CircuitState::Open => {}
        }
    }

    pub(crate) fn record_failure(&mut self, config: &CircuitConfig) {
        match self.state {
            CircuitState::Closed => {
                self.prune_stale_failures(config);
                self.failure_timestamps.push_back(Instant::now());
                if self.failure_timestamps.len() as u32 >= config.failure_threshold {
                    self.trip();
                }
            }
            CircuitState::HalfOpen => {
                self.trip();
            }
            CircuitState::Open => {}
        }
    }
}
