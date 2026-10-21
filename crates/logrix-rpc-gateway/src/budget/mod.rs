pub mod reservation;
pub mod tracker;

pub use reservation::CuReservation;
pub use tracker::{
    method_cu, CuBudgetTracker, COST_PER_MILLION_CU_USD, CU_BLOCK_NUMBER, CU_GET_BLOCK,
    CU_GET_BLOCK_ENVELOPE, CU_GET_LOGS,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_accumulation_and_exhaustion() {
        let tracker = CuBudgetTracker::new(Some(100));
        assert!(!tracker.is_exhausted());

        tracker.record_method("eth_blockNumber");
        assert_eq!(tracker.total_consumed(), 10);
        assert!(!tracker.is_exhausted());

        tracker.record_method("eth_getLogs");
        assert_eq!(tracker.total_consumed(), 85);
        assert!(!tracker.is_exhausted());

        tracker.record_method("eth_getBlockByNumber");
        assert_eq!(tracker.total_consumed(), 105);
        assert!(tracker.is_exhausted());
    }

    #[test]
    fn test_atomic_reservation_and_commit() {
        let tracker = CuBudgetTracker::new(Some(100));
        let reservation = tracker.try_reserve(75).expect("reservation within limit");
        assert_eq!(tracker.total_consumed(), 75);

        // Next request for 30 CU exceeds 100 max and is rejected
        assert!(tracker.try_reserve(30).is_none());

        // Commit reservation
        reservation.commit();
        assert_eq!(tracker.total_consumed(), 75);
    }

    #[test]
    fn test_atomic_reservation_and_rollback_on_drop() {
        let tracker = CuBudgetTracker::new(Some(100));
        {
            let _reservation = tracker.try_reserve(80).expect("reservation within limit");
            assert_eq!(tracker.total_consumed(), 80);
            // Dropped without commit -> refunds
        }
        assert_eq!(tracker.total_consumed(), 0);
    }
}
