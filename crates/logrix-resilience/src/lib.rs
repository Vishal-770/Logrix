//! Shared resilience layer for Logrix (timeouts, retries, rate limits, circuit breakers).

pub mod circuit_breaker;
pub mod rate_limit;
pub mod retry;

pub use circuit_breaker::{CircuitBreaker, CircuitState};
pub use rate_limit::RateLimiter;
pub use retry::{RetryBudget, RetryPolicy};
