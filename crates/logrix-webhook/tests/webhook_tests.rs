use logrix_core::domain::WebhookPayload;
use logrix_webhook::{generate_secret, sign_payload, verify_signature, WebhookHttpClient};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_generate_secret_format() {
    let secret = generate_secret();
    assert!(secret.starts_with("whsec_"));
    assert_eq!(secret.len(), 6 + 32);
}

#[test]
fn test_hmac_sign_and_verify_roundtrip() {
    let secret = generate_secret();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let payload = b"{\"event\": \"chain.reorg\", \"depth\": 3}";
    let header = sign_payload(&secret, now, payload);

    assert!(header.starts_with(&format!("t={now},v1=")));

    // Should verify successfully within 300 second tolerance
    let valid = verify_signature(&secret, &header, payload, 300);
    assert!(valid, "Signature should be valid");
}

#[test]
fn test_hmac_verify_rejects_tampered_payload() {
    let secret = generate_secret();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let payload = b"{\"event\": \"chain.reorg\", \"depth\": 3}";
    let header = sign_payload(&secret, now, payload);

    let tampered_payload = b"{\"event\": \"chain.reorg\", \"depth\": 999}";
    let valid = verify_signature(&secret, &header, tampered_payload, 300);
    assert!(!valid, "Tampered payload must fail signature verification");
}

#[test]
fn test_hmac_verify_rejects_expired_timestamp() {
    let secret = generate_secret();
    let old_ts = 1000000; // Far in the past

    let payload = b"{\"event\": \"chain.reorg\"}";
    let header = sign_payload(&secret, old_ts, payload);

    // Tolerance of 300 seconds will reject old timestamp
    let valid = verify_signature(&secret, &header, payload, 300);
    assert!(
        !valid,
        "Old timestamp must be rejected to prevent replay attacks"
    );
}

#[tokio::test]
async fn test_webhook_http_client_handles_unreachable_endpoint() {
    let client = WebhookHttpClient::default();
    let payload = WebhookPayload::new(
        "chain.reorg",
        1,
        serde_json::json!({ "depth": 2, "ancestor": 100 }),
    );

    // Attempting to dispatch to a dead local port
    let res = client
        .dispatch("http://127.0.0.1:59999/nonexistent", "whsec_test", &payload)
        .await;

    assert!(!res.success);
    assert!(res.error_message.is_some());
}
