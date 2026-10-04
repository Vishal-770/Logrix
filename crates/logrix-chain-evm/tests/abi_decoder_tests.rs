use alloy_dyn_abi::DynSolValue;
use alloy_primitives::{address, b256, Bytes, B256, I256, U256};
use logrix_chain_evm::AbiEventDecoder;
use logrix_core::domain::EventLog;

#[test]
fn test_abi_decoder_erc20_transfer() {
    let mut decoder = AbiEventDecoder::new();
    let selector = decoder
        .add_event_signature("Transfer(address indexed from, address indexed to, uint256 value)")
        .expect("signature parse failed");

    assert_eq!(
        selector,
        b256!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
    );

    let from = address!("1111111111111111111111111111111111111111");
    let to = address!("2222222222222222222222222222222222222222");
    let mut t1 = [0u8; 32];
    t1[12..32].copy_from_slice(from.as_slice());
    let mut t2 = [0u8; 32];
    t2[12..32].copy_from_slice(to.as_slice());

    let amount = U256::from(5_000_000u64);
    let amount_bytes = amount.to_be_bytes::<32>();

    let log = EventLog {
        address: address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
        topics: vec![selector, B256::from(t1), B256::from(t2)],
        data: Bytes::from(amount_bytes.to_vec()),
        tx_hash: b256!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        log_index: 12,
        tx_index: 3,
        block_number: 19_000_000,
        block_hash: b256!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
    };

    let decoded = decoder.decode_log(&log).expect("should decode transfer");
    assert_eq!(decoded.event_name, "Transfer");
    assert_eq!(decoded.block_number, 19_000_000);
    assert_eq!(decoded.log_index, 12);
    assert_eq!(
        decoded
            .params
            .get("from")
            .unwrap()
            .as_str()
            .unwrap()
            .to_lowercase(),
        format!("{:#x}", from).to_lowercase()
    );
    assert_eq!(
        decoded
            .params
            .get("to")
            .unwrap()
            .as_str()
            .unwrap()
            .to_lowercase(),
        format!("{:#x}", to).to_lowercase()
    );
    assert_eq!(
        decoded.params.get("value").unwrap().as_str().unwrap(),
        "5000000"
    );
}

#[test]
fn test_abi_decoder_uniswap_v3_swap_signed_integers() {
    let mut decoder = AbiEventDecoder::new();
    let selector = decoder
        .add_event_signature(
            "Swap(address indexed sender, address indexed recipient, int256 amount0, int256 amount1, uint160 sqrtPriceX96, uint128 liquidity, int24 tick)",
        )
        .expect("uniswap swap sig parse failed");

    let sender = address!("e592427a0aece92de3edee1f18e0157c05861564");
    let recipient = address!("3333333333333333333333333333333333333333");
    let mut t1 = [0u8; 32];
    t1[12..32].copy_from_slice(sender.as_slice());
    let mut t2 = [0u8; 32];
    t2[12..32].copy_from_slice(recipient.as_slice());

    // Non-indexed data: (int256, int256, uint160, uint128, int24)
    let amount0 = I256::unchecked_from(-1_000_000_000);
    let amount1 = I256::unchecked_from(2_500_000_000i64);
    let sqrt_price = U256::from(1234567890u64);
    let liquidity = U256::from(9876543210u64);
    let tick = I256::unchecked_from(-198230);

    let tuple_val = DynSolValue::Tuple(vec![
        DynSolValue::Int(amount0, 256),
        DynSolValue::Int(amount1, 256),
        DynSolValue::Uint(sqrt_price, 160),
        DynSolValue::Uint(liquidity, 128),
        DynSolValue::Int(tick, 24),
    ]);
    let encoded_data = tuple_val.abi_encode();

    let log = EventLog {
        address: address!("88e6a0c2ddd26feeb64f039a2c41296fcb3f5640"),
        topics: vec![selector, B256::from(t1), B256::from(t2)],
        data: Bytes::from(encoded_data),
        tx_hash: B256::ZERO,
        log_index: 0,
        tx_index: 0,
        block_number: 18_000_000,
        block_hash: B256::ZERO,
    };

    let decoded = decoder
        .decode_log(&log)
        .expect("should decode Uniswap Swap");
    assert_eq!(decoded.event_name, "Swap");
    assert_eq!(
        decoded.params.get("amount0").unwrap().as_str().unwrap(),
        "-1000000000"
    );
    assert_eq!(
        decoded.params.get("amount1").unwrap().as_str().unwrap(),
        "2500000000"
    );
    assert_eq!(
        decoded.params.get("tick").unwrap().as_str().unwrap(),
        "-198230"
    );
    assert_eq!(
        decoded
            .params
            .get("sqrtPriceX96")
            .unwrap()
            .as_str()
            .unwrap(),
        "1234567890"
    );
}

#[test]
fn test_abi_decoder_full_json_abi() {
    let mut decoder = AbiEventDecoder::new();
    let json_abi = r#"[
      {
        "anonymous": false,
        "name": "OrderFulfilled",
        "type": "event",
        "inputs": [
          {"indexed": true, "name": "orderHash", "type": "bytes32"},
          {"indexed": true, "name": "offerer", "type": "address"},
          {"indexed": false, "name": "totalPrice", "type": "uint256"}
        ]
      },
      {
        "anonymous": false,
        "name": "Cancelled",
        "type": "event",
        "inputs": [
          {"indexed": true, "name": "orderHash", "type": "bytes32"}
        ]
      }
    ]"#;

    let count = decoder
        .add_abi_json(json_abi)
        .expect("JSON ABI parse failed");
    assert_eq!(count, 2);
    assert_eq!(decoder.len(), 2);
}
