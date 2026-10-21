use logrix_core::domain::{WebhookDelivery, WebhookEndpoint};
use logrix_webhook::generate_secret;
use uuid::Uuid;

#[test]
fn test_endpoint_secret_masking() {
    let ep = WebhookEndpoint::new(
        "https://example.com/webhook",
        "whsec_1234567890abcdef1234567890abcdef",
        vec!["reorg".to_string()],
    );
    let masked = ep.masked_secret();
    assert!(masked.starts_with("whsec_"));
    assert!(masked.contains("..."));
    assert!(!masked.contains("1234567890abcdef"));
}

#[test]
fn test_endpoint_events_filter_matching() {
    let ep = WebhookEndpoint::new(
        "https://example.com/webhook",
        generate_secret(),
        vec!["reorg".to_string(), "Transfer".to_string()],
    );
    assert!(ep.events.contains(&"reorg".to_string()));
    assert!(ep.events.contains(&"Transfer".to_string()));
    assert!(!ep.events.contains(&"Approval".to_string()));
}

#[test]
fn test_webhook_delivery_struct_creation() {
    let id = Uuid::new_v4();
    let delivery = WebhookDelivery {
        id,
        endpoint_id: Some(Uuid::new_v4()),
        event_type: "reorg".into(),
        payload: serde_json::json!({ "depth": 2 }),
        status_code: Some(200),
        success: true,
        attempt: 2,
        is_retry: true,
        error_message: None,
        latency_ms: 45,
        created_at: chrono::Utc::now(),
    };
    assert_eq!(delivery.id, id);
    assert!(delivery.success);
    assert!(delivery.is_retry);
    assert_eq!(delivery.attempt, 2);
    assert_eq!(delivery.status_code, Some(200));
}

#[test]
fn test_endpoint_with_custom_retries() {
    let ep = WebhookEndpoint::with_retries(
        "https://example.com/webhook",
        generate_secret(),
        vec!["Transfer".to_string()],
        3,
    );
    assert_eq!(ep.max_retries, 3);
    assert!(ep.is_active);
}

#[test]
fn test_webhook_config_defaults_and_builder() {
    let cfg = logrix_webhook::WebhookConfig::default();
    assert_eq!(cfg.timeout, logrix_webhook::DEFAULT_TIMEOUT);
    assert_eq!(cfg.max_retries, logrix_webhook::DEFAULT_MAX_RETRIES);
    assert_eq!(
        cfg.worker_idle_sleep,
        logrix_webhook::DEFAULT_WORKER_IDLE_SLEEP
    );
    assert_eq!(
        cfg.worker_error_backoff,
        logrix_webhook::DEFAULT_WORKER_ERROR_BACKOFF
    );
    assert_eq!(
        cfg.pool_idle_timeout,
        logrix_webhook::DEFAULT_POOL_IDLE_TIMEOUT
    );
    assert_eq!(
        cfg.pool_max_idle_per_host,
        logrix_webhook::DEFAULT_POOL_MAX_IDLE_PER_HOST
    );
    assert_eq!(
        cfg.max_concurrent_dispatches,
        logrix_webhook::DEFAULT_MAX_CONCURRENT_DISPATCHES
    );

    let customized = cfg
        .with_timeout(std::time::Duration::from_secs(12))
        .with_max_retries(4)
        .with_max_concurrent_dispatches(100)
        .with_worker_idle_sleep(std::time::Duration::from_millis(50))
        .with_worker_error_backoff(std::time::Duration::from_millis(250));

    assert_eq!(customized.timeout, std::time::Duration::from_secs(12));
    assert_eq!(customized.max_retries, 4);
    assert_eq!(customized.max_concurrent_dispatches, 100);
    assert_eq!(
        customized.worker_idle_sleep,
        std::time::Duration::from_millis(50)
    );
    assert_eq!(
        customized.worker_error_backoff,
        std::time::Duration::from_millis(250)
    );
}

#[tokio::test]
async fn test_concurrent_fan_out_does_not_block() {
    let client = logrix_webhook::WebhookHttpClient::new(std::time::Duration::from_millis(100), 1);
    let payload = logrix_core::domain::WebhookPayload::new(
        "Transfer",
        1,
        serde_json::json!({ "from": "0x1", "to": "0x2" }),
    );

    let ep_dead = logrix_core::domain::WebhookEndpoint::with_retries(
        "http://127.0.0.1:59998/webhook",
        "whsec_dead",
        vec!["Transfer".into()],
        0,
    );

    // Endpoint dispatch fails cleanly without hanging
    let start = std::time::Instant::now();
    let res = client
        .dispatch_with_retries(&ep_dead.url, &ep_dead.secret, &payload, 1)
        .await;
    assert!(!res.success);
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}
