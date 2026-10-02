use async_trait::async_trait;
use lapin::{
    options::{
        BasicAckOptions, BasicNackOptions, BasicPublishOptions, ExchangeDeclareOptions,
        QueueBindOptions, QueueDeclareOptions,
    },
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};
use logrix_core::{
    domain::{MessageHandle, QueueMessage, QueueType},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
    ports::QueuePort,
};
use std::sync::Arc;
use tracing::{debug, info};

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

    fn queue_name_and_key(queue_type: QueueType) -> (&'static str, &'static str) {
        match queue_type {
            QueueType::Live => (LIVE_QUEUE, "live"),
            QueueType::Backfill => (BACKFILL_QUEUE, "backfill"),
            QueueType::Webhook => (WEBHOOK_QUEUE, "webhook"),
            QueueType::DeadLetter => (DLQ_QUEUE, "dlq"),
        }
    }
}

#[async_trait]
impl QueuePort for RabbitMQQueue {
    async fn publish(&self, queue_type: QueueType, message: &QueueMessage) -> LogrixResult<()> {
        let (_, routing_key) = Self::queue_name_and_key(queue_type);
        let payload = serde_json::to_vec(message).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Failed to serialize queue message: {e}"),
            )
        })?;

        self.channel
            .basic_publish(
                DEFAULT_EXCHANGE,
                routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default().with_delivery_mode(2), // Persistent message
            )
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to publish message: {e}"),
                )
            })?
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("RabbitMQ publish confirmation failed: {e}"),
                )
            })?;

        debug!(queue = ?queue_type, "Published message to RabbitMQ");
        Ok(())
    }

    async fn consume(&self, queue_type: QueueType) -> LogrixResult<Option<MessageHandle>> {
        let (queue_name, _) = Self::queue_name_and_key(queue_type);

        let msg = self
            .channel
            .basic_get(
                queue_name,
                lapin::options::BasicGetOptions { no_ack: false },
            )
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to get message from {queue_name}: {e}"),
                )
            })?;

        match msg {
            Some(delivery) => {
                let message: QueueMessage =
                    serde_json::from_slice(&delivery.data).map_err(|e| {
                        LogrixError::new(
                            ErrorClass::Permanent,
                            ErrorSource::Queue,
                            format!("Failed to deserialize message: {e}"),
                        )
                    })?;

                Ok(Some(MessageHandle::new(
                    delivery.delivery.delivery_tag.to_string(),
                    message,
                )))
            }
            None => Ok(None),
        }
    }

    async fn ack(&self, handle: &MessageHandle) -> LogrixResult<()> {
        let tag: u64 = handle.receipt_id.parse().map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Invalid delivery tag '{}': {e}", handle.receipt_id),
            )
        })?;

        self.channel
            .basic_ack(tag, BasicAckOptions { multiple: false })
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to ack message tag {tag}: {e}"),
                )
            })
    }

    async fn nack(&self, handle: &MessageHandle, requeue: bool) -> LogrixResult<()> {
        let tag: u64 = handle.receipt_id.parse().map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Invalid delivery tag '{}': {e}", handle.receipt_id),
            )
        })?;

        self.channel
            .basic_nack(
                tag,
                BasicNackOptions {
                    requeue,
                    multiple: false,
                },
            )
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to nack message tag {tag}: {e}"),
                )
            })
    }

    async fn dead_letter(&self, handle: &MessageHandle, reason: &str) -> LogrixResult<()> {
        // 1. Publish to DLQ queue with reason
        let dlq_msg = handle.message.clone();
        self.publish(QueueType::DeadLetter, &dlq_msg).await?;

        // 2. Acknowledge original message to remove it from the processing queue
        self.ack(handle).await?;

        info!(
            handle = %handle.receipt_id,
            reason = %reason,
            "Message moved to Dead-Letter Queue"
        );
        Ok(())
    }

    async fn depth(&self, queue_type: QueueType) -> LogrixResult<u64> {
        let (queue_name, _) = Self::queue_name_and_key(queue_type);

        let queue_state = self
            .channel
            .queue_declare(
                queue_name,
                QueueDeclareOptions {
                    passive: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to query depth for {queue_name}: {e}"),
                )
            })?;

        Ok(queue_state.message_count() as u64)
    }
}
