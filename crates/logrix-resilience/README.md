# logrix-resilience

Reusable retry-delay calculations, token buckets, and a circuit breaker for
Logrix callers. These primitives do not execute requests, apply HTTP timeouts,
classify errors, or run a retry loop on their own.

## Prerequisites

Use the repository's Rust workspace and lockfile. The async primitives require
Tokio; time-based acquisition/recovery needs a running Tokio timer. No broker,
database, or RPC service is required. Stateful primitives use `Arc<Mutex<_>>`;
clones share state within a process, not across replicas.

## Minimal example

With `logrix-resilience` and Tokio (macros and runtime features):

```rust
use logrix_resilience::{CircuitBreaker, RateLimiter, RetryBudget, RetryPolicy};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let policy = RetryPolicy::default().with_jitter(false);
    assert_eq!(policy.max_attempts, 5);
    assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(400));

    let limiter = RateLimiter::new(10, 5.0);
    let budget = RetryBudget::new(3, 1.0);
    let breaker = CircuitBreaker::new(5, 2, Duration::from_secs(30));
    if breaker.can_execute().await {
        limiter.acquire(1.0).await;
        // Execute one operation here and record its actual result.
        breaker.on_success().await;
    }
    assert!(budget.try_acquire().await);
}
```

Call `on_failure()` for a failed permitted operation. A retry loop should check
the error class, `should_retry(attempt)`, and its budget before sleeping/retrying.

## Public configuration APIs and defaults

### RetryPolicy

All five fields are public. `RetryPolicy::new(max_attempts, initial_delay,
max_delay)` takes explicit values for the first three and sets multiplier 2.0
and jitter true. `Default` uses:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `max_attempts` | `u32` | `5` | Boundary used by `should_retry(attempt)`. |
| `initial_delay` | `Duration` | `200 ms` | Delay ceiling for attempt 1. |
| `max_delay` | `Duration` | `10 s` | Maximum delay ceiling. |
| `multiplier` | `f64` | `2.0` | Exponential factor; set with `with_multiplier`. |
| `jitter` | `bool` | `true` | Full jitter; set with `with_jitter`. |

`should_retry(attempt)` is exactly `attempt < max_attempts`, including attempt
zero. With a one-based attempt counter, the default permits retry after failures
1 through 4, for five total attempts. The caller owns that counter and loop.

`delay_for_attempt(0)` is zero. For positive attempts, the ceiling is
`min(initial_delay_ms * multiplier^min(attempt - 1, 10), max_delay_ms)`.
Jitter samples between zero and that ceiling; results use whole milliseconds.
The exponent stops growing after attempt 11. Computing a delay does not check
`max_attempts`.

These field names/defaults are distinct from `EvmClientConfig`'s adapter settings
`max_retries = 3`, `retry_initial_backoff = 200 ms`, and
`retry_max_backoff = 5 s`.

### CircuitBreaker

`CircuitBreaker::new(failure_threshold: u32, success_threshold: u32,
reset_timeout: Duration)` has no `Default` implementation or implicit thresholds.
It starts `Closed`. Constructor parameters are private after construction.

- In `Closed`, successes reset the consecutive failure count; reaching the
  failure threshold opens the circuit and resets that count to zero.
- `can_execute()` rejects calls while open until `reset_timeout` has elapsed.
  That check, rather than a background timer or `state()`, enters `HalfOpen`.
- Half-open admission is limited to `success_threshold` probes over that recovery
  cycle. Successful results close the circuit when the threshold is reached;
  any half-open failure reopens it. There is no probe guard or timeout to release
  admission when a permitted call is abandoned; callers must report outcomes.
- `reset()` closes and clears counters; `trip()` opens and clears counters.
  `state()` and `failure_count()` inspect state. Only `CircuitState`, not the
  breaker object, supports Serde serialization.

### RateLimiter and RetryBudget

| Constructor | Parameters | Initial state |
|---|---|---|
| `RateLimiter::new(capacity, refill_rate_per_sec)` | `u32` burst capacity, `f64` tokens/second | Full bucket. `try_acquire(cost: f64)` consumes immediately or returns false. |
| `RetryBudget::new(capacity, fill_rate_per_sec)` | `u32` burst capacity, `f64` retry tokens/second | Full bucket. Each `try_acquire()` consumes one token or returns false. |

Neither type has default capacity/rate values. Refill happens lazily on calls
and inspectors, capped at capacity. `available_tokens()` and
`available_budget()` return fractional balances. A budget permits an initial
burst as well as refills; it is not a strict sliding-window requests/second cap.

`RateLimiter::acquire(cost)` retries after a computed deficit/refill delay of at
least 1 ms. With a non-positive refill rate it polls every 10 ms. It has no
timeout or fairness guarantee. Cost greater than capacity cannot be satisfied.

## Errors and validation

Constructors/builders return values directly, not `Result`, and do not validate
thresholds, capacities, costs, rates, or multipliers. Use positive thresholds,
positive capacity, finite non-negative refill rates, finite costs within capacity,
and finite non-negative multipliers/delays. Zero refill is useful for a fixed
budget, but exhausted blocking acquisition then cannot finish without external
cancellation. Invalid floating-point inputs can produce unusable state or a
panic during duration/jitter calculation. Denied admission/exhaustion is reported
with `bool`; this crate defines no error type.

## Environment and CLI ownership

This crate parses no environment variables, command-line flags, or YAML.
The CLI maps `RPC_CB_THRESHOLD` / `--rpc-cb-threshold` (5) and
`RPC_COOLDOWN_SECS` / `--rpc-cooldown-secs` (30 seconds) into gateway breaker
settings with success threshold 2. `RPC_TIMEOUT_SECS` / `--rpc-timeout-secs`
(10 seconds) configures the EVM HTTP client, not a resilience primitive.
SDK/manifest settings such as `indexing.rpcCbThreshold` belong to the surrounding
configuration tooling; passing a manifest to a primitive does nothing.

## Tests and coverage

From the repository root, with dependencies already cached:

```sh
cargo test -p logrix-resilience --offline --locked
```

`tests/resilience_tests.rs` covers deterministic backoff/capping, the default
retry boundary, builder options, budget exhaustion, token consumption, computed
waits, breaker transitions, manual controls, and probe limiting. It does not
establish jitter distribution, invalid-input handling, fairness, distributed
limits, or cancellation recovery for abandoned half-open probes.

## Related documentation

- [Configuration reference](../../docs/configuration.md)
- [Core errors and ports](../logrix-core/README.md)
- [EVM client](../logrix-chain-evm/README.md) and [RPC gateway](../logrix-rpc-gateway/README.md)
- [Resilience and reorgs](../../docs/03-resilience-reorgs.md)
