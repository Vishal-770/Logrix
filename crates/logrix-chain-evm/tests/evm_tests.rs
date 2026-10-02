use alloy_primitives::{address, b256, Bytes, B256, U256};
use logrix_chain_evm::{decode_erc20_transfer, AdaptiveChunker, ERC20_TRANSFER_TOPIC};
use logrix_core::domain::EventLog;
use std::time::Duration;

#[test]
fn test_adaptive_chunker_aimd() {
    let chunker = AdaptiveChunker::new(500, 10, 2000);
    assert_eq!(chunker.chunk_size(), 500);

    // Multiplicative decrease on failure
    chunker.record_failure();
    assert_eq!(chunker.chunk_size(), 250);

    chunker.record_failure();
    assert_eq!(chunker.chunk_size(), 125);

    // Additive increase on 3 consecutive successes with good latency
    chunker.record_success(Duration::from_millis(200));
    chunker.record_success(Duration::from_millis(200));
    chunker.record_success(Duration::from_millis(200));
    assert_eq!(chunker.chunk_size(), 175); // 125 + 50 = 175
}

#[test]
fn test_adaptive_chunker_bounds() {
    let chunker = AdaptiveChunker::new(50, 10, 100);

    // Repeated failures shouldn't drop below min_size
    for _ in 0..10 {
        chunker.record_failure();
    }
    assert_eq!(chunker.chunk_size(), 10);

    // Repeated successes shouldn't exceed max_size
    for _ in 0..30 {
        chunker.record_success(Duration::from_millis(100));
    }
    assert_eq!(chunker.chunk_size(), 100);
}

#[test]
fn test_decode_valid_erc20_transfer() {
    let contract = address!("75faf114eafb1BDbe2F0316DF893fd58CE46AA4d");
    let from = address!("1111111111111111111111111111111111111111");
    let to = address!("2222222222222222222222222222222222222222");

    // Pad addresses to 32 bytes for topics
    let mut topic1 = [0u8; 32];
    topic1[12..32].copy_from_slice(from.as_slice());
    let mut topic2 = [0u8; 32];
    topic2[12..32].copy_from_slice(to.as_slice());

    // Amount: 1,000,000 (1 USDC, 6 decimals)
    let amount = U256::from(1_000_000u64);
    let amount_bytes = amount.to_be_bytes::<32>();

    let log = EventLog {
        address: contract,
        topics: vec![ERC20_TRANSFER_TOPIC, B256::from(topic1), B256::from(topic2)],
        data: Bytes::copy_from_slice(&amount_bytes),
        tx_hash: b256!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        log_index: 3,
        tx_index: 1,
        block_number: 1234567,
        block_hash: b256!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    };

    let decoded = decode_erc20_transfer(421614, &log).expect("should decode transfer");

    assert_eq!(decoded.chain_id, 421614);
    assert_eq!(decoded.block_number, 1234567);
    assert_eq!(decoded.log_index, 3);
    assert_eq!(decoded.amount, "1000000");
    assert_eq!(
        decoded.from_address.to_lowercase(),
        format!("{:#x}", from).to_lowercase()
    );
    assert_eq!(
        decoded.to_address.to_lowercase(),
        format!("{:#x}", to).to_lowercase()
    );
}

#[test]
fn test_decode_non_transfer_event() {
    let dummy_topic = b256!("1111111111111111111111111111111111111111111111111111111111111111");
    let log = EventLog {
        address: address!("75faf114eafb1BDbe2F0316DF893fd58CE46AA4d"),
        topics: vec![dummy_topic],
        data: Bytes::from(vec![0x00]),
        tx_hash: B256::ZERO,
        log_index: 0,
        tx_index: 0,
        block_number: 100,
        block_hash: B256::ZERO,
    };

    assert!(decode_erc20_transfer(421614, &log).is_none());
}
