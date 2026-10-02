use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::{debug, warn};

/// Adaptive Additive-Increase / Multiplicative-Decrease (AIMD) block chunk sizer.
///
/// Prevents RPC payload limit errors (e.g. "response too large", "query returned >10000 results")
/// while dynamically scaling up block chunking during quiet periods for maximum throughput.
#[derive(Debug)]
pub struct AdaptiveChunker {
    current_size: AtomicU64,
    min_size: u64,
    max_size: u64,
    additive_step: u64,
    consecutive_successes: AtomicU64,
    success_threshold_to_grow: u64,
    target_latency: Duration,
}

impl AdaptiveChunker {
    pub fn new(initial_size: u64, min_size: u64, max_size: u64) -> Self {
        Self {
            current_size: AtomicU64::new(initial_size.clamp(min_size, max_size)),
            min_size,
            max_size,
            additive_step: 50,
            consecutive_successes: AtomicU64::new(0),
            success_threshold_to_grow: 3,
            target_latency: Duration::from_millis(800),
        }
    }

    /// Retrieve the current recommended block chunk size.
    pub fn chunk_size(&self) -> u64 {
        self.current_size.load(Ordering::Relaxed)
    }

    /// Record a successful RPC fetch and latency.
    pub fn record_success(&self, latency: Duration) {
        let successes = self.consecutive_successes.fetch_add(1, Ordering::Relaxed) + 1;

        if successes >= self.success_threshold_to_grow && latency < self.target_latency {
            self.consecutive_successes.store(0, Ordering::Relaxed);
            let mut cur = self.current_size.load(Ordering::Relaxed);
            loop {
                let new_size = (cur + self.additive_step).min(self.max_size);
                match self.current_size.compare_exchange_weak(
                    cur,
                    new_size,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(old) => {
                        if new_size != old {
                            debug!(
                                old_chunk = old,
                                new_chunk = new_size,
                                "Adaptive chunker increased chunk size"
                            );
                        }
                        break;
                    }
                    Err(actual) => cur = actual,
                }
            }
        }
    }

    /// Record an RPC error (e.g. timeout, payload size limit, rate limit), triggering multiplicative backoff.
    pub fn record_failure(&self) {
        self.consecutive_successes.store(0, Ordering::Relaxed);
        let mut cur = self.current_size.load(Ordering::Relaxed);
        loop {
            let new_size = (cur / 2).max(self.min_size);
            match self.current_size.compare_exchange_weak(
                cur,
                new_size,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(old) => {
                    let cut_size = (old / 2).max(self.min_size);
                    warn!(
                        old_chunk = old,
                        new_chunk = cut_size,
                        "Adaptive chunker cut chunk size due to RPC error"
                    );
                    break;
                }
                Err(actual) => cur = actual,
            }
        }
    }
}
