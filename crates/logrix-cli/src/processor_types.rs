use alloy_primitives::Address;
use logrix_blob_s3::S3BlobStore;
use logrix_core::{domain::ChainId, ports::QueuePort};
use logrix_handlers::UserLogicEngine;
use logrix_reconciler::{ReorgDetector, ReorgHandler};
use logrix_store_postgres::PostgresStore;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub chain_id: ChainId,
    pub database_url: String,
    pub target_contract: Address,
    pub ring_buffer_depth: usize,
    pub webhook_url: Option<String>,
    pub webhook_secret: String,
    pub reconciler_interval_secs: u64,
    pub manifest_path: Option<String>,
    pub enable_webhooks: bool,
    pub s3_bucket: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_prefix: Option<String>,
}

impl ProcessorConfig {
    pub fn from_cli(cli: &crate::args::Cli, chain_id: ChainId, target_contract: Address) -> Self {
        Self {
            chain_id,
            database_url: cli.database_url.clone(),
            target_contract,
            ring_buffer_depth: cli.ring_buffer_depth,
            webhook_url: cli.webhook_url.clone(),
            webhook_secret: cli.webhook_secret.clone(),
            reconciler_interval_secs: cli.reconciler_interval_secs,
            manifest_path: cli.manifest_path.clone(),
            enable_webhooks: cli.enable_webhooks,
            s3_bucket: cli.s3_bucket.clone(),
            s3_endpoint: cli.s3_endpoint.clone(),
            s3_prefix: cli.s3_prefix.clone(),
        }
    }
}

pub struct ProcessorContext<'a> {
    pub config: &'a ProcessorConfig,
    pub store: &'a PostgresStore,
    pub blob_store: Option<&'a S3BlobStore>,
    pub detector: &'a ReorgDetector,
    pub handler: &'a ReorgHandler,
    pub gateway: &'a logrix_rpc_gateway::RpcGateway,
    pub queue: &'a Arc<dyn QueuePort>,
    pub engine: Option<&'a UserLogicEngine>,
}

impl<'a> ProcessorContext<'a> {
    pub fn target_addresses(&self) -> Vec<Address> {
        if let Some(engine) = self.engine {
            let addrs: Vec<_> = engine
                .manifest()
                .contracts
                .iter()
                .map(|c| c.address)
                .collect();
            if !addrs.is_empty() {
                return addrs;
            }
        }
        vec![self.config.target_contract]
    }
}
