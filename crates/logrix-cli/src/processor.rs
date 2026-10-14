use logrix_blob_s3::{S3BlobStore, S3Config};
use logrix_core::{
    domain::{QueueMessage, QueueType},
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

pub use crate::processor_types::*;

pub async fn run_processor(
    config: ProcessorConfig,
    gateway: Arc<logrix_rpc_gateway::RpcGateway>,
    queue: Arc<dyn QueuePort>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Logrix Processor with Reorg Engine & Self-Healing Reconciler");
    let pg_cfg = logrix_store_postgres::PostgresConfig::new(&config.database_url, "default")
        .max_connections(config.database_max_connections)
        .min_connections(config.database_min_connections);
    let store = Arc::new(PostgresStore::connect_with_config(&pg_cfg).await?);
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
    let mut reconciler = GapReconciler::new(
        config.chain_id,
        store.clone(),
        queue.clone(),
        gateway.clone(),
        Duration::from_secs(config.reconciler_interval_secs),
    )
    .with_lag_threshold(config.reconciler_lag_threshold);
    if let Some(cs) = config.reconciler_chunk_size {
        reconciler = reconciler.with_chunk_size(cs);
    }
    let (shutdown_tx, shutdown_rx) = crate::shutdown::shutdown_channel();
    let reconciler_rx = shutdown_rx.clone();
    tokio::spawn(async move {
        reconciler
            .run_loop_with_shutdown(crate::shutdown::wait_shutdown(reconciler_rx))
            .await;
    });

    if config.enable_webhooks {
        let wh_store = WebhookStore::new(store.pool().clone());
        let secret = if config.webhook_secret.is_empty() {
            generate_secret()
        } else {
            config.webhook_secret.clone()
        };
        let wh_cfg = logrix_webhook::WebhookConfig::new(
            Duration::from_secs(config.webhook_timeout_secs),
            config.webhook_max_retries,
            Duration::from_millis(config.webhook_worker_idle_ms),
            Duration::from_millis(config.webhook_worker_backoff_ms),
        );
        let wh_dispatcher = WebhookDispatcherService::new_with_config(
            queue.clone(),
            Some(wh_store),
            config.webhook_url.clone(),
            secret,
            wh_cfg,
        );
        let wh_rx = shutdown_rx.clone();
        tokio::spawn(async move {
            wh_dispatcher
                .run_loop_with_shutdown(crate::shutdown::wait_shutdown(wh_rx))
                .await;
        });
    }

    let engine_opt = if let Some(ref path) = config.manifest_path {
        let manifest = logrix_handlers::Manifest::from_file(path)?;
        let mut engine = UserLogicEngine::new(manifest)?
            .with_global_limits(config.wasm_fuel_limit, config.wasm_max_memory_mb);
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

    run_consumer_loop(&ctx, shutdown_tx).await
}

async fn run_consumer_loop(
    ctx: &ProcessorContext<'_>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting processor consumer loop; listening for shutdown signals");
    let mut backoff = Duration::from_millis(500);
    let max_backoff = Duration::from_secs(5);

    loop {
        let msg_res = tokio::select! {
            _ = crate::shutdown::wait_for_shutdown_signal() => {
                info!("Shutdown signal received; draining processor consumer loop");
                let _ = shutdown_tx.send(true);
                break;
            }
            res = ctx.queue.consume(QueueType::Backfill) => match res {
                Ok(Some(h)) => {
                    backoff = Duration::from_millis(500);
                    Some((QueueType::Backfill, h))
                }
                Ok(None) => match ctx.queue.consume(QueueType::Live).await {
                    Ok(opt) => {
                        backoff = Duration::from_millis(500);
                        opt.map(|h| (QueueType::Live, h))
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Error consuming from live queue; backing off");
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(max_backoff);
                        None
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "Error consuming from backfill queue; backing off");
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(max_backoff);
                    None
                }
            }
        };

        let Some((_q_type, handle)) = msg_res else {
            tokio::time::sleep(Duration::from_millis(100)).await;
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
    let _ = shutdown_tx.send(true);
    info!("Processor consumer loop exited cleanly");
    Ok(())
}
