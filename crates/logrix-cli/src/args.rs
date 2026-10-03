use clap::{Parser, Subcommand};

#[derive(Parser, Debug, Clone)]
#[command(
    name = "logrix",
    version,
    about = "High-performance modular blockchain indexer"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// PostgreSQL connection URL (supports local K8s Postgres and AWS RDS)
    #[arg(
        long,
        env = "DATABASE_URL",
        default_value = "postgres://logrix:logrix@localhost:5432/logrix"
    )]
    pub database_url: String,

    /// Queue driver to use: "rabbitmq" (default) or "sqs" (AWS SQS)
    #[arg(long, env = "QUEUE_DRIVER", default_value = "rabbitmq")]
    pub queue_driver: String,

    /// RabbitMQ connection URL (used when QUEUE_DRIVER=rabbitmq)
    #[arg(
        long,
        env = "RABBITMQ_URL",
        default_value = "amqp://logrix:logrix@localhost:5672/%2f"
    )]
    pub rabbitmq_url: String,

    /// AWS SQS Live Block Queue URL (used when QUEUE_DRIVER=sqs)
    #[arg(long, env = "SQS_LIVE_URL", default_value = "")]
    pub sqs_live_url: String,

    /// AWS SQS Backfill Queue URL (used when QUEUE_DRIVER=sqs)
    #[arg(long, env = "SQS_BACKFILL_URL", default_value = "")]
    pub sqs_backfill_url: String,

    /// AWS SQS Webhook Queue URL (used when QUEUE_DRIVER=sqs)
    #[arg(long, env = "SQS_WEBHOOK_URL", default_value = "")]
    pub sqs_webhook_url: String,

    /// AWS SQS DLQ Queue URL (used when QUEUE_DRIVER=sqs)
    #[arg(long, env = "SQS_DLQ_URL", default_value = "")]
    pub sqs_dlq_url: String,

    /// Custom AWS endpoint override (e.g. http://localhost:4566 for LocalStack)
    #[arg(long, env = "SQS_ENDPOINT")]
    pub sqs_endpoint: Option<String>,

    /// Target EVM JSON-RPC URL
    #[arg(
        long,
        env = "RPC_URL",
        default_value = "https://sepolia-rollup.arbitrum.io/rpc"
    )]
    pub rpc_url: String,

    /// Secondary / Fallback EVM JSON-RPC URLs (comma-separated)
    #[arg(long, env = "RPC_FALLBACK_URLS", value_delimiter = ',')]
    pub rpc_fallback_urls: Vec<String>,

    /// Compute Unit (CU) budget limit for historical backfill
    #[arg(long, env = "CU_BUDGET")]
    pub cu_budget: Option<u64>,

    /// Bulk stream archive endpoint override (e.g. SQD / HyperSync)
    #[arg(long, env = "BULK_STREAM_URL")]
    pub bulk_stream_url: Option<String>,

    #[arg(long, env = "CHAIN_ID", default_value_t = 421614)]
    pub chain_id: u64,

    /// Target contract address to index (defaults to Arbitrum Sepolia USDC)
    #[arg(
        long,
        env = "CONTRACT_ADDRESS",
        default_value = "0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d"
    )]
    pub contract_address: String,

    /// Initial block to begin indexing if no database checkpoint exists
    #[arg(long, env = "START_BLOCK")]
    pub start_block: Option<u64>,

    /// Target Webhook URL for external reorg and event notifications
    #[arg(long, env = "WEBHOOK_URL")]
    pub webhook_url: Option<String>,

    /// Background gap reconciler polling interval in seconds
    #[arg(long, env = "RECONCILER_INTERVAL_SECS", default_value_t = 30)]
    pub reconciler_interval_secs: u64,

    /// In-memory ring buffer depth for parent-hash reorg detection
    #[arg(long, env = "RING_BUFFER_DEPTH", default_value_t = 128)]
    pub ring_buffer_depth: usize,

    /// Optional path to YAML manifest for declarative mappings and WASM handlers
    #[arg(long, env = "MANIFEST_PATH")]
    pub manifest_path: Option<String>,

    /// Optional path to entity schema definition (schema.graphql or schema.yaml)
    #[arg(long, env = "SCHEMA_PATH")]
    pub schema_path: Option<String>,

    /// GraphQL maximum query depth limit
    #[arg(long, env = "GRAPHQL_MAX_DEPTH", default_value_t = 7)]
    pub graphql_max_depth: usize,

    /// GraphQL maximum query complexity limit
    #[arg(long, env = "GRAPHQL_MAX_COMPLEXITY", default_value_t = 200)]
    pub graphql_max_complexity: usize,

    /// GraphQL default pagination page size limit
    #[arg(long, env = "GRAPHQL_DEFAULT_LIMIT", default_value_t = 100)]
    pub graphql_default_limit: usize,

    /// GraphQL maximum pagination page size limit
    #[arg(long, env = "GRAPHQL_MAX_LIMIT", default_value_t = 1000)]
    pub graphql_max_limit: usize,

    /// Enable Kubernetes Lease active-passive leader election for Ingester
    #[arg(long, env = "ENABLE_LEADER_ELECTION", default_value_t = false)]
    pub enable_leader_election: bool,

    /// Kubernetes Lease name for active-passive ingester HA
    #[arg(long, env = "LEASE_NAME", default_value = "logrix-ingester-lease")]
    pub lease_name: String,

    /// Kubernetes namespace where Lease resource is managed
    #[arg(long, env = "K8S_NAMESPACE", default_value = "default")]
    pub k8s_namespace: String,

    /// Pod or instance identity used in the Kubernetes Lease holderIdentity
    #[arg(long, env = "POD_NAME", default_value = "local-instance")]
    pub pod_name: String,

    /// Opt-in flag to enable outgoing webhook dispatching and worker loop
    #[arg(long, env = "ENABLE_WEBHOOKS", default_value_t = false)]
    pub enable_webhooks: bool,

    /// Webhook signing secret used for default HMAC signature header
    #[arg(long, env = "WEBHOOK_SECRET", default_value = "")]
    pub webhook_secret: String,

    /// AWS S3 bucket for compressed raw block and transfer blob archiving
    #[arg(long, env = "S3_BUCKET")]
    pub s3_bucket: Option<String>,

    /// Custom S3 endpoint override (e.g. MinIO, LocalStack, Cloudflare R2)
    #[arg(long, env = "S3_ENDPOINT")]
    pub s3_endpoint: Option<String>,

    /// Optional S3 key prefix for archived blobs
    #[arg(long, env = "S3_PREFIX")]
    pub s3_prefix: Option<String>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Migrate,
    Ingester,
    Processor,
    WebhookDispatcher,
    Backfill {
        #[arg(long)]
        from_block: u64,
        #[arg(long)]
        to_block: u64,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    Api {
        #[arg(long, env = "PORT", default_value_t = 4000)]
        port: u16,
    },
    AllInOne {
        #[arg(long, env = "PORT", default_value_t = 4000)]
        port: u16,
    },
}
