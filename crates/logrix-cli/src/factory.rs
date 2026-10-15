use crate::args::Cli;
use logrix_core::{domain::ChainId, ports::QueuePort};
use logrix_queue_rabbitmq::RabbitMQQueue;
use logrix_queue_sqs::SqsQueue;
use std::sync::Arc;
use tracing::info;

pub async fn create_queue_adapter(
    cli: &Cli,
) -> Result<Arc<dyn QueuePort>, Box<dyn std::error::Error>> {
    if cli.queue_driver.eq_ignore_ascii_case("sqs") {
        info!("Initializing AWS SQS queue adapter...");
        let queue = SqsQueue::from_env_with_params(
            &cli.sqs_live_url,
            &cli.sqs_backfill_url,
            &cli.sqs_webhook_url,
            &cli.sqs_dlq_url,
            cli.sqs_endpoint.as_deref(),
            cli.tuning.sqs_wait_time_seconds,
            cli.tuning.sqs_max_messages,
        )
        .await?;
        Ok(Arc::new(queue))
    } else {
        info!("Initializing RabbitMQ AMQP queue adapter...");
        let config = logrix_queue_rabbitmq::RabbitMQConfig::new(&cli.rabbitmq_url)
            .prefetch_count(cli.tuning.rabbitmq_prefetch)
            .connection_timeout(std::time::Duration::from_secs(
                cli.tuning.rabbitmq_timeout_secs,
            ));
        let queue = RabbitMQQueue::connect_with_config(&config).await?;
        Ok(Arc::new(queue))
    }
}

pub async fn create_rpc_gateway(
    cli: &Cli,
    chain_id: ChainId,
) -> Result<Arc<logrix_rpc_gateway::RpcGateway>, Box<dyn std::error::Error>> {
    use logrix_chain_evm::EvmClientConfig;
    use logrix_rpc_gateway::{GatewayConfig, ManagedProvider, ProviderPool, RpcGateway};
    use std::time::Duration;

    let rpc_url = crate::env::expand_env_placeholders(&cli.rpc_url, "RPC_URL")?;
    let fallback_urls = cli
        .rpc_fallback_urls
        .iter()
        .map(|url| crate::env::expand_env_placeholders(url, "RPC_FALLBACK_URLS"))
        .collect::<Result<Vec<_>, _>>()?;

    let gateway_cfg = GatewayConfig::default()
        .with_circuit_breaker(
            cli.tuning.rpc_cb_threshold,
            2,
            Duration::from_secs(cli.tuning.rpc_cooldown_secs),
        )
        .with_cooldown_bounds(
            cli.tuning.rpc_cooldown_secs,
            cli.tuning.rpc_cooldown_secs * 4,
        );

    let evm_cfg =
        EvmClientConfig::default().with_timeout(Duration::from_secs(cli.tuning.rpc_timeout_secs));

    let pool = ProviderPool::new(chain_id);
    let primary = ManagedProvider::new_with_config(
        "primary",
        &rpc_url,
        1,
        chain_id,
        &gateway_cfg,
        Some(evm_cfg.clone()),
    );
    pool.add_provider(primary).await;

    for (idx, fallback_url) in fallback_urls.iter().enumerate() {
        if !fallback_url.trim().is_empty() {
            let name = format!("fallback-{}", idx + 1);
            let p = ManagedProvider::new_with_config(
                name,
                fallback_url.trim(),
                (idx + 2) as u32,
                chain_id,
                &gateway_cfg,
                Some(evm_cfg.clone()),
            );
            pool.add_provider(p).await;
        }
    }

    let gateway = RpcGateway::new_with_config(
        chain_id,
        pool,
        cli.cu_budget,
        cli.bulk_stream_url.clone(),
        &gateway_cfg,
    );
    Ok(Arc::new(gateway))
}
