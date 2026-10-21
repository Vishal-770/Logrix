use alloy_primitives::Address;
use logrix_api::start_api_server_with_schema;
use logrix_core::{domain::ChainId, ports::LeaderElectionPort, LocalLeaderElector};
use logrix_store_postgres::{PostgresConfig, PostgresStore};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::args::Cli;
use crate::factory::{create_queue_adapter, create_rpc_gateway};
use crate::ingester::{run_ingester, IngesterConfig};
use crate::processor::{run_processor, ProcessorConfig};

fn postgres_config(cli: &Cli) -> PostgresConfig {
    PostgresConfig::new(&cli.database_url, "default")
        .max_connections(cli.database_max_connections)
        .min_connections(cli.database_min_connections)
        .statement_timeout(std::time::Duration::from_secs(
            cli.database_statement_timeout_secs,
        ))
}

pub async fn run_all_in_one(
    cli: Cli,
    chain_id: ChainId,
    target_contract: Address,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting Logrix All-In-One service...");
    let pg_cfg = postgres_config(&cli);
    let store = PostgresStore::connect_with_config(&pg_cfg).await?;
    store.migrate().await?;

    let queue = create_queue_adapter(&cli).await?;
    let gateway = create_rpc_gateway(&cli, chain_id).await?;

    let ingester_queue = queue.clone();
    let ingester_gw = gateway.clone();
    let ingester_leader: Arc<dyn LeaderElectionPort> = Arc::new(LocalLeaderElector::new());
    let ingester_config = IngesterConfig::from_cli(&cli, chain_id);

    let ingester_handle = tokio::spawn(async move {
        if let Err(e) = run_ingester(
            ingester_config,
            ingester_gw,
            ingester_queue,
            ingester_leader,
        )
        .await
        {
            error!(error = %e, "Ingester task failed");
        }
    });

    let processor_queue = queue.clone();
    let processor_gw = gateway.clone();
    let proc_config = ProcessorConfig::from_cli(&cli, chain_id, target_contract)?;
    let processor_handle = tokio::spawn(async move {
        if let Err(e) = run_processor(proc_config, processor_gw, processor_queue).await {
            error!(error = %e, "Processor task failed");
        }
    });

    let store_api = Arc::new(PostgresStore::connect_with_config(&pg_cfg).await?);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let schema_path_clone = cli.schema_path.clone();
    let api_config = logrix_api::ApiConfig {
        max_depth: cli.graphql_max_depth,
        max_complexity: cli.graphql_max_complexity,
        default_limit: cli.graphql_default_limit,
        max_limit: cli.graphql_max_limit,
        query_timeout_secs: cli.tuning.graphql_query_timeout_secs,
        chain_id: Some(cli.chain_id),
        allowed_origins: Vec::new(),
    };
    let broadcaster = Arc::new(logrix_api::SubscriptionBroadcaster::default());
    broadcaster
        .clone()
        .start_postgres_listener(store_api.pool().clone());
    let api_handle = tokio::spawn(async move {
        if let Err(e) = start_api_server_with_schema(
            store_api,
            schema_path_clone.as_deref(),
            Some(api_config),
            Some(broadcaster),
            addr,
        )
        .await
        {
            error!(error = %e, "API server failed");
        }
    });

    tokio::select! {
        _ = crate::shutdown::wait_for_shutdown_signal() => {
            info!("Received shutdown signal. Draining tasks gracefully...");
        }
        res = ingester_handle => { warn!(?res, "Ingester terminated"); }
        res = processor_handle => { warn!(?res, "Processor terminated"); }
        res = api_handle => { warn!(?res, "API terminated"); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::postgres_config;
    use crate::args::Cli;
    use clap::Parser;

    #[test]
    fn all_in_one_database_config_uses_cli_pool_limits() {
        let cli = Cli::parse_from([
            "logrix",
            "--database-max-connections",
            "17",
            "--database-min-connections",
            "4",
            "all-in-one",
        ]);
        let config = postgres_config(&cli);

        assert_eq!(config.max_connections, 17);
        assert_eq!(config.min_connections, 4);
    }
}
