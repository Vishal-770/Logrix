use alloy_primitives::Address;
use logrix_blob_s3::{S3BlobStore, S3Config};
use logrix_core::{domain::ChainId, error::LogrixResult, ports::QueuePort};
use logrix_handlers::UserLogicEngine;
use logrix_reconciler::{ReorgDetector, ReorgHandler};
use logrix_store_postgres::PostgresStore;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ProcessorConfig {
    pub chain_id: ChainId,
    pub database_url: String,
    pub database_max_connections: u32,
    pub database_min_connections: u32,
    pub database_statement_timeout_secs: u64,
    pub target_contract: Address,
    pub ring_buffer_depth: usize,
    pub webhook_url: Option<String>,
    pub webhook_secret: String,
    pub reconciler_interval_secs: u64,
    pub reconciler_chunk_size: Option<u64>,
    pub reconciler_lag_threshold: u64,
    pub chunk_size: u64,
    pub manifest_path: Option<String>,
    pub enable_webhooks: bool,
    pub s3_bucket: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_prefix: Option<String>,
    pub wasm_fuel_limit: u64,
    pub wasm_max_memory_mb: u32,
    pub webhook_timeout_secs: u64,
    pub webhook_max_retries: u32,
    pub webhook_worker_idle_ms: u64,
    pub webhook_worker_backoff_ms: u64,
}

impl ProcessorConfig {
    pub fn from_cli(
        cli: &crate::args::Cli,
        chain_id: ChainId,
        target_contract: Address,
    ) -> LogrixResult<Self> {
        let webhook_url = cli
            .webhook_url
            .as_deref()
            .map(|url| crate::env::expand_env_placeholders(url, "WEBHOOK_URL"))
            .transpose()?;
        let webhook_secret =
            crate::env::expand_env_placeholders(&cli.webhook_secret, "WEBHOOK_SECRET")?;
        Ok(Self {
            chain_id,
            database_url: cli.database_url.clone(),
            database_max_connections: cli.database_max_connections,
            database_min_connections: cli.database_min_connections,
            database_statement_timeout_secs: cli.database_statement_timeout_secs,
            target_contract,
            ring_buffer_depth: cli.ring_buffer_depth,
            webhook_url,
            webhook_secret,
            reconciler_interval_secs: cli.reconciler_interval_secs,
            reconciler_chunk_size: cli.tuning.reconciler_chunk_size,
            reconciler_lag_threshold: cli.tuning.reconciler_lag_threshold,
            chunk_size: cli.chunk_size,
            manifest_path: cli.manifest_path.clone(),
            enable_webhooks: cli.enable_webhooks,
            s3_bucket: cli.s3_bucket.clone(),
            s3_endpoint: cli.s3_endpoint.clone(),
            s3_prefix: cli.s3_prefix.clone(),
            wasm_fuel_limit: cli.tuning.wasm_fuel_limit,
            wasm_max_memory_mb: cli.tuning.wasm_max_memory_mb,
            webhook_timeout_secs: cli.tuning.webhook_timeout_secs,
            webhook_max_retries: cli.tuning.webhook_max_retries,
            webhook_worker_idle_ms: cli.tuning.webhook_worker_idle_ms,
            webhook_worker_backoff_ms: cli.tuning.webhook_worker_backoff_ms,
        })
    }

    pub async fn init_blob_store(&self) -> LogrixResult<Option<S3BlobStore>> {
        if let Some(ref bucket) = self.s3_bucket {
            let s3_conf =
                S3Config::from_env(bucket, self.s3_prefix.clone(), self.s3_endpoint.as_deref())
                    .await?;
            Ok(Some(S3BlobStore::new(s3_conf)))
        } else {
            Ok(None)
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

pub async fn load_user_engine(
    config: &ProcessorConfig,
    store: &PostgresStore,
) -> Result<Option<UserLogicEngine>, Box<dyn std::error::Error>> {
    let Some(ref path) = config.manifest_path else {
        return Ok(None);
    };

    let manifest = logrix_handlers::Manifest::from_file(path)?;
    let mut engine = UserLogicEngine::new(manifest)?
        .with_global_limits(config.wasm_fuel_limit, config.wasm_max_memory_mb);
    for contract in &engine.manifest().contracts.clone() {
        if let Some(ref wasm_path) = contract.wasm_handler {
            let resolved = if std::path::Path::new(wasm_path).exists() {
                wasm_path.clone()
            } else if std::path::Path::new("/etc/logrix/handlers/mapping.wasm").exists() {
                "/etc/logrix/handlers/mapping.wasm".to_string()
            } else {
                wasm_path.clone()
            };
            engine.load_wasm_handler_file(contract.address, resolved)?;
        }
    }
    crate::warmup::prewarm_state_store(store, &engine, config.chain_id.as_u64()).await;
    Ok(Some(engine))
}
