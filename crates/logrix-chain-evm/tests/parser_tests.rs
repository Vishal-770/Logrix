use alloy_primitives::{address, b256};
use logrix_chain_evm::parser::{parse_hex_u64, parse_required_hex_u64, try_parse_rpc_log};
use logrix_core::error::ErrorClass;
use serde_json::json;

#[test]
fn test_strict_hex_u64_parsing() {
    assert_eq!(
        parse_required_hex_u64(Some("0x10"), "blockNumber").unwrap(),
        16
    );
    assert_eq!(
        parse_required_hex_u64(Some("10"), "blockNumber").unwrap(),
        16
    );
    assert_eq!(parse_required_hex_u64(Some("0x0"), "logIndex").unwrap(), 0);

    let err_missing = parse_required_hex_u64(None, "blockNumber").unwrap_err();
    assert_eq!(err_missing.class, ErrorClass::Integrity);

    let err_invalid = parse_required_hex_u64(Some("0xzz"), "blockNumber").unwrap_err();
    assert_eq!(err_invalid.class, ErrorClass::Integrity);
}

#[test]
fn test_parse_hex_u64_fallback() {
    assert_eq!(parse_hex_u64(Some("0x20")), 32);
    assert_eq!(parse_hex_u64(Some("invalid")), 0);
    assert_eq!(parse_hex_u64(None), 0);
}

#[test]
fn test_strict_rpc_log_parsing_success() {
    let valid_log = json!({
        "address": "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d",
        "topics": [
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            "0x0000000000000000000000001111111111111111111111111111111111111111"
        ],
        "data": "0x00000000000000000000000000000000000000000000000000000000000f4240",
        "transactionHash": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "blockHash": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "blockNumber": "0x1000",
        "logIndex": "0x5",
        "transactionIndex": "0x2"
    });

    let log = try_parse_rpc_log(&valid_log).expect("valid RPC log should parse");
    assert_eq!(
        log.address,
        address!("75faf114eafb1bdbe2f0316df893fd58ce46aa4d")
    );
    assert_eq!(log.topics.len(), 2);
    assert_eq!(
        log.tx_hash,
        b256!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(
        log.block_hash,
        b256!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
    );
    assert_eq!(log.block_number, 4096);
    assert_eq!(log.log_index, 5);
    assert_eq!(log.tx_index, 2);
    assert_eq!(log.data.len(), 32);
}

#[test]
fn test_strict_rpc_log_rejects_missing_block_metadata() {
    let missing_block_num = json!({
        "address": "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d",
        "transactionHash": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "blockHash": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "logIndex": "0x5",
        "transactionIndex": "0x2"
    });
    let err = try_parse_rpc_log(&missing_block_num).unwrap_err();
    assert_eq!(err.class, ErrorClass::Integrity);
    assert!(err.message.contains("blockNumber"));

    let missing_tx_hash = json!({
        "address": "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d",
        "blockNumber": "0x1000",
        "blockHash": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "logIndex": "0x5",
        "transactionIndex": "0x2"
    });
    let err = try_parse_rpc_log(&missing_tx_hash).unwrap_err();
    assert_eq!(err.class, ErrorClass::Permanent);
    assert!(err.message.contains("transactionHash"));

    let missing_log_index = json!({
        "address": "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d",
        "transactionHash": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "blockHash": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "blockNumber": "0x1000",
        "transactionIndex": "0x2"
    });
    let err = try_parse_rpc_log(&missing_log_index).unwrap_err();
    assert_eq!(err.class, ErrorClass::Integrity);
    assert!(err.message.contains("logIndex"));
}

#[test]
fn test_strict_rpc_log_rejects_malformed_fields() {
    let malformed_addr = json!({
        "address": "not_an_address",
        "transactionHash": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "blockHash": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "blockNumber": "0x1000",
        "logIndex": "0x0",
        "transactionIndex": "0x0"
    });
    let err = try_parse_rpc_log(&malformed_addr).unwrap_err();
    assert_eq!(err.class, ErrorClass::Integrity);

    let malformed_data = json!({
        "address": "0x75faf114eafb1bdbe2f0316df893fd58ce46aa4d",
        "data": "0xinvalidhex",
        "transactionHash": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "blockHash": "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "blockNumber": "0x1000",
        "logIndex": "0x0",
        "transactionIndex": "0x0"
    });
    let err = try_parse_rpc_log(&malformed_data).unwrap_err();
    assert_eq!(err.class, ErrorClass::Integrity);
}
