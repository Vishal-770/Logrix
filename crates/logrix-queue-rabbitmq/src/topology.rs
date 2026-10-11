use crate::config::RabbitMQConfig;
use lapin::{
    options::{ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions},
    types::FieldTable,
    Channel, ExchangeKind,
};
use logrix_core::{
    domain::QueueType,
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};

/// Resolved physical queue topology names and bindings.
#[derive(Debug, Clone)]
pub struct QueueTopology {
    pub exchange: String,
    pub live_queue: String,
    pub backfill_queue: String,
    pub webhook_queue: String,
    pub dlq_queue: String,
}

impl QueueTopology {
    pub fn from_config(config: &RabbitMQConfig) -> Self {
        let prefix = config
            .queue_prefix
            .as_deref()
            .map(|p| format!("{}.", p.trim_end_matches('.')))
            .unwrap_or_default();

        Self {
            exchange: config.exchange_name.clone(),
            live_queue: format!("{prefix}logrix.blocks.live"),
            backfill_queue: format!("{prefix}logrix.blocks.backfill"),
            webhook_queue: format!("{prefix}logrix.blocks.webhook"),
            dlq_queue: format!("{prefix}logrix.blocks.dlq"),
        }
    }

    pub fn queue_and_key(&self, queue_type: QueueType) -> (&str, &'static str) {
        match queue_type {
            QueueType::Live => (&self.live_queue, "live"),
            QueueType::Backfill => (&self.backfill_queue, "backfill"),
            QueueType::Webhook => (&self.webhook_queue, "webhook"),
            QueueType::DeadLetter => (&self.dlq_queue, "dlq"),
        }
    }

    /// Declare exchange, queues, and bindings idempotently on the channel.
    pub async fn declare(&self, channel: &Channel) -> LogrixResult<()> {
        channel
            .exchange_declare(
                &self.exchange,
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
                    format!("Failed to declare exchange '{}': {e}", self.exchange),
                )
            })?;

        let queues = [
            (&self.live_queue, "live"),
            (&self.backfill_queue, "backfill"),
            (&self.webhook_queue, "webhook"),
            (&self.dlq_queue, "dlq"),
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
                        format!("Failed to declare queue '{q_name}': {e}"),
                    )
                })?;

            channel
                .queue_bind(
                    q_name,
                    &self.exchange,
                    routing_key,
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Permanent,
                        ErrorSource::Queue,
                        format!(
                            "Failed to bind queue '{q_name}' to '{}': {e}",
                            self.exchange
                        ),
                    )
                })?;
        }

        Ok(())
    }
}
