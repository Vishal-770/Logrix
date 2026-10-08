use alloy_primitives::Address;
use logrix_blob_s3::{S3BlobStore, S3Config};
use logrix_core::{
    domain::{ChainId, QueueMessage, QueueType},
    ports::QueuePort,
};
use logrix_handlers::UserLogicEngine;
use logrix_reconciler::{GapReconciler, ReorgDetector, ReorgHandler};
use logrix_store_postgres::PostgresStore;
use logrix_webhook::{generate_secret, WebhookDispatcherService, WebhookStore};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

use crate::handlers::{handle_backfill_range, handle_live_block};

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub chain_id: ChainId,
    pub database_url: String,
    pub target_contract: Address,
    pub ring_buffer_depth: usize,
    pub webhook_url: Option<String>,
    pub webhook_secret: String,
    pub reconciler_interval_secs: u64,
    pub manifest_path: Option<String>,
    pub enable_webhooks: bool,
    pub s3_bucket: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_prefix: Option<String>,
}

impl ProcessorConfig {
    pub fn from_cli(cli: &crate::args::Cli, chain_id: ChainId, target_contract: Address) -> Self {
        Self {
            chain_id,
            database_url: cli.database_url.clone(),
            target_contract,
            ring_buffer_depth: cli.ring_buffer_depth,
            webhook_url: cli.webhook_url.clone(),
            webhook_secret: cli.webhook_secret.clone(),
            reconciler_interval_secs: cli.reconciler_interval_secs,
            manifest_path: cli.manifest_path.clone(),
            enable_webhooks: cli.enable_webhooks,
            s3_bucket: cli.s3_bucket.clone(),
            s3_endpoint: cli.s3_endpoint.clone(),
            s3_prefix: cli.s3_prefix.clone(),
        }
    }
}

pub struct ProcessorContext<'a> {
    pub config: &'a ProcessorConfig,
    pub store: &'a PostgresStore,
    pub blob_store: Option<&'a S3BlobStore>,
    pub detector: &'a ReorgDetector,
    pub handler: &'a ReorgHandler,
    pub gateway: &'a logrix_rpc_gateway::RpcGateway,
    pub queue: &'a Arc<dyn QueuePort>,
    pub engine: Option<&'a UserLogicEngine>,
}

pub async fn run_processor(
    config: ProcessorConfig,
    gateway: Arc<logrix_rpc_gateway::RpcGateway>,
    queue: Arc<dyn QueuePort>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Logrix Processor with Reorg Engine & Self-Healing Reconciler");
    let store = Arc::new(PostgresStore::connect(&config.database_url, "default").await?);
    let blob_store = if let Some(ref bucket) = config.s3_bucket {
        let s3_conf = S3Config::from_env(
            bucket,
            config.s3_prefix.clone(),
            config.s3_endpoint.as_deref(),
        )
        .await?;
        Some(S3BlobStore::new(s3_conf))
    } else {
        None
    };

    let detector = ReorgDetector::new(config.chain_id, gateway.clone(), config.ring_buffer_depth);
    let handler = ReorgHandler::new(
        config.chain_id,
        store.clone(),
        queue.clone(),
        detector.clone(),
    );
    let reconciler = GapReconciler::new(
        config.chain_id,
        store.clone(),
        queue.clone(),
        gateway.clone(),
        Duration::from_secs(config.reconciler_interval_secs),
    );
    tokio::spawn(async move {
        reconciler.run_loop().await;
    });

    if config.enable_webhooks {
        let wh_store = WebhookStore::new(store.pool().clone());
        let secret = if config.webhook_secret.is_empty() {
            generate_secret()
        } else {
            config.webhook_secret.clone()
        };
        let wh_dispatcher = WebhookDispatcherService::new(
            queue.clone(),
            Some(wh_store),
            config.webhook_url.clone(),
            secret,
        );
        tokio::spawn(async move {
            wh_dispatcher.run_loop().await;
        });
    }

    let engine_opt = if let Some(ref path) = config.manifest_path {
        let manifest = logrix_handlers::Manifest::from_file(path)?;
        let mut engine = UserLogicEngine::new(manifest)?;
        for contract in &engine.manifest().contracts.clone() {
            if let Some(ref wasm_path) = contract.wasm_handler {
                let resolved = if std::path::Path::new(wasm_path).exists() {
                    wasm_path.clone()
                } else if std::path::Path::new("/etc/logrix/handlers/mapping.wasm").exists() {
                    "/etc/logrix/handlers/mapping.wasm".to_string()
                } else {
                    wasm_path.clone()
                };
                engine.load_wasm_handler_file(contract.address, resolved)?;
            }
        }
        crate::warmup::prewarm_state_store(&store, &engine, config.chain_id.as_u64()).await;
        Some(engine)
    } else {
        None
    };

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
    info!("Starting processor consumer loop; listening for shutdown signals");
    loop {
        let msg_res = tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received; draining processor consumer loop");
                break;
            }
            res = ctx.queue.consume(QueueType::Backfill) => match res? {
                Some(h) => Some((QueueType::Backfill, h)),
                None => ctx
                    .queue
                    .consume(QueueType::Live)
                    .await?
                    .map(|h| (QueueType::Live, h)),
            }
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
            _ => {
                let _ = ctx.queue.ack(&handle).await;
            }
        }
    }
    info!("Processor consumer loop exited cleanly");
    Ok(())
}
