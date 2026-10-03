use alloy_primitives::Address;
use logrix_blob_s3::{S3BlobStore, S3Config};
use logrix_core::{
    domain::{BlockRangeJob, ChainId, MessageHandle, QueueMessage, QueueType},
    ports::ChainPort,
    ports::QueuePort,
};
use logrix_handlers::UserLogicEngine;
use logrix_reconciler::{ContinuityStatus, GapReconciler, ReorgDetector, ReorgHandler, WebhookDispatcher};
use logrix_store_postgres::PostgresStore;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

use crate::indexer::{index_envelope, index_logs_batch};

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub chain_id: ChainId,
    pub database_url: String,
    pub target_contract: Address,
    pub ring_buffer_depth: usize,
    pub webhook_url: Option<String>,
    pub reconciler_interval_secs: u64,
    pub manifest_path: Option<String>,
    pub enable_webhooks: bool,
    pub s3_bucket: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_prefix: Option<String>,
}

struct ProcessorContext<'a> {
    config: &'a ProcessorConfig,
    store: &'a PostgresStore,
    blob_store: Option<&'a S3BlobStore>,
    detector: &'a ReorgDetector,
    handler: &'a ReorgHandler,
    gateway: &'a logrix_rpc_gateway::RpcGateway,
    queue: &'a Arc<dyn QueuePort>,
    engine: Option<&'a UserLogicEngine>,
}

pub async fn run_processor(
    config: ProcessorConfig,
    gateway: Arc<logrix_rpc_gateway::RpcGateway>,
    queue: Arc<dyn QueuePort>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Logrix Processor with Reorg Engine & Self-Healing Reconciler");
    let store = Arc::new(PostgresStore::connect(&config.database_url, "default").await?);
    let blob_store = if let Some(ref bucket) = config.s3_bucket {
        let s3_conf = S3Config::from_env(bucket, config.s3_prefix.clone(), config.s3_endpoint.as_deref()).await?;
        Some(S3BlobStore::new(s3_conf))
    } else {
        None
    };

    let detector = ReorgDetector::new(config.chain_id, gateway.clone(), config.ring_buffer_depth);
    let handler = ReorgHandler::new(config.chain_id, store.clone(), queue.clone(), detector.clone());
    let reconciler = GapReconciler::new(config.chain_id, store.clone(), queue.clone(), gateway.clone(), Duration::from_secs(config.reconciler_interval_secs));
    let webhook_dispatcher = WebhookDispatcher::new(queue.clone(), config.webhook_url.clone());

    let engine_opt = if let Some(ref path) = config.manifest_path {
        let manifest = logrix_handlers::Manifest::from_file(path)?;
        let mut engine = UserLogicEngine::new(manifest)?;
        for contract in &engine.manifest().contracts.clone() {
            if let Some(ref wasm_path) = contract.wasm_handler {
                engine.load_wasm_handler_file(contract.address, wasm_path)?;
            }
        }
        Some(engine)
    } else {
        None
    };

    tokio::spawn(async move { reconciler.run_loop().await; });
    if config.enable_webhooks {
        tokio::spawn(async move { webhook_dispatcher.run_loop().await; });
    }

    let ctx = ProcessorContext {
        config: &config,
        store: &store,
        blob_store: blob_store.as_ref(),
        detector: &detector,
        handler: &handler,
        gateway: &gateway,
        queue: &queue,
        engine: engine_opt.as_ref(),
    };

    run_consumer_loop(&ctx).await
}

async fn run_consumer_loop(ctx: &ProcessorContext<'_>) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let msg_res = match ctx.queue.consume(QueueType::Backfill).await? {
            Some(h) => Some((QueueType::Backfill, h)),
            None => ctx.queue.consume(QueueType::Live).await?.map(|h| (QueueType::Live, h)),
        };

        let Some((_q_type, handle)) = msg_res else {
            tokio::time::sleep(Duration::from_millis(200)).await;
            continue;
        };

        match &handle.message {
            QueueMessage::LiveBlock(job) => {
                handle_live_block(ctx, job.block_number, &handle).await?;
            }
            QueueMessage::BackfillRange(job) => {
                handle_backfill_range(ctx, job, &handle).await?;
            }
            _ => { let _ = ctx.queue.ack(&handle).await; }
        }
    }
}

