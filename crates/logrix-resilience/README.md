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
