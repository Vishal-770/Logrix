//! EVM Chain JSON-RPC client adapter implementing `ChainPort`.
//!
//! Provides adaptive AIMD block chunking, token bucket rate limiting,
//! exponential backoff, and ERC-20 event decoding.

pub mod batch;
pub mod chain_port;
pub mod chunker;
pub mod client;
pub mod decoder;
pub mod parser;

pub use chunker::AdaptiveChunker;
pub use client::EvmChainClient;
pub use decoder::{
    decode_erc20_transfer, AbiEventDecoder, DecodedEvent, DecodedTransfer, ERC20_TRANSFER_TOPIC,
};
