use alloy_primitives::B256;
use logrix_core::domain::{BlockEnvelope, ChainId};
use logrix_reconciler::ReorgDetector;
use logrix_testkit::MockChainPort;
use std::sync::Arc;

#[tokio::test]
async fn test_reorg_detector_max_depth_limit_guard() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    let detector = ReorgDetector::new(chain_id, mock_chain.clone(), 32).with_max_reorg_depth(2);

    let h10 = B256::repeat_byte(0x10);
    let h11 = B256::repeat_byte(0x11);
    let h12 = B256::repeat_byte(0x12);
    let h13 = B256::repeat_byte(0x13);

    detector
        .check_envelope(&BlockEnvelope::new(
            chain_id,
            10,
            h10,
            B256::ZERO,
            100,
            vec![],
        ))
        .await
        .unwrap();
    detector
        .check_envelope(&BlockEnvelope::new(chain_id, 11, h11, h10, 110, vec![]))
        .await
        .unwrap();
    detector
        .check_envelope(&BlockEnvelope::new(chain_id, 12, h12, h11, 120, vec![]))
        .await
        .unwrap();
    detector
        .check_envelope(&BlockEnvelope::new(chain_id, 13, h13, h12, 130, vec![]))
        .await
        .unwrap();

    let divergent_parent = B256::repeat_byte(0x99);

    mock_chain
        .add_block(BlockEnvelope::new(
            chain_id,
            13,
            divergent_parent,
            B256::repeat_byte(0x83),
            130,
            vec![],
        ))
        .await;
    mock_chain
        .add_block(BlockEnvelope::new(
            chain_id,
            12,
            B256::repeat_byte(0x83),
            B256::repeat_byte(0x82),
            120,
            vec![],
        ))
        .await;
    mock_chain
        .add_block(BlockEnvelope::new(
            chain_id,
            11,
            B256::repeat_byte(0x82),
            B256::repeat_byte(0x81),
            110,
            vec![],
        ))
        .await;
    let b14_fork = BlockEnvelope::new(
        chain_id,
        14,
        B256::repeat_byte(0x98),
        divergent_parent,
        140,
        vec![],
    );

    let res = detector.check_envelope(&b14_fork).await;
    assert!(res.is_err());
    let err_msg = res.err().unwrap().to_string();
    assert!(err_msg.contains("max depth limit") || err_msg.contains("exceeded"));
}
