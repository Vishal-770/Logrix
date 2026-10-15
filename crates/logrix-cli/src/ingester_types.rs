use logrix_core::{domain::ChainId, error::LogrixResult};

/// Ingester configuration hierarchy merging CLI args, env, and manifest defaults.
#[derive(Debug, Clone)]
pub struct IngesterConfig {
    pub chain_id: ChainId,
    pub database_url: String,
    pub database_max_connections: u32,
    pub database_min_connections: u32,
    pub start_block: Option<u64>,
    pub chunk_size: u64,
    pub max_queue_depth: u64,
    pub chain_head_poll_ms: u64,
    pub manifest_path: Option<String>,
}

impl IngesterConfig {
    pub fn from_cli(cli: &crate::args::Cli, chain_id: ChainId) -> Self {
        Self {
            chain_id,
            database_url: cli.database_url.clone(),
            database_max_connections: cli.database_max_connections,
            database_min_connections: cli.database_min_connections,
            start_block: cli.start_block,
            chunk_size: cli.chunk_size,
            max_queue_depth: cli.max_queue_depth,
            chain_head_poll_ms: cli.tuning.chain_head_poll_ms,
            manifest_path: cli.manifest_path.clone(),
        }
    }

    /// Resolve effective operational values respecting manifest indexing overrides.
    pub fn resolve_tuning(&self) -> LogrixResult<(u64, u64, u64)> {
        let mut chunk_size = self.chunk_size.max(1);
        let mut max_queue_depth = self.max_queue_depth;
        let mut chain_head_poll_ms = self.chain_head_poll_ms;

        if let Some(ref path) = self.manifest_path {
            let manifest = logrix_handlers::Manifest::from_file(path)?;
            if let Some(ref idx) = manifest.indexing {
                if let Some(cs) = idx.chunk_size {
                    chunk_size = cs.max(1);
                }
                if let Some(mqd) = idx.max_queue_depth {
                    max_queue_depth = mqd;
                }
                if let Some(chp) = idx.chain_head_poll_ms {
                    chain_head_poll_ms = chp;
                }
            }
        }
        Ok((chunk_size, max_queue_depth, chain_head_poll_ms))
    }
}
