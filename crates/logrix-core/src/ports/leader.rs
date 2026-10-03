use crate::error::LogrixResult;
use async_trait::async_trait;

/// Port for leader election in distributed and high-availability topologies.
#[async_trait]
pub trait LeaderElectionPort: Send + Sync {
    /// Attempt to acquire or renew leadership. Returns true if this instance is leader.
    async fn try_acquire_or_renew(&self) -> LogrixResult<bool>;

    /// Check if this instance currently holds active leadership.
    async fn is_leader(&self) -> bool;

    /// Step down voluntarily from leadership.
    async fn step_down(&self) -> LogrixResult<()>;
}
