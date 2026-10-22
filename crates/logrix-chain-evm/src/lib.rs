//! EVM Chain JSON-RPC client adapter implementing `ChainPort`.
//!
//! Provides adaptive AIMD block chunking, token bucket rate limiting,
//! exponential backoff, and ERC-20 event decoding.

pub mod batch;
pub mod chain_port;
pub mod chunker;
pub mod client;
pub mod config;
pub mod decoder;
pub mod events;
pub mod hex_util;
pub mod parser;
pub mod rpc_call;

pub use chunker::AdaptiveChunker;
pub use client::EvmChainClient;
pub use config::{
    EvmClientConfig, DEFAULT_CHUNK_ADDITIVE_STEP, DEFAULT_CHUNK_INITIAL, DEFAULT_CHUNK_MAX,
    DEFAULT_CHUNK_MIN, DEFAULT_CHUNK_SUCCESS_THRESHOLD, DEFAULT_CHUNK_TARGET_LATENCY,
    DEFAULT_MAX_RETRIES, DEFAULT_POOL_IDLE_TIMEOUT, DEFAULT_POOL_MAX_IDLE_PER_HOST,
    DEFAULT_RATE_LIMIT_CAPACITY, DEFAULT_RATE_LIMIT_REFILL_RATE, DEFAULT_RETRY_INITIAL_BACKOFF,
    DEFAULT_RETRY_MAX_BACKOFF, DEFAULT_TIMEOUT,
};
pub use decoder::{decode_erc20_transfer, decode_erc20_transfer_with_time, AbiEventDecoder};
pub use events::{DecodedEvent, DecodedTransfer, ERC20_TRANSFER_TOPIC};
