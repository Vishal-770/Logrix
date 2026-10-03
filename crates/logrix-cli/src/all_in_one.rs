use alloy_primitives::Address;
use logrix_api::start_api_server_with_schema;
use logrix_core::{domain::ChainId, ports::LeaderElectionPort, LocalLeaderElector};
use logrix_store_postgres::PostgresStore;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::args::Cli;
use crate::factory::{create_queue_adapter, create_rpc_gateway};
use crate::ingester::run_ingester;
use crate::processor::{run_processor, ProcessorConfig};

pub async fn run_all_in_one(
    cli: Cli,
    chain_id: ChainId,
    target_contract: Address,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
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
        if let Err(e) = run_ingester(
            chain_id,
            ingester_gw,
            ingester_queue,
            &db_url,
            start,
            ingester_leader,
        )
        .await
        {
            error!(error = %e, "Ingester task failed");
        }
    });

    let processor_queue = queue.clone();
    let processor_gw = gateway.clone();
    let proc_config = ProcessorConfig::from_cli(&cli, chain_id, target_contract);
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
        if let Err(e) = start_api_server_with_schema(
            store_api,
            schema_path_clone.as_deref(),
            Some(api_config),
            addr,
        )
        .await
        {
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
