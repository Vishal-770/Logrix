use async_trait::async_trait;
use logrix_core::{
    domain::{MessageHandle, QueueMessage, QueueType},
    error::LogrixResult,
    ports::QueuePort,
};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

/// Thread-safe in-memory QueuePort implementation for tests.
#[derive(Debug, Clone, Default)]
pub struct MockQueuePort {
    queues: Arc<Mutex<HashMap<QueueType, VecDeque<QueueMessage>>>>,
    in_flight: Arc<Mutex<HashMap<String, (QueueType, QueueMessage)>>>,
    pub acked: Arc<Mutex<Vec<String>>>,
    pub nacked: Arc<Mutex<Vec<(String, bool)>>>,
    pub dead_lettered: Arc<Mutex<Vec<(String, String)>>>,
}

impl MockQueuePort {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn clear(&self) {
        self.queues.lock().await.clear();
        self.in_flight.lock().await.clear();
        self.acked.lock().await.clear();
        self.nacked.lock().await.clear();
        self.dead_lettered.lock().await.clear();
    }
}

#[async_trait]
impl QueuePort for MockQueuePort {
    async fn publish(&self, queue_type: QueueType, message: &QueueMessage) -> LogrixResult<()> {
        let mut queues = self.queues.lock().await;
        queues
            .entry(queue_type)
            .or_default()
            .push_back(message.clone());
        Ok(())
    }

    async fn consume(&self, queue_type: QueueType) -> LogrixResult<Option<MessageHandle>> {
        let mut queues = self.queues.lock().await;
        if let Some(msg) = queues.entry(queue_type).or_default().pop_front() {
            let receipt_id = Uuid::new_v4().to_string();
            self.in_flight
                .lock()
                .await
                .insert(receipt_id.clone(), (queue_type, msg.clone()));
            Ok(Some(MessageHandle::new(receipt_id, msg)))
        } else {
            Ok(None)
        }
    }

    async fn ack(&self, handle: &MessageHandle) -> LogrixResult<()> {
        self.in_flight.lock().await.remove(&handle.receipt_id);
        self.acked.lock().await.push(handle.receipt_id.clone());
        Ok(())
    }

    async fn nack(&self, handle: &MessageHandle, requeue: bool) -> LogrixResult<()> {
        if let Some((q_type, msg)) = self.in_flight.lock().await.remove(&handle.receipt_id) {
            if requeue {
                let mut queues = self.queues.lock().await;
                queues.entry(q_type).or_default().push_front(msg);
            }
        }
        self.nacked
            .lock()
            .await
            .push((handle.receipt_id.clone(), requeue));
        Ok(())
    }

    async fn dead_letter(&self, handle: &MessageHandle, reason: &str) -> LogrixResult<()> {
        if let Some((_, msg)) = self.in_flight.lock().await.remove(&handle.receipt_id) {
            let mut queues = self.queues.lock().await;
            queues
                .entry(QueueType::DeadLetter)
                .or_default()
                .push_back(msg);
        }
        self.dead_lettered
            .lock()
            .await
            .push((handle.receipt_id.clone(), reason.to_string()));
        Ok(())
    }

    async fn depth(&self, queue_type: QueueType) -> LogrixResult<u64> {
        let queues = self.queues.lock().await;
        let count = queues.get(&queue_type).map(|q| q.len() as u64).unwrap_or(0);
        Ok(count)
    }
}
