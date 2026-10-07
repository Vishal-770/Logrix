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
