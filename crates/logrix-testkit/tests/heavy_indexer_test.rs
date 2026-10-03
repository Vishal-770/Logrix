use alloy_primitives::{address, b256, Bytes};
use logrix_core::{
    domain::{BlockEnvelope, ChainId, Checkpoint, EventLog, LiveBlockJob, QueueMessage, QueueType},
    ports::{ChainPort, QueuePort, StorePort},
};
use logrix_reconciler::{ContinuityStatus, ReorgDetector, ReorgHandler};
use logrix_testkit::{MockChainPort, MockQueuePort, MockStorePort};
use std::sync::Arc;

/// Heavy end-to-end stress test simulating a live running blockchain indexer:
/// - Ingestion pipeline producing blocks
/// - Processor pipeline consuming, detecting reorgs, handling gaps, and committing atomic checkpoints
/// - Chaos reorg injection: sudden fork at block 105 causing atomic rollback to block 104 and replay
#[tokio::test]
async fn test_heavy_indexer_pipeline_under_chaos_reorg() {
    let chain_id = ChainId::ARBITRUM_SEPOLIA;
    let chain = Arc::new(MockChainPort::new(chain_id));
    let queue = Arc::new(MockQueuePort::new());
    let store = Arc::new(MockStorePort::new());

    let target_contract = address!("75faf114eafb1BDbe2F0316DF893fd58CE46AA4d");

    // Pre-populate canonical chain blocks 101 to 105
    let mut prev_hash = b256!("0000000000000000000000000000000000000000000000000000000000000000");
    for b in 101..=105 {
        let block_hash = alloy_primitives::B256::repeat_byte(b as u8);
        let log = EventLog {
            address: target_contract,
            topics: vec![b256!(
                "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
            )],
            data: Bytes::from(vec![0x01, 0x02, 0x03]),
            tx_hash: alloy_primitives::B256::repeat_byte((b + 10) as u8),
            log_index: 0,
            tx_index: 0,
            block_number: b,
            block_hash,
        };
        let envelope = BlockEnvelope::new(
            chain_id,
            b,
            block_hash,
            prev_hash,
            1700000000 + b,
            vec![log],
        );
        chain.add_block(envelope).await;
        prev_hash = block_hash;
    }

    // Step 1: Ingest and process blocks 101 -> 105
    for b in 101..=105 {
        let envelope = chain
            .fetch_block_envelope(b, &[target_contract])
            .await
            .unwrap()
            .unwrap();
        let job = LiveBlockJob::new(chain_id, b, envelope.block_hash, envelope.parent_hash);
        queue
            .publish(QueueType::Live, &QueueMessage::LiveBlock(job))
            .await
            .unwrap();
    }

    assert_eq!(queue.depth(QueueType::Live).await.unwrap(), 5);

    let detector = Arc::new(ReorgDetector::new(chain_id, chain.clone(), 128));
    let handler = ReorgHandler::new(chain_id, store.clone(), queue.clone(), (*detector).clone());

    // Consume and process canonical blocks
    while let Some(handle) = queue.consume(QueueType::Live).await.unwrap() {
        if let QueueMessage::LiveBlock(job) = &handle.message {
            let env = chain
                .fetch_block_envelope(job.block_number, &[target_contract])
                .await
                .unwrap()
                .unwrap();
            let status = detector.check_envelope(&env).await.unwrap();
            assert_eq!(status, ContinuityStatus::Continuous);

            let cp = Checkpoint::new(chain_id, env.block_number, env.block_hash, true);
            store
                .write_events_and_checkpoint(&env.logs, &cp)
                .await
                .unwrap();
            queue.ack(&handle).await.unwrap();
        }
    }

    // Verify all 5 blocks processed
    assert_eq!(store.total_events().await, 5);
    let cp = store.get_checkpoint(chain_id).await.unwrap().unwrap();
    assert_eq!(cp.last_indexed_block, 105);

    // Step 2: Chaos Injection - Fork occurs at block 105!
    // Block 105 is mined on a competing branch with different hash and log payload
    let fork_hash = b256!("2222222222222222222222222222222222222222222222222222222222222222");
    let forked_log = EventLog {
        address: target_contract,
        topics: vec![b256!(
            "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
        )],
        data: Bytes::from(vec![0x09, 0x09]),
        tx_hash: b256!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        log_index: 0,
        tx_index: 0,
        block_number: 105,
        block_hash: fork_hash,
    };
    let parent_104_hash = alloy_primitives::B256::repeat_byte(104);
    let forked_envelope = BlockEnvelope::new(
        chain_id,
        105,
        fork_hash,
        parent_104_hash,
        1700000000 + 105,
        vec![forked_log],
    );
    chain.add_block(forked_envelope.clone()).await;

    // Detect reorg on the forked envelope
    let status = detector.check_envelope(&forked_envelope).await.unwrap();
    match status {
        ContinuityStatus::ReorgDetected {
            fork_block,
            fork_hash: detected_hash,
            reorg_depth,
        } => {
            assert_eq!(fork_block, 104);
            assert_eq!(reorg_depth, 1);
            // Execute atomic rollback
            handler
                .execute_rollback(fork_block, detected_hash, reorg_depth)
                .await
                .unwrap();
        }
        other => panic!("Expected ReorgDetected, got {:?}", other),
    }

    // Step 3: Verify state after atomic rollback
    let cp_post_reorg = store.get_checkpoint(chain_id).await.unwrap().unwrap();
    assert_eq!(cp_post_reorg.last_indexed_block, 104);
    assert_eq!(store.total_events().await, 4);

    // Step 4: Re-process the new canonical block 105
    let cp_replayed = Checkpoint::new(chain_id, 105, forked_envelope.block_hash, true);
    store
        .write_events_and_checkpoint(&forked_envelope.logs, &cp_replayed)
        .await
        .unwrap();

    let final_cp = store.get_checkpoint(chain_id).await.unwrap().unwrap();
    assert_eq!(final_cp.last_indexed_block, 105);
    assert_eq!(store.total_events().await, 5);
}
