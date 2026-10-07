# logrix-chain-evm

EVM client implementation for log extraction, chunking, and JSON-RPC decoding.

## Overview
Connects to EVM execution nodes and archive nodes to extract logs matching indexed contracts.

## Features
- **Adaptive AIMD Chunker (`chunker.rs`)**: Dynamically expands and halves `eth_getLogs` block query ranges in response to node response size limits.
- **Token Bucket Rate Limiter**: Client-side rate limiting to stay within provider request quotas.
- **Exponential Retry Policy**: Automatic retry backoff on transient network dropouts and 429 rate limit errors.
