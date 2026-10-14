use clap::Args;

/// Extended tuning arguments for Logrix runtime components.
#[derive(Args, Debug, Clone)]
pub struct ExtendedArgs {
    /// Background gap reconciler chunk size for slicing missing block intervals
    #[arg(long, env = "RECONCILER_CHUNK_SIZE")]
    pub reconciler_chunk_size: Option<u64>,

    /// Background gap reconciler block lag threshold before reconciliation kicks in
    #[arg(long, env = "RECONCILER_LAG_THRESHOLD", default_value_t = 50)]
    pub reconciler_lag_threshold: u64,

    /// Poll interval in milliseconds when ingester reaches chain head
    #[arg(long, env = "CHAIN_HEAD_POLL_MS", default_value_t = 1500)]
    pub chain_head_poll_ms: u64,

    /// RabbitMQ consumer prefetch QoS limit
    #[arg(long, env = "RABBITMQ_PREFETCH", default_value_t = 100)]
    pub rabbitmq_prefetch: u16,

    /// RabbitMQ connection timeout in seconds
    #[arg(long, env = "RABBITMQ_TIMEOUT_SECS", default_value_t = 10)]
    pub rabbitmq_timeout_secs: u64,

    /// SQS receive message long polling wait time in seconds (0-20)
    #[arg(long, env = "SQS_WAIT_TIME_SECONDS", default_value_t = 2)]
    pub sqs_wait_time_seconds: i32,

    /// SQS receive message max batch count (1-10)
    #[arg(long, env = "SQS_MAX_MESSAGES", default_value_t = 1)]
    pub sqs_max_messages: i32,

    /// WASM engine fuel instruction budget per contract event execution
    #[arg(long, env = "WASM_FUEL_LIMIT", default_value_t = 1_000_000)]
    pub wasm_fuel_limit: u64,

    /// WASM linear memory allocation upper limit in megabytes
    #[arg(long, env = "WASM_MAX_MEMORY_MB", default_value_t = 128)]
    pub wasm_max_memory_mb: u32,

    /// EVM JSON-RPC HTTP client timeout in seconds
    #[arg(long, env = "RPC_TIMEOUT_SECS", default_value_t = 10)]
    pub rpc_timeout_secs: u64,

    /// EVM JSON-RPC circuit breaker consecutive failure trip threshold
    #[arg(long, env = "RPC_CB_THRESHOLD", default_value_t = 5)]
    pub rpc_cb_threshold: u32,

    /// EVM JSON-RPC circuit breaker initial cooldown in seconds
    #[arg(long, env = "RPC_COOLDOWN_SECS", default_value_t = 30)]
    pub rpc_cooldown_secs: u64,

    /// Outgoing Webhook HTTP delivery request timeout in seconds
    #[arg(long, env = "WEBHOOK_TIMEOUT_SECS", default_value_t = 5)]
    pub webhook_timeout_secs: u64,

    /// Outgoing Webhook max delivery retry attempts
    #[arg(long, env = "WEBHOOK_MAX_RETRIES", default_value_t = 1)]
    pub webhook_max_retries: u32,

    /// Webhook background worker idle sleep duration in milliseconds
    #[arg(long, env = "WEBHOOK_WORKER_IDLE_MS", default_value_t = 100)]
    pub webhook_worker_idle_ms: u64,

    /// Webhook background worker error backoff duration in milliseconds
    #[arg(long, env = "WEBHOOK_WORKER_BACKOFF_MS", default_value_t = 500)]
    pub webhook_worker_backoff_ms: u64,

    /// GraphQL query execution timeout in seconds
    #[arg(long, env = "GRAPHQL_QUERY_TIMEOUT_SECS", default_value_t = 5)]
    pub graphql_query_timeout_secs: u64,
}

impl Default for ExtendedArgs {
    fn default() -> Self {
        Self {
            reconciler_chunk_size: None,
            reconciler_lag_threshold: 50,
            chain_head_poll_ms: 1500,
            rabbitmq_prefetch: 100,
            rabbitmq_timeout_secs: 10,
            sqs_wait_time_seconds: 2,
            sqs_max_messages: 1,
            wasm_fuel_limit: 1_000_000,
            wasm_max_memory_mb: 128,
            rpc_timeout_secs: 10,
            rpc_cb_threshold: 5,
            rpc_cooldown_secs: 30,
            webhook_timeout_secs: 5,
            webhook_max_retries: 1,
            webhook_worker_idle_ms: 100,
            webhook_worker_backoff_ms: 500,
            graphql_query_timeout_secs: 5,
        }
    }
}
