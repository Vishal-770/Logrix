pub mod all_in_one;
pub mod args;
pub mod backfill;
pub mod commands;
pub mod deploy;
pub mod factory;
pub mod handlers;
pub mod indexer;
pub mod ingester;
pub mod init;
pub mod processor;
pub mod scaffold;
pub mod scaffold_templates;
pub mod status_tui;
pub mod webhook_runner;

use alloy_primitives::Address;
use clap::Parser;
use logrix_api::start_api_server_with_schema;
use logrix_core::{
    domain::ChainId, ports::LeaderElectionPort, KubernetesLeaseElector, LocalLeaderElector,
};
use logrix_store_postgres::PostgresStore;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::info;

use all_in_one::run_all_in_one;
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
    let target_contract: Address = cli
        .contract_address
        .parse()
        .expect("Valid target contract address");

    match cli.command {
        Commands::Init {
            name,
            profile,
            network,
            contract,
            start_block,
            logic,
            non_interactive,
        } => {
            init::run_init(init::InitOptions {
                name,
                profile,
                network,
                contract_address: contract,
                start_block,
                logic,
                non_interactive,
            })?;
        }
        Commands::Status { watch } => {
            status_tui::run_status(&cli.database_url, watch).await?;
        }
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
            run_ingester(
                chain_id,
                gateway,
                queue,
                &cli.database_url,
                cli.start_block,
                leader_elector,
            )
            .await?;
        }
        Commands::Processor => {
            let queue = create_queue_adapter(&cli).await?;
            let gateway = create_rpc_gateway(&cli, chain_id).await;
            let config = ProcessorConfig::from_cli(&cli, chain_id, target_contract);
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
                dry_run,
                queue,
            )
            .await?;
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
            start_api_server_with_schema(store, cli.schema_path.as_deref(), Some(api_config), addr)
                .await?;
        }
        Commands::AllInOne { port } => {
            run_all_in_one(cli, chain_id, target_contract, port).await?;
        }
        Commands::Deploy { local, aws } => {
            deploy::run_deploy(local, aws)?;
        }
    }

    Ok(())
}
