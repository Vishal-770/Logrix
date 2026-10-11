use logrix_api::routes::webhook_ops::validate_webhook_url;

#[test]
fn test_valid_public_webhook_urls() {
    assert!(validate_webhook_url("https://example.com/webhook").is_ok());
    assert!(validate_webhook_url("https://hooks.slack.com/services/T00/B00/X00").is_ok());
    assert!(validate_webhook_url("http://93.184.216.34:8080/hook").is_ok());
    assert!(validate_webhook_url("https://my-service.org/events?env=prod").is_ok());
}

#[test]
fn test_rejects_loopback_and_local_hosts() {
    assert!(validate_webhook_url("http://127.0.0.1:8080/events").is_err());
    assert!(validate_webhook_url("http://127.0.0.2/hook").is_err());
    assert!(validate_webhook_url("http://localhost:8080/events").is_err());
    assert!(validate_webhook_url("http://sub.localhost:3000/api").is_err());
    assert!(validate_webhook_url("http://[::1]:8080/hook").is_err());
    assert!(validate_webhook_url("http://cluster.internal/webhook").is_err());
    assert!(validate_webhook_url("http://gateway.local/webhook").is_err());
}

#[test]
fn test_rejects_private_rfc1918_addresses() {
    // 10.0.0.0/8
    assert!(validate_webhook_url("http://10.0.0.1/webhook").is_err());
    assert!(validate_webhook_url("http://10.255.255.255/webhook").is_err());

    // 172.16.0.0/12
    assert!(validate_webhook_url("http://172.16.0.1:9000/hook").is_err());
    assert!(validate_webhook_url("http://172.31.255.255:9000/hook").is_err());

    // 192.168.0.0/16
    assert!(validate_webhook_url("http://192.168.1.1/hook").is_err());
    assert!(validate_webhook_url("http://192.168.0.254:8080/hook").is_err());
}

#[test]
fn test_rejects_cloud_metadata_link_local() {
    // AWS/GCP/Azure link-local metadata IP
    assert!(validate_webhook_url("http://169.254.169.254/latest/meta-data").is_err());
    assert!(validate_webhook_url("http://169.254.1.1/hook").is_err());
}

#[test]
fn test_rejects_non_http_schemes_and_invalid_urls() {
    assert!(validate_webhook_url("ftp://example.com/hooks").is_err());
    assert!(validate_webhook_url("file:///etc/passwd").is_err());
    assert!(validate_webhook_url("gopher://example.com").is_err());
    assert!(validate_webhook_url("/relative/path").is_err());
    assert!(validate_webhook_url("not-a-valid-url").is_err());
    assert!(validate_webhook_url("https://").is_err());
}
