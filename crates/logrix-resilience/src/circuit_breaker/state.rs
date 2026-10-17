use serde::{Deserialize, Serialize};
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
    pub(crate) failure_count: u32,
    pub(crate) success_count: u32,
    pub(crate) half_open_probes: u32,
    pub(crate) last_state_change: Instant,
    pub(crate) last_failure_time: Option<Instant>,
}

impl InnerState {
    pub(crate) fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            half_open_probes: 0,
            last_state_change: Instant::now(),
            last_failure_time: None,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.success_count = 0;
        self.half_open_probes = 0;
        self.last_state_change = Instant::now();
        self.last_failure_time = None;
    }

    pub(crate) fn trip(&mut self) {
        self.state = CircuitState::Open;
        self.failure_count = 0;
        self.success_count = 0;
        self.half_open_probes = 0;
        self.last_state_change = Instant::now();
    }
}
