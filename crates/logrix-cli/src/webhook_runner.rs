use crate::args::Cli;
use logrix_core::ports::QueuePort;
use std::sync::Arc;
use tracing::info;

pub async fn run_webhook_dispatcher(
    cli: &Cli,
    queue: Arc<dyn QueuePort>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting dedicated Logrix Webhook Dispatcher service");
    let pool = sqlx::PgPool::connect(&cli.database_url).await.ok();
    let store = pool.map(logrix_webhook::WebhookStore::new);
    let secret = if cli.webhook_secret.is_empty() {
        logrix_webhook::generate_secret()
    } else {
        cli.webhook_secret.clone()
    };
    let dispatcher = logrix_webhook::WebhookDispatcherService::new(
        queue,
        store,
        cli.webhook_url.clone(),
        secret,
    );
    dispatcher.run_loop().await;
    Ok(())
}
