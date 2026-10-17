use crate::queue::SqsQueue;
use aws_sdk_sqs::types::{DeleteMessageBatchRequestEntry, SendMessageBatchRequestEntry};
use logrix_core::{
    domain::{MessageHandle, QueueMessage, QueueType},
    error::{ErrorClass, ErrorSource, LogrixError, LogrixResult},
};

pub const SQS_MAX_BATCH_SIZE: usize = 10;

/// Prepare an entry for SQS `SendMessageBatch`.
pub fn build_send_batch_entry(
    id: &str,
    message: &QueueMessage,
) -> LogrixResult<SendMessageBatchRequestEntry> {
    let body = serde_json::to_string(message).map_err(|e| {
        LogrixError::new(
            ErrorClass::Permanent,
            ErrorSource::Queue,
            format!("Failed to serialize batch message: {e}"),
        )
    })?;

    SendMessageBatchRequestEntry::builder()
        .id(id)
        .message_body(body)
        .build()
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Failed to build SQS batch entry: {e}"),
            )
        })
}

/// Prepare an entry for SQS `DeleteMessageBatch`.
pub fn build_delete_batch_entry(
    id: &str,
    receipt_handle: &str,
) -> LogrixResult<DeleteMessageBatchRequestEntry> {
    DeleteMessageBatchRequestEntry::builder()
        .id(id)
        .receipt_handle(receipt_handle)
        .build()
        .map_err(|e| {
            LogrixError::new(
                ErrorClass::Permanent,
                ErrorSource::Queue,
                format!("Failed to build SQS delete batch entry: {e}"),
            )
        })
}

impl SqsQueue {
    /// Batch publish messages up to AWS SQS limit (10 items per batch).
    pub async fn publish_batch(
        &self,
        queue_type: QueueType,
        messages: &[QueueMessage],
    ) -> LogrixResult<()> {
        let url = self.config().resolve_url(queue_type)?;
        for chunk in messages.chunks(SQS_MAX_BATCH_SIZE) {
            let mut entries = Vec::with_capacity(chunk.len());
            for (idx, msg) in chunk.iter().enumerate() {
                entries.push(build_send_batch_entry(&format!("m_{idx}"), msg)?);
            }
            let output = self
                .client()
                .send_message_batch()
                .queue_url(url)
                .set_entries(Some(entries))
                .send()
                .await
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Transient,
                        ErrorSource::Queue,
                        format!("SQS batch send failed: {e}"),
                    )
                })?;
            if !output.failed().is_empty() {
                let count = output.failed().len();
                return Err(LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("SQS batch send encountered {count} failed entries"),
                ));
            }
        }
        Ok(())
    }

    /// Batch acknowledge messages up to AWS SQS limit (10 items per batch).
    pub async fn ack_batch(&self, handles: &[MessageHandle]) -> LogrixResult<()> {
        for chunk in handles.chunks(SQS_MAX_BATCH_SIZE) {
            let target_queue = chunk[0].source_queue;
            if chunk
                .iter()
                .any(|handle| handle.source_queue != target_queue)
            {
                return Err(LogrixError::validation(
                    ErrorSource::Queue,
                    "SQS batch acknowledge cannot mix source queues",
                ));
            }
            let url = self.config().resolve_url(target_queue)?;
            let mut entries = Vec::with_capacity(chunk.len());
            for (idx, h) in chunk.iter().enumerate() {
                entries.push(build_delete_batch_entry(
                    &format!("d_{idx}"),
                    &h.receipt_id,
                )?);
            }
            let output = self
                .client()
                .delete_message_batch()
                .queue_url(url)
                .set_entries(Some(entries))
                .send()
                .await
                .map_err(|e| {
                    LogrixError::new(
                        ErrorClass::Transient,
                        ErrorSource::Queue,
                        format!("SQS batch delete failed: {e}"),
                    )
                })?;
            if !output.failed().is_empty() {
                let count = output.failed().len();
                return Err(LogrixError::new(
                    ErrorClass::Transient,
                    ErrorSource::Queue,
                    format!("SQS batch delete encountered {count} failed entries"),
                ));
            }
        }
        Ok(())
    }
}
