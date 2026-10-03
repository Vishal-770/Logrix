use crate::error::LogrixResult;
pub use crate::lease::KubernetesLeaseElector;
use crate::ports::LeaderElectionPort;
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::lease::{LeaseMetadata, LeaseObject, LeaseSpec};

/// In-memory leader elector for local standalone execution and testing.
#[derive(Debug, Default)]
pub struct LocalLeaderElector {
    is_leader: AtomicBool,
}

impl LocalLeaderElector {
    /// Create new local leader elector (defaults to leader).
    pub fn new() -> Self {
        Self {
            is_leader: AtomicBool::new(true),
        }
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
        self.is_leader.store(false, Ordering::SeqCst);
        Ok(())
    }
}

pub use crate::lease::KubernetesLeaseElector as K8sLeaseElector;
