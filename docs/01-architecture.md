# Logrix Architecture & Pipeline Anatomy

## Overview
Logrix is architected around the **Hexagonal / Ports-and-Adapters** pattern. The engine core contains zero vendor-specific, cloud-specific, or chain-specific assumptions. All external interactions are abstracted behind stable Rust traits.

```text
              ┌────────────────────────────────────────────────────────┐
              │                      LOGRIX CORE                       │
              │                                                        │
              │   Source Port ──► Filter ──► Decode ──► Handle ──► Sink │
              │        │                                           │   │
              └────────┼───────────────────────────────────────────┼───┘
                       │                                           │
         ┌─────────────┴─────────────┐               ┌─────────────┴─────────────┐
         ▼                           ▼               ▼                           ▼
  logrix-chain-evm            Bulk Providers   logrix-store-postgres       Webhooks
  (JSON-RPC Gateway)          (SQD/HyperSync)  (or SQLite / ClickHouse)    (Signed HTTP)
```

## The 5 Pipeline Stages

Every block job moves through five discrete pipeline stages:

1. **Source Port (`LiveSource` / `HistoricalSource`):**
   - Obtains raw block envelopes (`chain_id`, `block_ref`, `raw_logs`, `timestamp`).
   - Default: EVM JSON-RPC Gateway.
   - Future: Reth ExEx, SQD Portal, HyperSync, non-EVM adapters.

2. **Filter Stage:**
   - Filters events before decoding based on contract addresses and event topic signatures.
   - Allows custom WASM filters (e.g. "only decode Swaps where amountInUSD > $10,000").

3. **Decode Stage:**
   - Uses `alloy-dyn-abi` to decode hex event data and indexed topics against user-provided ABIs.
   - Decodes at runtime with zero recompilation needed when users supply new ABI JSON files.

4. **Handle Stage:**
   - Executes the user's business logic.
   - Supports 4 modes:
     * **Declarative:** YAML-based column mapping (fastest, zero code).
     * **WASM Sandboxed:** Compiled WASM binary (written in Rust, TypeScript, Go, Zig) running in `wasmtime` with memory/time limits.
     * **TypeScript:** Sandboxed JS-in-WASM runtime for familiar frontend/backend JS syntax.
     * **Remote gRPC:** Streams decoded events to an external microservice and writes back the mutations.

5. **Sink Port:**
   - Commits transformed event rows and advances the chain checkpoint in a single ACID database transaction.
   - Dispatches outgoing signed webhooks.

## The Extension Model: 4 Mechanisms

| Mechanism | Who Writes It | Isolation | Speed | Rebuild Core? | Best For |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Rust Crates** | Core / Contributors | In-process | Native | Yes (`--features`) | Official high-speed adapters |
| **WASM Plugins** | Any User | Memory & CPU sandbox | Fast | **No** | User handlers, transforms, custom filters |
| **gRPC Plugins** | Any User | Process-isolated | RPC network | **No** | Heavy services, adapters in Python/Go/Node |
| **Pipeline Hooks** | Any User | Configuration | Fast | **No** | Auditing, metrics, external notifications |
