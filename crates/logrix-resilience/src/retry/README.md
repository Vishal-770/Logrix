# Retry & Budget Subsystem

Provides exponential backoff policies with decorrelated jitter and token-bucket retry budgets for distributed pipeline operations and external API requests.

---

## Capabilities

- **Exponential Backoff**: Configurable multiplier scaling with strict upper-bound clamping.
- **Decorrelated Full Jitter**: Randomizes backoff intervals to eliminate synchronized client retries.
- **Retry Budget Enforcement**: Implements token-bucket pacing to prevent retry storms from cascading into upstream or downstream dependencies.
- **Synchronous Acquisition**: Thread-safe, non-blocking token checks powered by atomic locking primitives.

---

## API Reference & Usage

### 1. `RetryPolicy`

Defines retry attempt limits, initial and maximum delay intervals, backoff multipliers, and jitter behavior.

#### Configuration Options

| Option | Type | Default | Description |
|---|---|---|---|
| `max_attempts` | `u32` | `3` | Maximum number of attempts permitted before failing. |
| `initial_delay` | `Duration` | `100ms` | Base backoff interval applied before the first retry. |
| `max_delay` | `Duration` | `5s` | Maximum ceiling for computed backoff delays. |
| `multiplier` | `f64` | `2.0` | Exponential scaling factor applied per attempt. |
| `jitter` | `bool` | `true` | When enabled, randomizes delay within `[0, computed_delay)`. |

#### Example

```rust
use logrix_resilience::RetryPolicy;
use std::time::Duration;

let policy = RetryPolicy::new(
    4,
    Duration::from_millis(200),
    Duration::from_secs(5),
)
.with_multiplier(1.5)
.with_jitter(true);

let mut attempt = 1;
while policy.should_retry(attempt) {
    let delay = policy.delay_for_attempt(attempt);
    tokio::time::sleep(delay).await;
    
    // Execute request logic
    attempt += 1;
}
```

---

### 2. `RetryBudget`

Guards systems against retry amplification by allocating a sliding token allowance for retries relative to elapsed time.

#### Methods

- `new(capacity: u32, fill_rate_per_sec: f64) -> Self`: Instantiates a budget with maximum token capacity and replenishment velocity.
- `try_acquire(&self) -> bool`: Consumes a single retry token if available; returns `false` if the budget is exhausted.
- `available_budget(&self) -> f64`: Inspects current unreserved token balance.
- `capacity(&self) -> u32`: Returns configured capacity limit.
- `fill_rate_per_sec(&self) -> f64`: Returns replenishment rate in tokens per second.

#### Example

```rust
use logrix_resilience::RetryBudget;

let budget = RetryBudget::new(10, 2.0); // Capacity: 10, Refill: 2 tokens/sec

if budget.try_acquire() {
    // Retry permitted under operational budget
} else {
    // Budget exhausted; fail fast to preserve downstream health
}
```
