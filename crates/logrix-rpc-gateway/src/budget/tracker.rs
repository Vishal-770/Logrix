use super::reservation::CuReservation;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{info, warn};

pub const CU_BLOCK_NUMBER: u64 = 10;
pub const CU_GET_BLOCK: u64 = 20;
pub const CU_GET_LOGS: u64 = 75;
pub const CU_GET_BLOCK_ENVELOPE: u64 = 95;
pub const COST_PER_MILLION_CU_USD: f64 = 1.00;

pub fn method_cu(method: &str) -> u64 {
    match method {
        "eth_blockNumber" => CU_BLOCK_NUMBER,
        "eth_getBlockByNumber" => CU_GET_BLOCK,
        "eth_getLogs" => CU_GET_LOGS,
        "eth_getBlockEnvelope" => CU_GET_BLOCK_ENVELOPE,
        _ => 15,
    }
}

/// Atomic Compute Unit (CU) budget tracker.
#[derive(Debug, Clone)]
pub struct CuBudgetTracker {
    max_budget: Option<u64>,
    consumed_units: Arc<AtomicU64>,
}

impl CuBudgetTracker {
    pub fn new(max_budget: Option<u64>) -> Self {
        Self {
            max_budget,
            consumed_units: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Try to atomically reserve compute units before dispatching an RPC request.
    pub fn try_reserve(&self, cu: u64) -> Option<CuReservation> {
        if let Some(max) = self.max_budget {
            let mut current = self.consumed_units.load(Ordering::Relaxed);
            loop {
                if current.saturating_add(cu) > max {
                    return None;
                }
                match self.consumed_units.compare_exchange_weak(
                    current,
                    current + cu,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(actual) => current = actual,
                }
            }
        } else {
            self.consumed_units.fetch_add(cu, Ordering::Relaxed);
        }

        Some(CuReservation::new(Arc::clone(&self.consumed_units), cu))
    }

    pub fn record_method(&self, method: &str) -> u64 {
        self.record_cu(method_cu(method))
    }

    pub fn record_cu(&self, cu: u64) -> u64 {
        let prev = self.consumed_units.fetch_add(cu, Ordering::Relaxed);
        let total = prev + cu;
        if let Some(max) = self.max_budget {
            if total >= max && prev < max {
                warn!(consumed = total, budget = max, "RPC CU budget exhausted!");
            }
        }
        total
    }

    pub fn total_consumed(&self) -> u64 {
        self.consumed_units.load(Ordering::Relaxed)
    }

    pub fn max_budget(&self) -> Option<u64> {
        self.max_budget
    }

    pub fn is_exhausted(&self) -> bool {
        if let Some(max) = self.max_budget {
            self.total_consumed() >= max
        } else {
            false
        }
    }

    pub fn estimated_usd_cost(&self) -> f64 {
        (self.total_consumed() as f64 / 1_000_000.0) * COST_PER_MILLION_CU_USD
    }

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
        let total_cu = chunks * (CU_GET_LOGS + CU_GET_BLOCK);
        let estimated_usd = (total_cu as f64 / 1_000_000.0) * COST_PER_MILLION_CU_USD;
        (chunks, total_cu, estimated_usd)
    }

    pub fn report(&self) {
        let consumed = self.total_consumed();
        let cost = self.estimated_usd_cost();
        if let Some(max) = self.max_budget {
            let pct = (consumed as f64 / max as f64) * 100.0;
            info!(
                consumed,
                budget = max,
                percent_used = format!("{:.1}%", pct),
                "CU Budget Status"
            );
        } else {
            info!(
                consumed,
                estimated_usd = format!("${:.4}", cost),
                "CU Budget Status"
            );
        }
    }
}
