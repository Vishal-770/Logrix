# logrix-testkit

In-memory test mocks and compliance test harness for Logrix indexers.

## Overview
Provides fast, zero-dependency in-memory implementations of all core ports (`MockQueuePort`, `MockChainPort`, `MockStorePort`) enabling sub-second test execution without Docker or external databases.

## Mock Components
- **`MockQueuePort`**: Thread-safe queue simulation with inspection hooks for acknowledged, negatively acknowledged, and dead-lettered messages, configurable visibility timeouts (`set_visibility_timeout`), backpressure/bounded capacity (`set_max_capacity`), simulated latency (`set_simulated_latency`), and transient error fault injection (`inject_transient_errors`).
- **`MockChainPort`**: Programmable mock EVM client returning synthetic blocks, transactions, and logs, with artificial latency simulation and transient RPC network dropout injection.
- **`MockStorePort`**: In-memory map storage simulating entity mutations, checkpointing, and atomic rollbacks, with latency simulation and database lock contention injection.

## Simulation & Fault Injection
- **Transient Network Drops**: Inject exact counts of transient RPC or broker errors to verify circuit breakers and retry policies.
- **Visibility Timeout Expiry**: Test queue message recovery when consumer tasks crash without acknowledging.
- **Backpressure & Bounded Buffers**: Test pipeline behavior when queue buffers reach capacity limits.
