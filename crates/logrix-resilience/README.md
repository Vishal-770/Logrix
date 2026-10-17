# logrix-resilience

Fault-tolerance primitives for rate limiting, retry backoff, and circuit breaking across Logrix services.

`logrix-resilience` provides lightweight, zero-allocation reliability primitives designed for high-concurrency EVM blockchain indexing, RPC provider gateways, webhook dispatchers, and distributed storage pipelines.

---

## Subsystems

The crate is structured into three dedicated subsystems:

| Subsystem | Source Path | Description | Subsystem Guide |
|---|---|---|---|
| **Retry & Budget** | [`src/retry/`](src/retry) | Exponential backoff policies, decorrelated full jitter, and token-bucket retry budgets | [Retry Guide](src/retry/README.md) |
| **Rate Limiting** | [`src/rate_limit/`](src/rate_limit) | Fractional token bucket rate limiter with zero-spin asynchronous sleep calculation | [Rate Limit Guide](src/rate_limit/README.md) |
| **Circuit Breaker** | [`src/circuit_breaker/`](src/circuit_breaker) | Three-state breaker with probe throttling, sliding decay windows, and RAII execution guards | [Circuit Breaker Guide](src/circuit_breaker/README.md) |

---

## Architecture & Concurrency

1. **Sub-Microsecond In-Memory Fast Paths**:
   Internal state synchronization uses `parking_lot::Mutex`. Mutex hold times are bounded to simple arithmetic operations (~5 nanoseconds), enabling fast, synchronous checks (`can_execute()`, `try_acquire()`, `state()`) without Tokio future state machine allocations.
2. **Thundering Herd & Storm Mitigation**:
   - Full jitter decorrelates retry timing spikes across concurrent workers.
   - Half-open probe throttling limits concurrent exploratory traffic to recovering services.
   - Retry budgets enforce hard limits on the fraction of total traffic consumed by retries.
3. **Hexagonal Architecture Boundary**:
   `logrix-resilience` is a pure domain library with zero external driver dependencies (no database drivers, RPC libraries, or cloud SDKs).

---

## Workspace Integration

```
+-----------------------------------------------------------+
|                      logrix-rpc-gateway                   |
|         (Provider health, fallback routing, EWMA)         |
+-----------------------------+-----------------------------+
                              | uses
+-----------------------------v-----------------------------+
|                      logrix-resilience                    |
|   (CircuitBreaker, RateLimiter, RetryPolicy, RetryBudget) |
+-----------------------------------------------------------+
                              ^
                              | uses
+-----------------------------+-----------------------------+
|                      logrix-chain-evm                     |
|           (RPC pacing, request retries, throttling)       |
+-----------------------------------------------------------+
```

---

## Manifest Configuration (`logrix.yaml`)

Resilience policies can be configured declaratively under `indexing:` in `logrix.yaml`:

```yaml
indexing:
  rpcTimeoutSecs: 15        # Maximum request duration before cancellation
  rpcCbThreshold: 5         # Consecutive failures before tripping circuit breaker
  rpcCooldownSecs: 30       # Circuit breaker recovery cooldown in seconds
```

---

## Quick Example

```rust
use logrix_resilience::{CircuitBreaker, RateLimiter, RetryPolicy};
use std::time::Duration;

// Rate limiting
let limiter = RateLimiter::new(100, 50.0);
limiter.acquire(1.0).await;

// Circuit breaking
let breaker = CircuitBreaker::new(5, 2, Duration::from_secs(30));
if breaker.can_execute() {
    // Retry policy
    let retry = RetryPolicy::default();
    let delay = retry.delay_for_attempt(1);
}
```
