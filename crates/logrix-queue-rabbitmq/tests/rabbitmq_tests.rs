use logrix_core::domain::QueueType;
use logrix_queue_rabbitmq::{redact_amqp_url, QueueTopology, RabbitMQConfig, DEFAULT_EXCHANGE};
use std::time::Duration;

#[test]
fn test_rabbitmq_config_builder() {
    let user = "test_user";
    let token = "test_token";
    let host = "rabbitmq-cluster";
    let raw_url = format!("amqp://{user}:{token}@{host}:5672/%2f");

    let config = RabbitMQConfig::new(&raw_url)
        .prefetch_count(250)
        .exchange_name("staging.logrix.events")
        .queue_prefix("staging")
        .connection_timeout(Duration::from_secs(30));

    assert_eq!(config.prefetch_count, 250);
    assert_eq!(config.exchange_name, "staging.logrix.events");
    assert_eq!(config.queue_prefix.as_deref(), Some("staging"));
    assert_eq!(config.connection_timeout, Duration::from_secs(30));
    assert_eq!(
        config.redacted_url(),
        format!("amqp://{user}:***@{host}:5672/%2f")
    );
}

#[test]
fn test_amqp_url_redaction() {
    let u1 = format!("amqp://{}:{}@localhost:5672", "alice", "credential_one");
    assert_eq!(redact_amqp_url(&u1), "amqp://alice:***@localhost:5672");

    let u2 = format!(
        "amqps://{}:{}@broker.cloud:5671/vhost",
        "app", "credential_two"
    );
    assert_eq!(
        redact_amqp_url(&u2),
        "amqps://app:***@broker.cloud:5671/vhost"
    );

    assert_eq!(
        redact_amqp_url("amqp://localhost:5672"),
        "amqp://localhost:5672"
    );
}

#[test]
fn test_queue_topology_default_names() {
    let config = RabbitMQConfig::new("amqp://localhost:5672");
    let topology = QueueTopology::from_config(&config);

    assert_eq!(topology.exchange, DEFAULT_EXCHANGE);
    assert_eq!(topology.live_queue, "logrix.blocks.live");
    assert_eq!(topology.backfill_queue, "logrix.blocks.backfill");
    assert_eq!(topology.webhook_queue, "logrix.blocks.webhook");
    assert_eq!(topology.dlq_queue, "logrix.blocks.dlq");

    let (name, key) = topology.queue_and_key(QueueType::Live);
    assert_eq!(name, "logrix.blocks.live");
    assert_eq!(key, "live");

    let (name, key) = topology.queue_and_key(QueueType::DeadLetter);
    assert_eq!(name, "logrix.blocks.dlq");
    assert_eq!(key, "dlq");
}

#[test]
fn test_queue_topology_prefixed_names() {
    let config = RabbitMQConfig::new("amqp://localhost:5672")
        .queue_prefix("tenant1")
        .exchange_name("tenant1.events");
    let topology = QueueTopology::from_config(&config);

    assert_eq!(topology.exchange, "tenant1.events");
    assert_eq!(topology.live_queue, "tenant1.logrix.blocks.live");
    assert_eq!(topology.backfill_queue, "tenant1.logrix.blocks.backfill");
    assert_eq!(topology.webhook_queue, "tenant1.logrix.blocks.webhook");
    assert_eq!(topology.dlq_queue, "tenant1.logrix.blocks.dlq");
}
