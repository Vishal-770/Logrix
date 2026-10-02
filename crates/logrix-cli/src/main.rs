use alloy_primitives::{Address, B256};
use clap::{Parser, Subcommand};
use logrix_api::start_api_server;
use logrix_chain_evm::{decode_erc20_transfer, EvmChainClient};
use logrix_core::{
    domain::{BlockRangeJob, ChainId, Checkpoint, LiveBlockJob, QueueMessage, QueueType},
    ports::{ChainPort, QueuePort, StorePort},
};
use logrix_queue_rabbitmq::RabbitMQQueue;
use logrix_queue_sqs::SqsQueue;
use logrix_store_postgres::{PostgresStore, TokenTransfer};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

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
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    /// Apply database migrations
    Migrate,
    /// Run the chain block ingester and queue publisher
    Ingester,
    /// Run the queue consumer, event decoder, and PostgreSQL committer
    Processor,
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
            run_ingester(
                chain_id,
                &cli.rpc_url,
                queue,
                &cli.database_url,
                cli.start_block,
            )
            .await?;
        }
        Commands::Processor => {
            let queue = create_queue_adapter(&cli).await?;
            run_processor(
                chain_id,
                &cli.rpc_url,
                queue,
                &cli.database_url,
                target_contract,
            )
            .await?;
        }
        Commands::Api { port } => {
            let store = Arc::new(PostgresStore::connect(&cli.database_url, "default").await?);
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            start_api_server(store, addr).await?;
        }
        Commands::AllInOne { port } => {
            info!("Starting Logrix All-In-One service...");
            let store = PostgresStore::connect(&cli.database_url, "default").await?;
            store.migrate().await?;

            let queue = create_queue_adapter(&cli).await?;

            let db_url = cli.database_url.clone();
            let rpc = cli.rpc_url.clone();
            let start = cli.start_block;
            let ingester_queue = queue.clone();

            // Ingester task
            let ingester_handle = tokio::spawn(async move {
                if let Err(e) = run_ingester(chain_id, &rpc, ingester_queue, &db_url, start).await {
                    error!(error = %e, "Ingester task failed");
                }
            });

            // Processor task
            let db_url_proc = cli.database_url.clone();
            let rpc_proc = cli.rpc_url.clone();
            let processor_queue = queue.clone();
            let processor_handle = tokio::spawn(async move {
                if let Err(e) = run_processor(
                    chain_id,
                    &rpc_proc,
                    processor_queue,
                    &db_url_proc,
                    target_contract,
                )
                .await
                {
                    error!(error = %e, "Processor task failed");
                }
            });

            // API task
            let store_api = Arc::new(PostgresStore::connect(&cli.database_url, "default").await?);
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            let api_handle = tokio::spawn(async move {
                if let Err(e) = start_api_server(store_api, addr).await {
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
    rpc_url: &str,
    queue: Arc<dyn QueuePort>,
    database_url: &str,
    start_block: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!(
        chain = chain_id.as_u64(),
        rpc = rpc_url,
        "Starting Logrix Ingester"
    );

    let client = EvmChainClient::new(chain_id, rpc_url);
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
                let latest = client.get_latest_block_number().await?;
                let start = latest.saturating_sub(50);
                info!(
                    latest,
                    start, "No checkpoint found. Starting 50 blocks behind chain head"
                );
                start
            }
        }
    };

    loop {
        match client.get_latest_block_number().await {
            Ok(latest) => {
                if current_block <= latest {
                    let chunk_size = client.chunker().chunk_size();
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
                warn!(error = %e, "Failed to query latest block number from RPC");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

async fn run_processor(
    chain_id: ChainId,
    rpc_url: &str,
    queue: Arc<dyn QueuePort>,
    database_url: &str,
    target_contract: Address,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Logrix Processor");
    let client = EvmChainClient::new(chain_id, rpc_url);
    let store = Arc::new(PostgresStore::connect(database_url, "default").await?);

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
            Some((_q_type, handle)) => {
                let (from_block, to_block) = match &handle.message {
                    QueueMessage::BackfillRange(job) => (job.from_block, job.to_block),
                    QueueMessage::LiveBlock(job) => (job.block_number, job.block_number),
                    _ => {
                        let _ = queue.ack(&handle).await;
                        continue;
                    }
                };

                // Fetch logs for range
                match client
                    .fetch_logs(from_block, to_block, &[target_contract])
                    .await
                {
                    Ok(logs) => {
                        // 1. Decode ERC-20 transfers
                        let mut transfers = Vec::new();
                        for log in &logs {
                            if let Some(decoded) = decode_erc20_transfer(chain_id.as_u64(), log) {
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

                        // 2. Commit events and checkpoint atomically
                        let checkpoint = Checkpoint::new(chain_id, to_block, B256::ZERO, true);
                        if let Err(e) = store.write_events_and_checkpoint(&logs, &checkpoint).await
                        {
                            error!(
                                error = %e,
                                "Failed to commit events and checkpoint, requeueing"
                            );
                            let _ = queue.nack(&handle, true).await;
                            tokio::time::sleep(Duration::from_millis(500)).await;
                            continue;
                        }

                        // 3. Save domain transfers
                        if !transfers.is_empty() {
                            if let Err(e) = store.save_transfers_batch(&transfers).await {
                                error!(error = %e, "Failed to save token transfers");
                            }
                        }

                        // 4. Acknowledge message from queue
                        if let Err(e) = queue.ack(&handle).await {
                            error!(error = %e, "Failed to ack message handle");
                        }
                    }
                    Err(e) => {
                        warn!(
                            from_block,
                            to_block,
                            error = %e,
                            "Failed to fetch logs from RPC, requeueing"
                        );
                        let _ = queue.nack(&handle, true).await;
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                }
            }
            None => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}
