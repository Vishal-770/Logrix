# logrix-resilience

Fault-tolerance primitives for rate limiting, retry backoff, and circuit breaking in Logrix.

## Overview
Provides zero-allocation, thread-safe resilience utilities used across RPC clients, webhooks, and storage pipelines. Designed for high-concurrency workloads using single-mutex atomic state encapsulation to eliminate lock contention.

## Primitives
- **`CircuitBreaker`**:
  - Three-state machine (`Closed` -> `Open` -> `HalfOpen`) with atomic state transitions and configurable failure/success thresholds.
  - **Half-Open Probe Throttling**: Restricts concurrent test executions during recovery to prevent thundering herd storms from collapsing recovering upstreams.
  - **Operational Controls**: Includes manual override controls (`reset()`, `trip()`), failure inspection (`failure_count()`), and `serde` serialization for telemetry/health reporting.
- **`RateLimiter`**:
  - Token bucket rate limiter supporting fractional refill bursts and single-mutex atomic updates.
  - **Zero-Spin Acquisition**: `acquire()` computes the exact refill duration needed for missing tokens, replacing busy polling with precise asynchronous sleep.
  - **Inspection**: Real-time `available_tokens()` inspector.
- **`RetryPolicy`**:
  - Full-jitter exponential backoff retry calculations with capped ceilings to avoid correlated client retries.
  - Fluent builder configuration (`with_multiplier()`, `with_jitter()`).
- **`RetryBudget`**:
  - Token-bucket budget guarding against retry storms and downstream cascade failures.
  - Atomic state tracking with `available_budget()` inspection.

---

## Configuration & Tuning

### 1. Declarative Manifest Configuration (`logrix.yaml`)

End users configure resilience policies (circuit breakers, timeouts, rate limiting) under `indexing:` in `logrix.yaml`:

```yaml
indexing:
  rpcTimeoutSecs: 15        # Maximum request duration before timing out
  rpcCbThreshold: 5         # Consecutive failures before tripping circuit breaker
  rpcCooldownSecs: 30       # Circuit breaker recovery cooldown in seconds
```

| `logrix.yaml` Key | Type | Default Value | Required? | Description |
|---|---|---|---|---|
| `indexing.rpcTimeoutSecs` | `integer` | `10` (CLI) / `15` (Direct) | Optional | Request timeout before cancellation |
| `indexing.rpcCbThreshold` | `integer` | `5` (CLI) / `3` (Direct) | Optional | Consecutive failures to trip circuit breaker |
| `indexing.rpcCooldownSecs`| `integer` | `30` (CLI) / `10` (Direct) | Optional | Half-open recovery probing delay |

### 2. Programmatic Primitive Tuning

### 1. `CircuitBreaker`

| Parameter | Type | Default in Gateway | Default in CLI | Description |
|---|---|---|---|---|
| `failure_threshold` | `u32` | `3` errors | `5` errors (`RPC_CB_THRESHOLD`) | Consecutive errors before tripping circuit to `Open` |
| `success_threshold` | `u32` | `2` successes | `2` successes | Consecutive successful probes to restore to `Closed` |
| `reset_timeout` | `Duration` | `10s` | `30s` (`RPC_COOLDOWN_SECS`) | Duration spent in `Open` before probing in `HalfOpen` |

```rust
use logrix_resilience::CircuitBreaker;
use std::time::Duration;

<<<<<<< HEAD
let cb = CircuitBreaker::new(5, 2, Duration::from_secs(30));
if cb.can_execute().await {
    // Perform network call
}
```

### 2. `RateLimiter`

| Parameter | Type | Default in EVM Client | Description |
|---|---|---|---|
| `capacity` | `u32` | `100` tokens | Maximum bucket burst capacity |
| `refill_rate` | `f64` | `50.0` tokens/sec | Tokens added back per second |
| `initial_tokens` | `f64` | `capacity` | Initial balance (set via `with_initial_tokens`) |
| `min_sleep_floor` | `Duration` | `1ms` | Minimum delay floor (set via `with_min_sleep_floor`) |

```rust
use logrix_resilience::RateLimiter;
use std::time::Duration;

let limiter = RateLimiter::new(100, 50.0)
    .with_initial_tokens(0.0) // cold-start warmup
    .with_min_sleep_floor(Duration::from_micros(500));

if limiter.try_acquire(1.0) {
    // Non-blocking acquisition
}
limiter.acquire(1.0).await; // Waits asynchronously if tokens exhausted
```

### 3. `RetryPolicy`

| Parameter | Type | Default in EVM Client | Description |
|---|---|---|---|
| `max_retries` | `u32` | `3` attempts | Maximum retry attempts |
| `initial_backoff` | `Duration` | `200ms` | Initial backoff duration before first retry |
| `max_backoff` | `Duration` | `5s` | Maximum capped backoff ceiling |
| `multiplier` | `f64` | `2.0` | Exponential backoff growth factor |
| `jitter` | `bool` | `true` | Full jitter randomization to decorrelate retries |

```rust
use logrix_resilience::RetryPolicy;
use std::time::Duration;

let policy = RetryPolicy::new(3, Duration::from_millis(200), Duration::from_secs(5))
    .with_multiplier(2.0)
    .with_jitter(true);
```

### 4. `RetryBudget`

| Parameter | Type | Default | Description |
|---|---|---|---|
| `capacity` | `u32` | Custom | Maximum retry burst capacity |
| `fill_rate_per_sec` | `f64` | Custom | Retry tokens replenished per second |

```rust
use logrix_resilience::RetryBudget;

let budget = RetryBudget::new(10, 2.0);
if budget.try_acquire() {
    // Retry permitted synchronously without .await
}
```
