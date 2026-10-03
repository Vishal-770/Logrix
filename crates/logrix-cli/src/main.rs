pub mod args;
pub mod backfill;
pub mod factory;
pub mod indexer;
pub mod ingester;
pub mod processor;
pub mod webhook_runner;

use alloy_primitives::Address;
use clap::Parser;
use logrix_api::start_api_server_with_schema;
use logrix_core::{
    domain::ChainId,
    ports::LeaderElectionPort,
    KubernetesLeaseElector, LocalLeaderElector,
};
use logrix_store_postgres::PostgresStore;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info, warn};

use args::{Cli, Commands};
use backfill::run_backfill;
use factory::{create_queue_adapter, create_rpc_gateway};
use ingester::run_ingester;
use processor::{run_processor, ProcessorConfig};
use webhook_runner::run_webhook_dispatcher;

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
    let target_contract: Address = cli.contract_address.parse().expect("Valid target contract address");

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
            let leader_elector: Arc<dyn LeaderElectionPort> = if cli.enable_leader_election {
                Arc::new(KubernetesLeaseElector::new(cli.lease_name.clone(), cli.k8s_namespace.clone(), cli.pod_name.clone(), None, None))
            } else {
                Arc::new(LocalLeaderElector::new())
            };
            run_ingester(chain_id, gateway, queue, &cli.database_url, cli.start_block, leader_elector).await?;
        }
        Commands::Processor => {
            let queue = create_queue_adapter(&cli).await?;
            let gateway = create_rpc_gateway(&cli, chain_id).await;
            let config = build_processor_config(&cli, chain_id, target_contract);
            run_processor(config, gateway, queue).await?;
        }
        Commands::WebhookDispatcher => {
            let queue = create_queue_adapter(&cli).await?;
            run_webhook_dispatcher(&cli, queue).await?;
        }
        Commands::Backfill { from_block, to_block, dry_run } => {
            let queue = create_queue_adapter(&cli).await?;
            run_backfill(chain_id, target_contract, from_block, to_block, dry_run, queue).await?;
        }
        Commands::Api { port } => {
            let store = Arc::new(PostgresStore::connect(&cli.database_url, "default").await?);
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            let api_config = logrix_api::ApiConfig {
                max_depth: cli.graphql_max_depth,
                max_complexity: cli.graphql_max_complexity,
                default_limit: cli.graphql_default_limit,
                max_limit: cli.graphql_max_limit,
                query_timeout_secs: 5,
            };
            start_api_server_with_schema(store, cli.schema_path.as_deref(), Some(api_config), addr).await?;
        }
        Commands::AllInOne { port } => {
            run_all_in_one(cli, chain_id, target_contract, port).await?;
        }
    }

    Ok(())
}

fn build_processor_config(cli: &Cli, chain_id: ChainId, target_contract: Address) -> ProcessorConfig {
    ProcessorConfig {
        chain_id,
        database_url: cli.database_url.clone(),
        target_contract,
        ring_buffer_depth: cli.ring_buffer_depth,
        webhook_url: cli.webhook_url.clone(),
        reconciler_interval_secs: cli.reconciler_interval_secs,
        manifest_path: cli.manifest_path.clone(),
        enable_webhooks: cli.enable_webhooks,
        s3_bucket: cli.s3_bucket.clone(),
        s3_endpoint: cli.s3_endpoint.clone(),
        s3_prefix: cli.s3_prefix.clone(),
    }
}

async fn run_all_in_one(cli: Cli, chain_id: ChainId, target_contract: Address, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Logrix All-In-One service...");
    let store = PostgresStore::connect(&cli.database_url, "default").await?;
    store.migrate().await?;

    let queue = create_queue_adapter(&cli).await?;
    let gateway = create_rpc_gateway(&cli, chain_id).await;

    let db_url = cli.database_url.clone();
    let start = cli.start_block;
    let ingester_queue = queue.clone();
    let ingester_gw = gateway.clone();
    let ingester_leader: Arc<dyn LeaderElectionPort> = Arc::new(LocalLeaderElector::new());

    let ingester_handle = tokio::spawn(async move {
        if let Err(e) = run_ingester(chain_id, ingester_gw, ingester_queue, &db_url, start, ingester_leader).await {
            error!(error = %e, "Ingester task failed");
        }
    });

    let processor_queue = queue.clone();
    let processor_gw = gateway.clone();
    let proc_config = build_processor_config(&cli, chain_id, target_contract);
    let processor_handle = tokio::spawn(async move {
        if let Err(e) = run_processor(proc_config, processor_gw, processor_queue).await {
            error!(error = %e, "Processor task failed");
        }
    });

    let store_api = Arc::new(PostgresStore::connect(&cli.database_url, "default").await?);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let schema_path_clone = cli.schema_path.clone();
    let api_config = logrix_api::ApiConfig {
        max_depth: cli.graphql_max_depth,
        max_complexity: cli.graphql_max_complexity,
        default_limit: cli.graphql_default_limit,
        max_limit: cli.graphql_max_limit,
        query_timeout_secs: 5,
    };
    let api_handle = tokio::spawn(async move {
        if let Err(e) = start_api_server_with_schema(store_api, schema_path_clone.as_deref(), Some(api_config), addr).await {
            error!(error = %e, "API server failed");
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal (Ctrl+C). Draining tasks gracefully...");
        }
        res = ingester_handle => { warn!(?res, "Ingester terminated"); }
        res = processor_handle => { warn!(?res, "Processor terminated"); }
        res = api_handle => { warn!(?res, "API terminated"); }
    }
    Ok(())
}
