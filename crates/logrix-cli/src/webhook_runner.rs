use crate::args::Cli;
use logrix_core::domain::WebhookEndpoint;
use logrix_core::ports::QueuePort;
use logrix_handlers::Manifest;
use std::sync::Arc;
use tracing::info;

pub async fn run_webhook_dispatcher(
    cli: &Cli,
    queue: Arc<dyn QueuePort>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting dedicated Logrix Webhook Dispatcher service");
    let pool = sqlx::PgPool::connect(&cli.database_url).await.ok();
    let store = pool.map(logrix_webhook::WebhookStore::new);

    if let (Some(store_ref), Some(ref path)) = (&store, &cli.manifest_path) {
        seed_manifest_webhooks(store_ref, path).await;
    }

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

async fn seed_manifest_webhooks(store: &logrix_webhook::WebhookStore, path: &str) {
    let Ok(manifest) = Manifest::from_file(path) else { return; };
    let existing = store.list_active_endpoints().await.unwrap_or_default();
    for hook in manifest.webhooks {
        if existing.iter().any(|e| e.url == hook.url) { continue; }
        let secret = hook.secret.unwrap_or_else(logrix_webhook::generate_secret);
        let retries = hook.max_retries.unwrap_or(0);
        let ep = WebhookEndpoint::with_retries(hook.url.clone(), secret, hook.events, retries);
        if store.create_endpoint(&ep).await.is_ok() {
            info!(url = %hook.url, "Seeded webhook endpoint from manifest");
        }
    }
}
