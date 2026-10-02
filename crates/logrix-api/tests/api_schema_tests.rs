use logrix_api::HealthStatus;

#[test]
fn test_health_status_construction() {
    let health = HealthStatus {
        status: "UP".to_string(),
        version: "0.1.0".to_string(),
    };
    assert_eq!(health.status, "UP");
    assert_eq!(health.version, "0.1.0");
}
