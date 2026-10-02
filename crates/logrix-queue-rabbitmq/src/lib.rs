//! RabbitMQ queue adapter implementing `QueuePort`.
//!
//! Provides AMQP 0-9-1 durable exchanges, bounded prefetch QoS (KEDA autoscaling friendly),
//! and automatic Dead-Letter Queue (DLQ) isolation for poison messages.

pub mod queue;

pub use queue::{RabbitMQQueue, BACKFILL_QUEUE, DEFAULT_EXCHANGE, DLQ_QUEUE, LIVE_QUEUE};
