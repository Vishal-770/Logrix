use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

/// Method-weighted Compute Unit (CU) consumption values.
///
/// Based on typical RPC provider tiers (Alchemy, Infura, QuickNode):
/// - `eth_blockNumber`: 10 CUs
/// - `eth_getBlockByNumber`: 20 CUs
/// - `eth_getLogs`: 75 CUs
pub const CU_BLOCK_NUMBER: u64 = 10;
pub const CU_GET_BLOCK: u64 = 20;
pub const CU_GET_LOGS: u64 = 75;

/// Cost per million Compute Units in USD (standard Alchemy tier: ~$1.00 per 1M CU).
pub const COST_PER_MILLION_CU_USD: f64 = 1.00;

/// Atomic Compute Unit (CU) budget tracker.
///
/// Keeps track of real-time CU expenditure and pauses historical backfills
/// if an optional spending cap is exhausted, protecting against runaway cloud bills.
#[derive(Debug, Clone)]
pub struct CuBudgetTracker {
    max_budget: Option<u64>,
    consumed_units: Arc<AtomicU64>,
}

impl CuBudgetTracker {
    /// Create a new budget tracker with an optional hard limit.
    pub fn new(max_budget: Option<u64>) -> Self {
        Self {
            max_budget,
            consumed_units: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Record consumption of compute units for a given RPC method name.
    pub fn record_method(&self, method: &str) -> u64 {
        let cu = match method {
            "eth_blockNumber" => CU_BLOCK_NUMBER,
            "eth_getBlockByNumber" => CU_GET_BLOCK,
            "eth_getLogs" => CU_GET_LOGS,
            _ => 15, // fallback generic estimate
        };
        self.record_cu(cu)
    }

    /// Record explicit compute unit count.
    pub fn record_cu(&self, cu: u64) -> u64 {
        let prev = self.consumed_units.fetch_add(cu, Ordering::Relaxed);
        let total = prev + cu;
        if let Some(max) = self.max_budget {
            if total >= max && prev < max {
                warn!(
                    consumed = total,
                    budget = max,
                    "RPC Compute Unit budget exhausted! Backfill operations will be gracefully paused."
                );
            }
        }
        total
    }

    /// Total compute units consumed so far.
    pub fn total_consumed(&self) -> u64 {
        self.consumed_units.load(Ordering::Relaxed)
    }

    /// Configured maximum budget, if set.
    pub fn max_budget(&self) -> Option<u64> {
        self.max_budget
    }

    /// Check if the budget has been exceeded.
    pub fn is_exhausted(&self) -> bool {
        if let Some(max) = self.max_budget {
            self.total_consumed() >= max
        } else {
            false
        }
    }

    /// Calculate estimated USD cost so far based on $1/million CU.
    pub fn estimated_usd_cost(&self) -> f64 {
        (self.total_consumed() as f64 / 1_000_000.0) * COST_PER_MILLION_CU_USD
    }

    /// Estimate budget and cost for a given block range and average chunk size.
    pub fn estimate_range_cost(from_block: u64, to_block: u64, chunk_size: u64) -> (u64, u64, f64) {
        let total_blocks = if to_block >= from_block {
            to_block - from_block + 1
        } else {
            0
        };
        let chunks = if chunk_size > 0 {
            total_blocks.div_ceil(chunk_size)
        } else {
            1
        };

        // Each chunk executes roughly:
        // - 1 eth_getLogs (75 CU)
        // - 1 eth_getBlockByNumber (20 CU)
        let total_cu = chunks * (CU_GET_LOGS + CU_GET_BLOCK);
        let estimated_usd = (total_cu as f64 / 1_000_000.0) * COST_PER_MILLION_CU_USD;
        (chunks, total_cu, estimated_usd)
    }

    /// Log current consumption metrics.
    pub fn report(&self) {
        let consumed = self.total_consumed();
        let cost = self.estimated_usd_cost();
        if let Some(max) = self.max_budget {
            let pct = (consumed as f64 / max as f64) * 100.0;
            info!(
                consumed,
                budget = max,
                percent_used = format!("{:.1}%", pct),
                estimated_usd = format!("${:.4}", cost),
                "RPC CU Budget Status"
            );
        } else {
            info!(
                consumed,
                estimated_usd = format!("${:.4}", cost),
                "RPC CU Budget Status (unlimited)"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_accumulation_and_exhaustion() {
        let tracker = CuBudgetTracker::new(Some(100));
        assert!(!tracker.is_exhausted());

        tracker.record_method("eth_blockNumber"); // +10 = 10
        assert_eq!(tracker.total_consumed(), 10);
        assert!(!tracker.is_exhausted());

        tracker.record_method("eth_getLogs"); // +75 = 85
        assert_eq!(tracker.total_consumed(), 85);
        assert!(!tracker.is_exhausted());

        tracker.record_method("eth_getBlockByNumber"); // +20 = 105
        assert_eq!(tracker.total_consumed(), 105);
        assert!(tracker.is_exhausted());
    }

    #[test]
    fn test_dry_run_estimation() {
        // 10,000 blocks with chunk size 500 = 20 chunks
        let (chunks, cu, usd) = CuBudgetTracker::estimate_range_cost(1, 10_000, 500);
        assert_eq!(chunks, 20);
        assert_eq!(cu, 20 * (75 + 20));
        assert!((usd - (1900.0 / 1_000_000.0) * 1.0).abs() < 1e-6);
    }
}
