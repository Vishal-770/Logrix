use crate::error::LogrixResult;
pub use crate::ports::leader::LeaderElectionPort;
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};

/// In-memory leader elector for local standalone execution and testing.
#[derive(Debug)]
pub struct LocalLeaderElector {
    is_leader: AtomicBool,
}

impl LocalLeaderElector {
    /// Create new local leader elector (defaults to leader).
    pub fn new() -> Self {
        Self::with_initial_state(true)
    }

    /// Create a local elector with a caller-controlled initial state.
    pub fn with_initial_state(is_leader: bool) -> Self {
        Self {
            is_leader: AtomicBool::new(is_leader),
        }
    }

    /// Set the simulated leadership state for local execution and tests.
    pub fn set_leader(&self, is_leader: bool) {
        self.is_leader.store(is_leader, Ordering::SeqCst);
    }

    /// Simulate losing leadership without requiring an external adapter.
    pub fn simulate_leadership_loss(&self) {
        self.set_leader(false);
    }
}

impl Default for LocalLeaderElector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LeaderElectionPort for LocalLeaderElector {
    async fn try_acquire_or_renew(&self) -> LogrixResult<bool> {
        self.is_leader.store(true, Ordering::SeqCst);
        Ok(true)
    }

    async fn is_leader(&self) -> bool {
        self.is_leader.load(Ordering::SeqCst)
    }

    async fn step_down(&self) -> LogrixResult<()> {
        self.simulate_leadership_loss();
        Ok(())
    }
}
