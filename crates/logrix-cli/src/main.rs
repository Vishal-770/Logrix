use alloy_primitives::{Address, B256};
use clap::{Parser, Subcommand};
use logrix_api::start_api_server_with_schema;
use logrix_chain_evm::decode_erc20_transfer;
use logrix_core::{
    domain::{BlockRangeJob, ChainId, Checkpoint, LiveBlockJob, QueueMessage, QueueType},
    ports::{ChainPort, QueuePort, StorePort},
};
use logrix_queue_rabbitmq::RabbitMQQueue;
use logrix_queue_sqs::SqsQueue;
use logrix_reconciler::{
    ContinuityStatus, GapReconciler, ReorgDetector, ReorgHandler, WebhookDispatcher,
};
use logrix_store_postgres::{PostgresStore, TokenTransfer};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "logrix",
    version,
    about = "High-performance modular blockchain indexer"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// PostgreSQL connection URL (supports local K8s Postgres and AWS RDS)
    #[arg(
        long,
        env = "DATABASE_URL",
        default_value = "postgres://logrix:logrix@localhost:5432/logrix"
    )]
    database_url: String,

    /// Queue driver to use: "rabbitmq" (default) or "sqs" (AWS SQS)
    #[arg(long, env = "QUEUE_DRIVER", default_value = "rabbitmq")]
    queue_driver: String,

    /// RabbitMQ connection URL (used when QUEUE_DRIVER=rabbitmq)
    #[arg(
        long,
        env = "RABBITMQ_URL",
        default_value = "amqp://logrix:logrix@localhost:5672/%2f"
    )]
    rabbitmq_url: String,

    /// AWS SQS Live Block Queue URL (used when QUEUE_DRIVER=sqs)
    #[arg(long, env = "SQS_LIVE_URL", default_value = "")]
    sqs_live_url: String,

    /// AWS SQS Backfill Queue URL (used when QUEUE_DRIVER=sqs)
    #[arg(long, env = "SQS_BACKFILL_URL", default_value = "")]
    sqs_backfill_url: String,

    /// AWS SQS Webhook Queue URL (used when QUEUE_DRIVER=sqs)
    #[arg(long, env = "SQS_WEBHOOK_URL", default_value = "")]
    sqs_webhook_url: String,

    /// AWS SQS DLQ Queue URL (used when QUEUE_DRIVER=sqs)
    #[arg(long, env = "SQS_DLQ_URL", default_value = "")]
    sqs_dlq_url: String,

    /// Custom AWS endpoint override (e.g. http://localhost:4566 for LocalStack)
    #[arg(long, env = "SQS_ENDPOINT")]
    sqs_endpoint: Option<String>,

    /// Target EVM JSON-RPC URL
    #[arg(
        long,
        env = "RPC_URL",
        default_value = "https://sepolia-rollup.arbitrum.io/rpc"
    )]
    rpc_url: String,

    /// Secondary / Fallback EVM JSON-RPC URLs (comma-separated)
    #[arg(long, env = "RPC_FALLBACK_URLS", value_delimiter = ',')]
    rpc_fallback_urls: Vec<String>,

    /// Compute Unit (CU) budget limit for historical backfill
    #[arg(long, env = "CU_BUDGET")]
    cu_budget: Option<u64>,

    /// Bulk stream archive endpoint override (e.g. SQD / HyperSync)
    #[arg(long, env = "BULK_STREAM_URL")]
    bulk_stream_url: Option<String>,

    #[arg(long, env = "CHAIN_ID", default_value_t = 421614)]
    chain_id: u64,

    /// Target contract address to index (defaults to Arbitrum Sepolia USDC)
    #[arg(
        long,
        env = "CONTRACT_ADDRESS",
        default_value = "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d"
    )]
    contract_address: String,

    /// Initial block to begin indexing if no database checkpoint exists
    #[arg(long, env = "START_BLOCK")]
    start_block: Option<u64>,

    /// Target Webhook URL for external reorg and event notifications
    #[arg(long, env = "WEBHOOK_URL")]
    webhook_url: Option<String>,

    /// Background gap reconciler polling interval in seconds
    #[arg(long, env = "RECONCILER_INTERVAL_SECS", default_value_t = 30)]
    reconciler_interval_secs: u64,

    /// In-memory ring buffer depth for parent-hash reorg detection
    #[arg(long, env = "RING_BUFFER_DEPTH", default_value_t = 128)]
    ring_buffer_depth: usize,

    /// Optional path to YAML manifest for declarative mappings and WASM handlers
    #[arg(long, env = "MANIFEST_PATH")]
    manifest_path: Option<String>,

    /// Optional path to entity schema definition (schema.graphql or schema.yaml)
    #[arg(long, env = "SCHEMA_PATH")]
    schema_path: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Apply database migrations
    Migrate,
    /// Run the chain block ingester and queue publisher
    Ingester,
    /// Run the queue consumer, event decoder, and PostgreSQL committer
    Processor,
    /// Run historical backfill or perform pre-flight cost estimation
    Backfill {
        #[arg(long)]
        from_block: u64,

        #[arg(long)]
        to_block: u64,

        /// Dry-run estimation of chunks, CUs, estimated USD costs, and duration
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Run the GraphQL query API server
    Api {
        #[arg(long, env = "PORT", default_value_t = 4000)]
        port: u16,
    },
    /// Run all services concurrently (Ingester + Processor + API)
    AllInOne {
        #[arg(long, env = "PORT", default_value_t = 4000)]
        port: u16,
    },
}

