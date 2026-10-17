# Circuit Breaker Subsystem

Three-state circuit breaker designed to isolate failing external services, prevent cascading timeouts, and control recovery probing.

---

## State Model

```
             +--------------------+
             |       Closed       |<------------------+
             +--------------------+                   |
               |                ^                     |
  failures >=  |                | success_count >=    |
  threshold    |                | success_threshold   |
               v                |                     |
             +--------------------+                   |
             |        Open        |                   |
             +--------------------+                   |
               |                                      |
  reset_timeout|                                      |
  elapsed      v                                      |
             +--------------------+                   |
             |      HalfOpen      |-------------------+
             +--------------------+
               |
  any failure  |
               +------------------> (Trips back to Open)
```

- **`Closed`**: Standard operational state. Failures increment `failure_count`. If `failure_count >= failure_threshold`, the circuit trips to `Open`.
- **`Open`**: Outage isolation state. Inbound calls are immediately rejected (`can_execute() == false`) without invoking network resources until `reset_timeout` elapses.
- **`HalfOpen`**: Recovery evaluation state. Allows a controlled number of probe requests (`half_open_max_probes`). Achieving `success_threshold` consecutive successes resets the circuit to `Closed`. Any failure immediately trips the circuit back to `Open`.

---

## API Reference & Usage

### 1. `CircuitConfig`

Builder configuration for failure thresholds, timeouts, probe limits, and decay windows.

| Setting | Type | Default | Description |
|---|---|---|---|
| `failure_threshold` | `u32` | `5` | Consecutive failures before tripping to `Open`. |
| `success_threshold` | `u32` | `2` | Consecutive successes in `HalfOpen` required to close the circuit. |
| `reset_timeout` | `Duration` | `30s` | Outage cooldown period before probing recovery in `HalfOpen`. |
| `half_open_max_probes` | `Option<u32>` | `None` (`success_threshold`) | Maximum concurrent exploratory probes allowed in `HalfOpen`. |
| `failure_window` | `Option<Duration>` | `None` | Sliding window after which historical failures decay back to zero. |

---

### 2. `CircuitBreaker`

```rust
use logrix_resilience::{CircuitBreaker, CircuitConfig};
use std::time::Duration;

let config = CircuitConfig::builder()
    .failure_threshold(5)
    .success_threshold(2)
    .reset_timeout(Duration::from_secs(30))
    .half_open_max_probes(1)
    .failure_window(Duration::from_secs(60))
    .build();

let breaker = CircuitBreaker::with_config(config);

// Synchronous permission check:
if breaker.can_execute() {
    // Perform outbound network call...
    breaker.on_success();
} else {
    // Circuit is open; route to backup provider or reject
}
```

---

### 3. `CircuitGuard` (RAII Execution Permit)

Guarantees probe tracking consistency during asynchronous cancellations, timeouts, or task drops. If an in-flight operation drops without calling `record_success()` or `finish()`, the permit automatically registers a failure.

```rust
if let Some(guard) = breaker.try_acquire() {
    let result = make_network_call().await;
    guard.finish(result.is_ok());
}
```
