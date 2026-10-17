# Rate Limiting Subsystem

High-throughput, fractional token bucket rate limiting designed for distributed RPC queries, webhook deliveries, and ingestion pipelines.

---

## Capabilities

- **Fractional Token Accounting**: Continuous sub-second token accumulation without discretization drift.
- **Zero-Spin Asynchronous Delays**: Calculates precise sleep durations for token shortfalls, avoiding thread spinning or busy loops.
- **Oversized Request Protection**: Safely handles request costs exceeding maximum bucket capacity without deadlocking execution threads.
- **Cold-Start Pacing**: Configurable initial token balances to control system warmup curves.

---

## API Reference & Usage

### `TokenBucketRateLimiter` (Aliased as `RateLimiter`)

Implements the token bucket algorithm with both non-blocking synchronous checks and asynchronous computed delays.

#### Configuration Options

| Option | Type | Default | Description |
|---|---|---|---|
| `capacity` | `u32` | Required | Maximum burst token capacity. |
| `refill_rate` | `f64` | Required | Rate of token replenishment per second. |
| `initial_tokens` | `f64` | `capacity` | Initial balance assigned upon creation (e.g. `0.0` for warmup). |
| `min_sleep_floor` | `Duration` | `1ms` | Minimum delay floor applied to sub-millisecond asynchronous waits. |

#### Methods

- `new(capacity: u32, refill_rate: f64) -> Self`: Creates a rate limiter initialized with full capacity.
- `with_initial_tokens(self, tokens: f64) -> Self`: Sets custom starting tokens.
- `with_min_sleep_floor(self, floor: Duration) -> Self`: Sets minimum sleep granularity.
- `try_acquire(&self, cost: f64) -> bool`: Immediately checks and deducts tokens if available; non-blocking.
- `acquire(&self, cost: f64) -> impl Future<Output = ()>`: Asynchronously awaits until the requested tokens are available.
- `available_tokens(&self) -> f64`: Inspects current token balance.
- `capacity(&self) -> f64`: Returns maximum token capacity.
- `refill_rate(&self) -> f64`: Returns replenishment rate per second.

#### Example

```rust
use logrix_resilience::RateLimiter;
use std::time::Duration;

let limiter = RateLimiter::new(100, 50.0)
    .with_initial_tokens(10.0)
    .with_min_sleep_floor(Duration::from_micros(500));

// Non-blocking synchronous check
if limiter.try_acquire(1.0) {
    // Process request immediately
}

// Asynchronous calculated wait
limiter.acquire(1.0).await;
```