async fn create_queue_adapter(cli: &Cli) -> Result<Arc<dyn QueuePort>, Box<dyn std::error::Error>> {
    if cli.queue_driver.eq_ignore_ascii_case("sqs") {
        info!("Initializing AWS SQS queue adapter...");
        let queue = SqsQueue::from_env(
            &cli.sqs_live_url,
            &cli.sqs_backfill_url,
            &cli.sqs_webhook_url,
            &cli.sqs_dlq_url,
            cli.sqs_endpoint.as_deref(),
        )
        .await?;
        Ok(Arc::new(queue))
    } else {
        info!("Initializing RabbitMQ AMQP queue adapter...");
        let queue = RabbitMQQueue::connect(&cli.rabbitmq_url, 100).await?;
        Ok(Arc::new(queue))
    }
}

async fn create_rpc_gateway(cli: &Cli, chain_id: ChainId) -> Arc<logrix_rpc_gateway::RpcGateway> {
    use logrix_rpc_gateway::{ManagedProvider, ProviderPool, RpcGateway};

    let pool = ProviderPool::new(chain_id);

    // Primary provider (priority 1)
    let primary = ManagedProvider::new("primary", &cli.rpc_url, 1, chain_id);
    pool.add_provider(primary).await;

    // Fallback providers (priority 2+)
    for (idx, fallback_url) in cli.rpc_fallback_urls.iter().enumerate() {
        if !fallback_url.trim().is_empty() {
            let name = format!("fallback-{}", idx + 1);
            let p = ManagedProvider::new(name, fallback_url.trim(), (idx + 2) as u32, chain_id);
            pool.add_provider(p).await;
        }
    }

    let gateway = RpcGateway::new(chain_id, pool, cli.cu_budget, cli.bulk_stream_url.clone());
    Arc::new(gateway)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,logrix=debug".into()),
        )
        .init();

    let cli = Cli::parse();
    let chain_id = ChainId::new(cli.chain_id);
    let target_contract: Address = cli
        .contract_address
        .parse()
        .expect("Valid target contract address");

    match cli.command {
        Commands::Migrate => {
            info!("Connecting to PostgreSQL and running migrations...");
            let store = PostgresStore::connect(&cli.database_url, "default").await?;
            store.migrate().await?;
            info!("Migrations finished successfully.");
        }
        Commands::Ingester => {
            let queue = create_queue_adapter(&cli).await?;
            let gateway = create_rpc_gateway(&cli, chain_id).await;
            run_ingester(chain_id, gateway, queue, &cli.database_url, cli.start_block).await?;
        }
        Commands::Processor => {
            let queue = create_queue_adapter(&cli).await?;
            let gateway = create_rpc_gateway(&cli, chain_id).await;
            let config = ProcessorConfig {
                chain_id,
                database_url: cli.database_url.clone(),
                target_contract,
                ring_buffer_depth: cli.ring_buffer_depth,
                webhook_url: cli.webhook_url.clone(),
                reconciler_interval_secs: cli.reconciler_interval_secs,
                manifest_path: cli.manifest_path.clone(),
            };
            run_processor(config, gateway, queue).await?;
        }
        Commands::Backfill {
            from_block,
            to_block,
            dry_run,
        } => {
            if dry_run {
                use logrix_rpc_gateway::CuBudgetTracker;
                let chunk_size = 500;
                let (chunks, cu, usd) =
                    CuBudgetTracker::estimate_range_cost(from_block, to_block, chunk_size);
                let total_blocks = if to_block >= from_block {
                    to_block - from_block + 1
                } else {
                    0
                };
                let est_seconds = chunks * 2; // rough assumption: ~2 sec per chunk

                println!("\n=======================================================");
                println!("           LOGRIX HISTORICAL BACKFILL DRY-RUN          ");
                println!("=======================================================");
                println!("  Target Chain ID       : {}", chain_id);
                println!("  Target Contract       : {}", target_contract);
                println!("  Block Range           : {} -> {}", from_block, to_block);
                println!("  Total Blocks          : {}", total_blocks);
                println!("  Average Chunk Size    : {} blocks", chunk_size);
                println!("  Estimated Chunks      : {}", chunks);
                println!("  Estimated CUs         : {} Compute Units", cu);
                println!("  Estimated Cloud Cost  : ${:.4} USD (@ $1/1M CU)", usd);
                println!(
                    "  Estimated Duration    : ~{} seconds ({:.1} minutes)",
                    est_seconds,
                    est_seconds as f64 / 60.0
                );
                println!("=======================================================\n");
            } else {
                info!(
                    from_block,
                    to_block,
                    chain = chain_id.as_u64(),
                    "Dispatching backfill range to queue..."
                );
                let queue = create_queue_adapter(&cli).await?;
                let job = BlockRangeJob::new(
                    chain_id,
                    from_block,
                    to_block,
                    format!("{chain_id}:{from_block}-{to_block}"),
                );
                queue
                    .publish(QueueType::Backfill, &QueueMessage::BackfillRange(job))
                    .await?;
                info!("Backfill range dispatched successfully.");
            }
        }
        Commands::Api { port } => {
            let store = Arc::new(PostgresStore::connect(&cli.database_url, "default").await?);
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            start_api_server_with_schema(store, cli.schema_path.as_deref(), addr).await?;
        }
        Commands::AllInOne { port } => {
            info!("Starting Logrix All-In-One service...");
            let store = PostgresStore::connect(&cli.database_url, "default").await?;
            store.migrate().await?;

            let queue = create_queue_adapter(&cli).await?;
            let gateway = create_rpc_gateway(&cli, chain_id).await;

            let db_url = cli.database_url.clone();
            let start = cli.start_block;
            let ingester_queue = queue.clone();
            let ingester_gw = gateway.clone();

            // Ingester task
            let ingester_handle = tokio::spawn(async move {
                if let Err(e) =
                    run_ingester(chain_id, ingester_gw, ingester_queue, &db_url, start).await
                {
                    error!(error = %e, "Ingester task failed");
                }
            });

            // Processor task
            let processor_queue = queue.clone();
            let processor_gw = gateway.clone();
            let proc_config = ProcessorConfig {
                chain_id,
                database_url: cli.database_url.clone(),
                target_contract,
                ring_buffer_depth: cli.ring_buffer_depth,
                webhook_url: cli.webhook_url.clone(),
                reconciler_interval_secs: cli.reconciler_interval_secs,
                manifest_path: cli.manifest_path.clone(),
            };
            let processor_handle = tokio::spawn(async move {
                if let Err(e) = run_processor(proc_config, processor_gw, processor_queue).await {
                    error!(error = %e, "Processor task failed");
                }
            });

            // API task
            let store_api = Arc::new(PostgresStore::connect(&cli.database_url, "default").await?);
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            let schema_path_clone = cli.schema_path.clone();
            let api_handle = tokio::spawn(async move {
                if let Err(e) =
                    start_api_server_with_schema(store_api, schema_path_clone.as_deref(), addr)
                        .await
                {
                    error!(error = %e, "API server failed");
                }
            });

            // Wait for shutdown or task failure
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("Received shutdown signal (Ctrl+C). Draining tasks gracefully...");
                }
                res = ingester_handle => {
                    warn!(?res, "Ingester terminated");
                }
                res = processor_handle => {
                    warn!(?res, "Processor terminated");
                }
                res = api_handle => {
                    warn!(?res, "API terminated");
                }
            }
        }
    }

    Ok(())
}

