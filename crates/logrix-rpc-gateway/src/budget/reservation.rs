use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// RAII reservation that holds pre-allocated CUs and refunds on cancellation.
#[derive(Debug)]
pub struct CuReservation {
    pub(crate) consumed_units: Arc<AtomicU64>,
    pub(crate) cu: u64,
    pub(crate) committed: bool,
}

impl CuReservation {
    pub(crate) fn new(consumed_units: Arc<AtomicU64>, cu: u64) -> Self {
        Self {
            consumed_units,
            cu,
            committed: false,
        }
    }

    /// Commit the reserved compute units as permanent expenditure.
    pub fn commit(mut self) {
        self.committed = true;
    }

    /// Get the reserved compute unit amount.
    pub fn reserved_cu(&self) -> u64 {
        self.cu
    }
}

impl Drop for CuReservation {
    fn drop(&mut self) {
        if !self.committed {
            self.consumed_units.fetch_sub(self.cu, Ordering::Relaxed);
        }
    }
}
