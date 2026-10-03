use lapin::{
    options::{ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions},
    types::FieldTable,
    Channel, Connection, ConnectionProperties, ExchangeKind,
};
use logrix_core::{
    domain::QueueType,
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};
use std::sync::Arc;
use tracing::info;

pub const DEFAULT_EXCHANGE: &str = "logrix.events";
pub const LIVE_QUEUE: &str = "logrix.blocks.live";
pub const BACKFILL_QUEUE: &str = "logrix.blocks.backfill";
pub const WEBHOOK_QUEUE: &str = "logrix.blocks.webhook";
pub const DLQ_QUEUE: &str = "logrix.blocks.dlq";

/// RabbitMQ AMQP implementation of `QueuePort` with bounded prefetch and manual ACKs.
#[derive(Clone)]
pub struct RabbitMQQueue {
    _conn: Arc<Connection>,
    channel: Arc<Channel>,
}

impl RabbitMQQueue {
    /// Connect to RabbitMQ, declare durable topology, and configure worker prefetch.
    pub async fn connect(amqp_url: &str, prefetch_count: u16) -> LogrixResult<Self> {
        let conn = Connection::connect(amqp_url, ConnectionProperties::default())
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to connect to RabbitMQ: {e}"),
                )
            })?;

        let channel = conn.create_channel().await.map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Queue,
                format!("Failed to create AMQP channel: {e}"),
            )
        })?;

        // 1. Declare direct exchange
        channel
            .exchange_declare(
                DEFAULT_EXCHANGE,
                ExchangeKind::Direct,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Permanent,
                    ErrorSource::Queue,
                    format!("Failed to declare exchange: {e}"),
                )
            })?;

        // 2. Declare durable queues & bindings
        let queues = [
            (LIVE_QUEUE, "live"),
            (BACKFILL_QUEUE, "backfill"),
            (WEBHOOK_QUEUE, "webhook"),
            (DLQ_QUEUE, "dlq"),
        ];

        for (q_name, routing_key) in queues {
            channel
                .queue_declare(
                    q_name,
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Permanent,
                        ErrorSource::Queue,
                        format!("Failed to declare queue {q_name}: {e}"),
                    )
                })?;

            channel
                .queue_bind(
                    q_name,
                    DEFAULT_EXCHANGE,
                    routing_key,
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Permanent,
                        ErrorSource::Queue,
                        format!("Failed to bind queue {q_name}: {e}"),
                    )
                })?;
        }

        // 3. Set prefetch QoS for KEDA-friendly horizontal pod autoscaling
        channel
            .basic_qos(prefetch_count, lapin::options::BasicQosOptions::default())
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to set QoS prefetch: {e}"),
                )
            })?;

        info!(
            prefetch = prefetch_count,
            "RabbitMQ topology initialized successfully"
        );
        Ok(Self {
            _conn: Arc::new(conn),
            channel: Arc::new(channel),
        })
    }

    /// Access internal channel.
    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub(crate) fn queue_name_and_key(queue_type: QueueType) -> (&'static str, &'static str) {
        match queue_type {
            QueueType::Live => (LIVE_QUEUE, "live"),
            QueueType::Backfill => (BACKFILL_QUEUE, "backfill"),
            QueueType::Webhook => (WEBHOOK_QUEUE, "webhook"),
            QueueType::DeadLetter => (DLQ_QUEUE, "dlq"),
        }
    }
}
