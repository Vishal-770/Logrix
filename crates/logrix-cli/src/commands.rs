use clap::Subcommand;

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Initialize a new indexer project with interactive wizard
    Init {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        network: Option<String>,
        #[arg(long)]
        contract: Option<String>,
        #[arg(long)]
        start_block: Option<u64>,
        #[arg(long)]
        logic: Option<String>,
        #[arg(long, default_value_t = false)]
        non_interactive: bool,
    },
    /// Live terminal status dashboard monitoring indexer health
    Status {
        #[arg(long, default_value_t = false)]
        watch: bool,
    },
    /// Run database migrations
    Migrate,
    /// Run active-passive Ingester node
    Ingester,
    /// Run Processor consumer worker
    Processor,
    /// Run Webhook Dispatcher service
    WebhookDispatcher,
    /// Run historical range backfill
    Backfill {
        #[arg(long)]
        from_block: u64,
        #[arg(long)]
        to_block: u64,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    /// Run GraphQL API server
    Api {
        #[arg(long, env = "PORT", default_value_t = 4000)]
        port: u16,
    },
    /// Run all-in-one local indexer process
    AllInOne {
        #[arg(long, env = "PORT", default_value_t = 4000)]
        port: u16,
    },
    /// 1-Click deployment for Local Docker or AWS EKS with auto-compiled WASM handlers
    Deploy {
        /// Deploy to local Docker environment (PostgreSQL, RabbitMQ, and indexer engine)
        #[arg(long)]
        local: bool,
        /// Deploy to AWS production environment (Terraform + EKS Helm)
        #[arg(long)]
        aws: bool,
    },
}
