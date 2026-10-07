# logrix-resilience

Fault-tolerance primitives for rate limiting, retry backoff, and circuit breaking in Logrix.

## Overview
Provides zero-allocation, thread-safe resilience utilities used across RPC clients, webhooks, and storage pipelines.

## Primitives
- **`CircuitBreaker`**: State machine (`Closed` -> `Open` -> `HalfOpen`) with consecutive failure counters, success thresholds, and cooldown durations.
- **`RateLimiter`**: Token bucket rate limiter supporting fractional burst replenishment.
- **`RetryPolicy`**: Full-jitter exponential backoff retry calculation to prevent thundering herd recovery storms.
