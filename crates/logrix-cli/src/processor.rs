use logrix_core::ports::QueuePort;
use logrix_reconciler::{GapReconciler, ReorgDetector, ReorgHandler};
use logrix_store_postgres::PostgresStore;
use logrix_webhook::{generate_secret, WebhookDispatcherService, WebhookStore};
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

pub use crate::processor_types::*;

type DynError = Box<dyn std::error::Error + Send + Sync>;

pub async fn run_processor(
    config: ProcessorConfig,
    gateway: Arc<logrix_rpc_gateway::RpcGateway>,
    queue: Arc<dyn QueuePort>,
) -> Result<(), DynError> {
    info!("Starting Logrix Processor with Reorg Engine & Self-Healing Reconciler");
    let pg_cfg = logrix_store_postgres::PostgresConfig::new(&config.database_url, "default")
        .max_connections(config.database_max_connections)
        .min_connections(config.database_min_connections)
        .statement_timeout(Duration::from_secs(config.database_statement_timeout_secs));
    let store = Arc::new(PostgresStore::connect_with_config(&pg_cfg).await?);
    let blob_store = config
        .init_blob_store()
        .await
        .map_err(|e| Box::new(e) as DynError)?;

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
    let reconciler_task = tokio::spawn(async move {
        reconciler
            .run_loop_with_shutdown(crate::shutdown::wait_shutdown(reconciler_rx))
            .await;
    });

    let mut webhook_task = if config.enable_webhooks {
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
        Some(tokio::spawn(async move {
            wh_dispatcher
                .run_loop_with_shutdown(crate::shutdown::wait_shutdown(wh_rx))
                .await;
        }))
    } else {
        None
    };

    let engine_opt = load_user_engine(&config, &store)
        .await
        .map_err(|e| Box::new(std::io::Error::other(e.to_string())) as DynError)?;

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

    let run_res = crate::processor_loop::run_consumer_loop(&ctx, shutdown_tx).await;

    // Gracefully join reconciler and webhook tasks with drain timeout
    let drain_timeout = Duration::from_secs(5);
    let _ = tokio::time::timeout(drain_timeout, async {
        let _ = reconciler_task.await;
        if let Some(wh) = webhook_task.as_mut() {
            let _ = wh.await;
        }
    })
    .await;

    run_res.map_err(|e| Box::new(std::io::Error::other(e.to_string())) as DynError)
}
