use crate::args::Cli;
use logrix_core::{domain::ChainId, ports::QueuePort};
use logrix_queue_rabbitmq::RabbitMQQueue;
use logrix_queue_sqs::SqsQueue;
use std::sync::Arc;
use tracing::info;

pub async fn create_queue_adapter(cli: &Cli) -> Result<Arc<dyn QueuePort>, Box<dyn std::error::Error>> {
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

pub async fn create_rpc_gateway(cli: &Cli, chain_id: ChainId) -> Arc<logrix_rpc_gateway::RpcGateway> {
    use logrix_rpc_gateway::{ManagedProvider, ProviderPool, RpcGateway};

    let pool = ProviderPool::new(chain_id);
    let primary = ManagedProvider::new("primary", &cli.rpc_url, 1, chain_id);
    pool.add_provider(primary).await;

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
