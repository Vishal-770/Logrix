//! RabbitMQ queue adapter implementing `QueuePort`.
//!
//! Provides AMQP 0-9-1 durable exchanges, bounded prefetch QoS (KEDA autoscaling friendly),
//! and automatic Dead-Letter Queue (DLQ) isolation for poison messages.

pub mod config;
pub mod port;
pub mod producer;
pub mod queue;
pub mod topology;

pub use config::{
    redact_amqp_url, RabbitMQConfig, DEFAULT_EXCHANGE, DEFAULT_PREFETCH, DEFAULT_TIMEOUT,
};
pub use queue::RabbitMQQueue;
pub use topology::QueueTopology;
