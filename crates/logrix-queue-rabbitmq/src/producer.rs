use lapin::{options::BasicPublishOptions, BasicProperties, Channel};
use logrix_core::{
    domain::{QueueMessage, QueueType},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};
use tracing::debug;

/// Publish a single persistent AMQP message and await confirmation.
pub async fn publish_single(
    channel: &Channel,
    exchange: &str,
    routing_key: &str,
    queue_type: QueueType,
    message: &QueueMessage,
) -> LogrixResult<()> {
    let payload = serde_json::to_vec(message).map_err(|e| {
        LogrixError::new(
            ErrorClass::Permanent,
            ErrorSource::Queue,
            format!("Failed to serialize queue message: {e}"),
        )
    })?;

    channel
        .basic_publish(
            exchange,
            routing_key,
            BasicPublishOptions::default(),
            &payload,
            BasicProperties::default().with_delivery_mode(2),
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

/// Pipeline a batch of persistent AMQP messages and await confirmations concurrently.
pub async fn publish_many(
    channel: &Channel,
    exchange: &str,
    routing_key: &str,
    queue_type: QueueType,
    messages: &[QueueMessage],
) -> LogrixResult<()> {
    if messages.is_empty() {
        return Ok(());
    }

    let mut confirms = Vec::with_capacity(messages.len());

    for msg in messages {
        let payload = serde_json::to_vec(msg).map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Failed to serialize queue message: {e}"),
            )
        })?;

        let confirm = channel
            .basic_publish(
                exchange,
                routing_key,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default().with_delivery_mode(2),
            )
            .await
            .map_err(|e| {
                LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("Failed to publish batch message: {e}"),
                )
            })?;

        confirms.push(confirm);
    }

    for confirm in confirms {
        confirm.await.map_err(|e| {
            LogrixError::new(
                ErrorClass::Transient,
                ErrorSource::Queue,
                format!("RabbitMQ batch publish confirmation failed: {e}"),
            )
        })?;
    }

    debug!(queue = ?queue_type, count = messages.len(), "Batch published to RabbitMQ");
    Ok(())
}
