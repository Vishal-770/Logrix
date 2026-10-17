# logrix-testkit

In-memory test mocks and compliance test harness for Logrix indexers.

## Overview
Provides fast, zero-dependency in-memory implementations of all core ports (`MockQueuePort`, `MockChainPort`, `MockStorePort`) enabling sub-second test execution without Docker or external databases.

## Mock Components
- **`MockQueuePort`**: Thread-safe queue simulation with inspection hooks for acknowledged, negatively acknowledged, and dead-lettered messages.
- **`MockChainPort`**: Programmable mock EVM client returning synthetic blocks, transactions, and logs.
- **`MockStorePort`**: In-memory map storage simulating entity mutations, checkpointing, and atomic rollbacks.