async fn run_ingester(
    chain_id: ChainId,
    gateway: Arc<logrix_rpc_gateway::RpcGateway>,
    queue: Arc<dyn QueuePort>,
    database_url: &str,
    start_block: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        chain = chain_id.as_u64(),
        "Starting Logrix Ingester with Cost-Aware RPC Gateway"
    );

    let store = PostgresStore::connect(database_url, "default").await?;

    // Determine initial block
    let mut current_block = match store.get_checkpoint(chain_id).await? {
        Some(cp) => {
            info!(
                checkpoint = cp.last_indexed_block,
                "Resuming from database checkpoint"
            );
            cp.last_indexed_block + 1
        }
        None => {
            if let Some(block) = start_block {
                info!(block, "Starting from configured start block");
                block
            } else {
                let latest = gateway.get_latest_block_number().await?;
                let start = latest.saturating_sub(50);
                info!(
                    latest,
                    start, "No checkpoint found. Starting 50 blocks behind chain head"
                );
                start
            }
        }
    };

    let chunk_size = 500u64;

    loop {
        match gateway.get_latest_block_number().await {
            Ok(latest) => {
                if current_block <= latest {
                    let to_block = (current_block + chunk_size - 1).min(latest);

                    if current_block == to_block && to_block == latest {
                        let job = LiveBlockJob::new(chain_id, to_block, B256::ZERO, B256::ZERO);
                        let msg = QueueMessage::LiveBlock(job);
                        queue.publish(QueueType::Live, &msg).await?;
                    } else {
                        let job = BlockRangeJob::new(
                            chain_id,
                            current_block,
                            to_block,
                            format!("{chain_id}:{current_block}-{to_block}"),
                        );
                        let msg = QueueMessage::BackfillRange(job);
                        queue.publish(QueueType::Backfill, &msg).await?;
                    }

                    current_block = to_block + 1;
                } else {
                    // Up to date, wait for next block
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to query latest block number from RPC Gateway");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Configuration for the Logrix stream and reorg processor.
#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub chain_id: ChainId,
    pub database_url: String,
    pub target_contract: Address,
    pub ring_buffer_depth: usize,
    pub webhook_url: Option<String>,
    pub reconciler_interval_secs: u64,
    pub manifest_path: Option<String>,
}

async fn run_processor(
    config: ProcessorConfig,
    gateway: Arc<logrix_rpc_gateway::RpcGateway>,
    queue: Arc<dyn QueuePort>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Logrix Processor with Reorg Engine & Self-Healing Reconciler");
    let store = Arc::new(PostgresStore::connect(&config.database_url, "default").await?);

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
    let webhook_dispatcher = WebhookDispatcher::new(queue.clone(), config.webhook_url);

    // Initialize User Logic Engine if manifest is provided
    let engine_opt = if let Some(ref path) = config.manifest_path {
        info!(path, "Loading user logic engine manifest");
        let manifest = logrix_handlers::Manifest::from_file(path)?;
        let mut engine = logrix_handlers::UserLogicEngine::new(manifest)?;
        for contract in &engine.manifest().contracts.clone() {
            if let Some(ref wasm_path) = contract.wasm_handler {
                info!(%contract.address, wasm_path, "Loading contract WASM handler");
                engine.load_wasm_handler_file(contract.address, wasm_path)?;
            }
        }
        Some(engine)
    } else {
        None
    };

    // Spawn background gap reconciler loop
    tokio::spawn(async move {
        reconciler.run_loop().await;
    });

    // Spawn background webhook notification loop
    tokio::spawn(async move {
        webhook_dispatcher.run_loop().await;
    });

    loop {
        // Try Backfill queue first, then Live queue
        let msg_opt = match queue.consume(QueueType::Backfill).await? {
            Some(h) => Some((QueueType::Backfill, h)),
            None => queue
                .consume(QueueType::Live)
                .await?
                .map(|h| (QueueType::Live, h)),
        };

        match msg_opt {
            Some((_q_type, handle)) => match &handle.message {
                QueueMessage::LiveBlock(job) => {
                    let block_num = job.block_number;
                    match gateway
                        .fetch_block_envelope(block_num, &[config.target_contract])
                        .await
                    {
                        Ok(Some(envelope)) => match detector.check_envelope(&envelope).await {
                            Ok(ContinuityStatus::Continuous) => {
                                let mut transfers = Vec::new();
                                for log in &envelope.logs {
                                    if let Some(decoded) =
                                        decode_erc20_transfer(config.chain_id.as_u64(), log)
                                    {
                                        transfers.push(TokenTransfer {
                                            chain_id: decoded.chain_id,
                                            block_number: decoded.block_number,
                                            block_hash: decoded.block_hash,
                                            tx_hash: decoded.tx_hash,
                                            log_index: decoded.log_index,
                                            contract_address: decoded.contract_address,
                                            from_address: decoded.from_address,
                                            to_address: decoded.to_address,
                                            amount: decoded.amount,
                                            timestamp: decoded.timestamp,
                                        });
                                    }
                                }

                                let checkpoint = Checkpoint::new(
                                    config.chain_id,
                                    block_num,
                                    envelope.block_hash,
                                    true,
                                );
                                if let Err(e) = store
                                    .write_events_and_checkpoint(&envelope.logs, &checkpoint)
                                    .await
                                {
                                    error!(
                                        error = %e,
                                        "Failed to commit live events and checkpoint, requeueing"
                                    );
                                    let _ = queue.nack(&handle, true).await;
                                    tokio::time::sleep(Duration::from_millis(500)).await;
                                    continue;
                                }

                                if !transfers.is_empty() {
                                    if let Err(e) = store.save_transfers_batch(&transfers).await {
                                        error!(error = %e, "Failed to save token transfers");
                                    }
                                }

                                if let Some(ref engine) = engine_opt {
                                    for log in &envelope.logs {
                                        if let Ok(staging) = engine.process_log(log).await {
                                            let (mutations, emitted) =
                                                engine.commit_staging(staging).await;
                                            if !emitted.is_empty() {
                                                let inserts: Vec<
                                                    logrix_store_postgres::EntityInsert,
                                                > = emitted
                                                    .into_iter()
                                                    .map(|e| {
                                                        let entity_id = e
                                                            .payload
                                                            .get("id")
                                                            .and_then(|v| v.as_str())
                                                            .unwrap_or("default")
                                                            .to_string();
                                                        logrix_store_postgres::EntityInsert {
                                                            entity_type: e.entity_type,
                                                            entity_id,
                                                            data: e.payload,
                                                        }
                                                    })
                                                    .collect();
                                                if let Err(e) = store
                                                    .save_entities_batch(
                                                        config.chain_id.as_u64(),
                                                        block_num,
                                                        &inserts,
                                                    )
                                                    .await
                                                {
                                                    error!(error = %e, "Failed to save dynamic schema entities");
                                                }
                                            }
                                            if !mutations.is_empty() {
                                                debug!(
                                                    mutations = mutations.len(),
                                                    "Committed user logic state mutations"
                                                );
                                            }
                                        }
                                    }
                                }

                                let _ = queue.ack(&handle).await;
                            }
                            Ok(ContinuityStatus::ReorgDetected {
                                fork_block,
                                fork_hash,
                                reorg_depth,
                            }) => {
                                warn!(
                                    fork_block,
                                    fork_hash = %fork_hash,
                                    reorg_depth,
                                    "Reorg detected! Executing atomic rollback"
                                );
                                if let Err(e) = handler
                                    .execute_rollback(fork_block, fork_hash, reorg_depth)
                                    .await
                                {
                                    error!(
                                        error = %e,
                                        "Reorg rollback execution failed, requeueing"
                                    );
                                    let _ = queue.nack(&handle, true).await;
                                    tokio::time::sleep(Duration::from_millis(1000)).await;
                                    continue;
                                }
                                let _ = queue.ack(&handle).await;
                            }
                            Ok(ContinuityStatus::GapDetected {
                                from_block,
                                to_block,
                            }) => {
                                warn!(
                                    from_block,
                                    to_block,
                                    "Gap detected in live stream; dispatching backfill range"
                                );
                                let gap_job = BlockRangeJob::new(
                                    config.chain_id,
                                    from_block,
                                    to_block,
                                    format!("{}:{from_block}-{to_block}", config.chain_id),
                                );
                                let _ = queue
                                    .publish(
                                        QueueType::Backfill,
                                        &QueueMessage::BackfillRange(gap_job),
                                    )
                                    .await;
                                let _ = queue.nack(&handle, true).await;
                                tokio::time::sleep(Duration::from_millis(500)).await;
                            }
                            Err(e) => {
                                error!(
                                    error = %e,
                                    "Reorg continuity check failed, requeueing"
                                );
                                let _ = queue.nack(&handle, true).await;
                                tokio::time::sleep(Duration::from_millis(500)).await;
                            }
                        },
                        Ok(None) => {
                            warn!(
                                block_num,
                                "Block envelope not found on chain yet, requeueing"
                            );
                            let _ = queue.nack(&handle, true).await;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                        Err(e) => {
                            warn!(
                                block_num,
                                error = %e,
                                "Failed to fetch block envelope from RPC Gateway, requeueing"
                            );
                            let _ = queue.nack(&handle, true).await;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
                QueueMessage::BackfillRange(job) => {
                    let (from_block, to_block) = (job.from_block, job.to_block);
                    match gateway
                        .fetch_logs(from_block, to_block, &[config.target_contract])
                        .await
                    {
                        Ok(logs) => {
                            let mut transfers = Vec::new();
                            for log in &logs {
                                if let Some(decoded) =
                                    decode_erc20_transfer(config.chain_id.as_u64(), log)
                                {
                                    transfers.push(TokenTransfer {
                                        chain_id: decoded.chain_id,
                                        block_number: decoded.block_number,
                                        block_hash: decoded.block_hash,
                                        tx_hash: decoded.tx_hash,
                                        log_index: decoded.log_index,
                                        contract_address: decoded.contract_address,
                                        from_address: decoded.from_address,
                                        to_address: decoded.to_address,
                                        amount: decoded.amount,
                                        timestamp: decoded.timestamp,
                                    });
                                }
                            }

                            let checkpoint =
                                Checkpoint::new(config.chain_id, to_block, B256::ZERO, true);
                            if let Err(e) =
                                store.write_events_and_checkpoint(&logs, &checkpoint).await
                            {
                                error!(
                                    error = %e,
                                    "Failed to commit backfill events and checkpoint, requeueing"
                                );
                                let _ = queue.nack(&handle, true).await;
                                tokio::time::sleep(Duration::from_millis(500)).await;
                                continue;
                            }

                            if !transfers.is_empty() {
                                if let Err(e) = store.save_transfers_batch(&transfers).await {
                                    error!(error = %e, "Failed to save token transfers");
                                }
                            }

                            if let Some(ref engine) = engine_opt {
                                for log in &logs {
                                    if let Ok(staging) = engine.process_log(log).await {
                                        let (mutations, emitted) =
                                            engine.commit_staging(staging).await;
                                        if !emitted.is_empty() {
                                            let inserts: Vec<logrix_store_postgres::EntityInsert> =
                                                emitted
                                                    .into_iter()
                                                    .map(|e| {
                                                        let entity_id = e
                                                            .payload
                                                            .get("id")
                                                            .and_then(|v| v.as_str())
                                                            .unwrap_or("default")
                                                            .to_string();
                                                        logrix_store_postgres::EntityInsert {
                                                            entity_type: e.entity_type,
                                                            entity_id,
                                                            data: e.payload,
                                                        }
                                                    })
                                                    .collect();
                                            if let Err(e) = store
                                                .save_entities_batch(
                                                    config.chain_id.as_u64(),
                                                    log.block_number,
                                                    &inserts,
                                                )
                                                .await
                                            {
                                                error!(error = %e, "Failed to save dynamic schema entities");
                                            }
                                        }
                                        if !mutations.is_empty() {
                                            debug!(
                                                mutations = mutations.len(),
                                                "Committed user logic state mutations"
                                            );
                                        }
                                    }
                                }
                            }

                            let _ = queue.ack(&handle).await;
                        }
                        Err(e) => {
                            warn!(
                                from_block,
                                to_block,
                                error = %e,
                                "Failed to fetch logs from RPC Gateway, requeueing"
                            );
                            let _ = queue.nack(&handle, true).await;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                    }
                }
                _ => {
                    let _ = queue.ack(&handle).await;
                }
            },
            None => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}
