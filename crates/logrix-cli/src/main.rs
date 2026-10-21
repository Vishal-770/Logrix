pub mod all_in_one;
pub mod args;
pub mod args_tuning;
pub mod backfill;
pub mod commands;
pub mod env;
pub mod factory;
pub mod handlers;
pub mod indexer;
pub mod ingester;
pub mod ingester_ops;
pub mod ingester_types;
pub mod leader;
pub mod processor;
pub mod processor_types;
pub mod shutdown;
pub mod status_tui;
pub mod warmup;
pub mod webhook_runner;

use alloy_primitives::Address;
use clap::Parser;
use logrix_api::start_api_server_with_schema;
use logrix_core::{domain::ChainId, ports::LeaderElectionPort, LocalLeaderElector};
use logrix_store_postgres::PostgresStore;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

use all_in_one::run_all_in_one;
use args::{Cli, Commands};
use backfill::run_backfill;
use factory::{create_queue_adapter, create_rpc_gateway};
use ingester::{run_ingester, IngesterConfig};
use leader::KubernetesLeaseElector;
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
    let target_contract: Address = cli
        .contract_address
        .parse()
        .expect("Valid target contract address");

    match cli.command {
        Commands::Status { watch } => {
            status_tui::run_status(&cli.database_url, watch).await?;
        }
        Commands::Migrate => {
            info!("Connecting to PostgreSQL and running migrations...");
            let pg_cfg = logrix_store_postgres::PostgresConfig::new(&cli.database_url, "default")
                .max_connections(cli.database_max_connections)
                .min_connections(cli.database_min_connections)
                .statement_timeout(std::time::Duration::from_secs(
                    cli.database_statement_timeout_secs,
                ));
            let store = PostgresStore::connect_with_config(&pg_cfg).await?;
            store.migrate().await?;
            info!("Migrations finished successfully.");
        }
        Commands::Ingester => {
            let queue = create_queue_adapter(&cli).await?;
            let gateway = create_rpc_gateway(&cli, chain_id).await?;
            let leader_elector: Arc<dyn LeaderElectionPort> = if cli.enable_leader_election {
                Arc::new(KubernetesLeaseElector::new(
                    cli.lease_name.clone(),
                    cli.k8s_namespace.clone(),
                    cli.pod_name.clone(),
                    None,
                    None,
                ))
            } else {
                Arc::new(LocalLeaderElector::new())
            };
            let ingester_config = IngesterConfig::from_cli(&cli, chain_id);
            run_ingester(ingester_config, gateway, queue, leader_elector).await?;
        }
        Commands::Processor => {
            let queue = create_queue_adapter(&cli).await?;
            let gateway = create_rpc_gateway(&cli, chain_id).await?;
            let config = ProcessorConfig::from_cli(&cli, chain_id, target_contract)?;
            run_processor(config, gateway, queue).await?;
        }
        Commands::WebhookDispatcher => {
            let queue = create_queue_adapter(&cli).await?;
            run_webhook_dispatcher(&cli, queue).await?;
        }
        Commands::Backfill {
            from_block,
            to_block,
            dry_run,
        } => {
            let queue = create_queue_adapter(&cli).await?;
            run_backfill(
                chain_id,
                target_contract,
                from_block,
                to_block,
                cli.chunk_size,
                dry_run,
                queue,
            )
            .await?;
        }
        Commands::Api { port } => {
            let pg_cfg = logrix_store_postgres::PostgresConfig::new(&cli.database_url, "default")
                .max_connections(cli.database_max_connections)
                .min_connections(cli.database_min_connections)
                .statement_timeout(std::time::Duration::from_secs(
                    cli.database_statement_timeout_secs,
                ));
            let store = Arc::new(PostgresStore::connect_with_config(&pg_cfg).await?);
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            let broadcaster = Arc::new(logrix_api::SubscriptionBroadcaster::default());
            broadcaster
                .clone()
                .start_postgres_listener(store.pool().clone());
            let api_config = logrix_api::ApiConfig {
                max_depth: cli.graphql_max_depth,
                max_complexity: cli.graphql_max_complexity,
                default_limit: cli.graphql_default_limit,
                max_limit: cli.graphql_max_limit,
                query_timeout_secs: cli.tuning.graphql_query_timeout_secs,
                chain_id: Some(cli.chain_id),
            };
            start_api_server_with_schema(
                store,
                cli.schema_path.as_deref(),
                Some(api_config),
                Some(broadcaster),
                addr,
            )
            .await?;
        }
        Commands::AllInOne { port } => {
            run_all_in_one(cli, chain_id, target_contract, port).await?;
        }
    }

    Ok(())
}
