use alloy_primitives::B256;
use logrix_core::domain::{BlockEnvelope, ChainId};
use logrix_reconciler::{ContinuityStatus, ReorgDetector, RollingBlockBuffer};
use logrix_testkit::MockChainPort;
use std::sync::Arc;

#[tokio::test]
async fn test_rolling_buffer_parent_continuity_and_rewind() {
    let mut buf = RollingBlockBuffer::new(5);

    let h1 = B256::repeat_byte(0x01);
    let h2 = B256::repeat_byte(0x02);
    let h3 = B256::repeat_byte(0x03);

    buf.push(logrix_core::domain::BlockRef::new(100, h1));
    buf.push(logrix_core::domain::BlockRef::new(101, h2));
    buf.push(logrix_core::domain::BlockRef::new(102, h3));

    assert_eq!(buf.len(), 3);
    assert_eq!(buf.latest().unwrap().number, 102);

    assert!(buf.is_parent_matching(103, h3));
    assert!(!buf.is_parent_matching(103, h1));

    buf.rewind_to(101);
    assert_eq!(buf.len(), 2);
    assert_eq!(buf.latest().unwrap().number, 101);
}

#[tokio::test]
async fn test_reorg_detector_catches_fork_and_ancestor() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    let h100 = B256::repeat_byte(0x10);
    let h101 = B256::repeat_byte(0x11);
    let h102 = B256::repeat_byte(0x12);

    let detector = ReorgDetector::new(chain_id, mock_chain.clone(), 32);

    let b100 = BlockEnvelope::new(chain_id, 100, h100, B256::ZERO, 1000, vec![]);
    let b101 = BlockEnvelope::new(chain_id, 101, h101, h100, 1010, vec![]);
    let b102 = BlockEnvelope::new(chain_id, 102, h102, h101, 1020, vec![]);

    assert_eq!(
        detector.check_envelope(&b100).await.unwrap(),
        ContinuityStatus::Continuous
    );
    assert_eq!(
        detector.check_envelope(&b101).await.unwrap(),
        ContinuityStatus::Continuous
    );
    assert_eq!(
        detector.check_envelope(&b102).await.unwrap(),
        ContinuityStatus::Continuous
    );

    let h102_fork = B256::repeat_byte(0x99);
    let b102_fork = BlockEnvelope::new(chain_id, 102, h102_fork, h101, 1025, vec![]);

    let status = detector.check_envelope(&b102_fork).await.unwrap();
    match status {
        ContinuityStatus::ReorgDetected {
            fork_block,
            fork_hash,
            reorg_depth,
        } => {
            assert_eq!(fork_block, 101);
            assert_eq!(fork_hash, h101);
            assert_eq!(reorg_depth, 1);
        }
        other => panic!("Expected ReorgDetected, got {:?}", other),
    }
}

#[tokio::test]
async fn test_reorg_detector_gap_and_duplicate_handling() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let mock_chain = Arc::new(MockChainPort::new(chain_id));

    let h100 = B256::repeat_byte(0x10);
    let h101 = B256::repeat_byte(0x11);
    let h105 = B256::repeat_byte(0x15);

    let detector = ReorgDetector::new(chain_id, mock_chain.clone(), 32);

    let b100 = BlockEnvelope::new(chain_id, 100, h100, B256::ZERO, 1000, vec![]);
    let b101 = BlockEnvelope::new(chain_id, 101, h101, h100, 1010, vec![]);
    let b105 = BlockEnvelope::new(chain_id, 105, h105, B256::repeat_byte(0x14), 1050, vec![]);

    assert_eq!(
        detector.check_envelope(&b100).await.unwrap(),
        ContinuityStatus::Continuous
    );
    assert_eq!(
        detector.check_envelope(&b101).await.unwrap(),
        ContinuityStatus::Continuous
    );

    let status = detector.check_envelope(&b105).await.unwrap();
    assert_eq!(
        status,
        ContinuityStatus::GapDetected {
            from_block: 102,
            to_block: 104
        }
    );

    assert_eq!(
        detector.check_envelope(&b101).await.unwrap(),
        ContinuityStatus::Continuous
    );

    let b101_divergent =
        BlockEnvelope::new(chain_id, 101, B256::repeat_byte(0x77), h100, 1011, vec![]);
    let status_reorg = detector.check_envelope(&b101_divergent).await.unwrap();
    match status_reorg {
        ContinuityStatus::ReorgDetected { fork_block, .. } => {
            assert_eq!(fork_block, 100);
        }
        other => panic!("Expected ReorgDetected, got {:?}", other),
    }
}