async fn handle_live_block(
    ctx: &ProcessorContext<'_>,
    block_num: u64,
    handle: &MessageHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let Ok(Some(env)) = ctx.gateway.fetch_block_envelope(block_num, &[ctx.config.target_contract]).await else {
        let _ = ctx.queue.nack(handle, true).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        return Ok(());
    };

    match ctx.detector.check_envelope(&env).await {
        Ok(ContinuityStatus::Continuous) => {
            if index_envelope(ctx.store, ctx.blob_store, ctx.config.chain_id.as_u64(), &env, ctx.engine).await.is_ok() {
                let _ = ctx.queue.ack(handle).await;
            } else {
                let _ = ctx.queue.nack(handle, true).await;
            }
        }
        Ok(ContinuityStatus::ReorgDetected { fork_block, fork_hash, reorg_depth }) => {
            warn!(fork_block, fork_hash = %fork_hash, reorg_depth, "Reorg detected! Executing atomic rollback");
            if ctx.handler.execute_rollback(fork_block, fork_hash, reorg_depth).await.is_ok() {
                if fork_block == env.block_number - 1 {
                    ctx.detector.buffer().write().await.push(env.block_ref());
                    let _ = index_envelope(ctx.store, ctx.blob_store, ctx.config.chain_id.as_u64(), &env, ctx.engine).await;
                } else {
                    let gap = BlockRangeJob::new(ctx.config.chain_id, fork_block + 1, env.block_number, format!("{}:{}-{}", ctx.config.chain_id, fork_block + 1, env.block_number));
                    let _ = ctx.queue.publish(QueueType::Backfill, &QueueMessage::BackfillRange(gap)).await;
                }
                let _ = ctx.queue.ack(handle).await;
            } else {
                let _ = ctx.queue.nack(handle, true).await;
            }
        }
        Ok(ContinuityStatus::GapDetected { from_block, to_block }) => {
            let gap = BlockRangeJob::new(ctx.config.chain_id, from_block, to_block, format!("{}:{from_block}-{to_block}", ctx.config.chain_id));
            let _ = ctx.queue.publish(QueueType::Backfill, &QueueMessage::BackfillRange(gap)).await;
            let _ = ctx.queue.nack(handle, true).await;
        }
        Err(_) => { let _ = ctx.queue.nack(handle, true).await; }
    }
    Ok(())
}

async fn handle_backfill_range(
    ctx: &ProcessorContext<'_>,
    job: &BlockRangeJob,
    handle: &MessageHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    match ctx.gateway.fetch_logs(job.from_block, job.to_block, &[ctx.config.target_contract]).await {
        Ok(logs) => {
            if index_logs_batch(ctx.store, ctx.config.chain_id, job.to_block, &logs, ctx.engine).await.is_ok() {
                let _ = ctx.queue.ack(handle).await;
            } else {
                let _ = ctx.queue.nack(handle, true).await;
            }
        }
        Err(e) => {
            let err = e.to_string();
            if err.contains("more than") || err.contains("limit") || err.contains("range") || err.contains("exceeded") || err.contains("10000 results") || err.contains("413") {
                let (first, second_opt) = job.split_half();
                warn!(from = job.from_block, to = job.to_block, "RPC range limit exceeded; halving range: {}..={}", first.from_block, first.to_block);
                let _ = ctx.queue.publish(QueueType::Backfill, &QueueMessage::BackfillRange(first)).await;
                if let Some(second) = second_opt {
                    let _ = ctx.queue.publish(QueueType::Backfill, &QueueMessage::BackfillRange(second)).await;
                }
                let _ = ctx.queue.ack(handle).await;
            } else {
                let _ = ctx.queue.nack(handle, true).await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
    Ok(())
}
