use crate::in_flight::{reap_expired, InFlightEntry};
use async_trait::async_trait;
use logrix_core::{
    domain::{MessageHandle, QueueMessage, QueueType},
    error::{ErrorSource, LogrixError, LogrixResult},
    ports::QueuePort,
};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

/// Thread-safe in-memory QueuePort with fault injection, latency, and visibility timeouts.
#[derive(Debug, Clone, Default)]
pub struct MockQueuePort {
    queues: Arc<Mutex<HashMap<QueueType, VecDeque<QueueMessage>>>>,
    in_flight: Arc<Mutex<HashMap<String, InFlightEntry>>>,
    pub acked: Arc<Mutex<Vec<String>>>,
    pub nacked: Arc<Mutex<Vec<(String, bool)>>>,
    pub dead_lettered: Arc<Mutex<Vec<(String, String)>>>,
    injected_errors: Arc<AtomicUsize>,
    visibility_timeout: Arc<Mutex<Option<Duration>>>,
    simulated_latency: Arc<Mutex<Option<Duration>>>,
    max_capacity: Arc<Mutex<Option<usize>>>,
}

impl MockQueuePort {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn inject_transient_errors(&self, count: usize) {
        self.injected_errors.store(count, Ordering::SeqCst);
    }

    pub async fn set_visibility_timeout(&self, timeout: Duration) {
        *self.visibility_timeout.lock().await = Some(timeout);
    }

    pub async fn set_simulated_latency(&self, latency: Duration) {
        *self.simulated_latency.lock().await = Some(latency);
    }

    pub async fn set_max_capacity(&self, capacity: usize) {
        *self.max_capacity.lock().await = Some(capacity);
    }

    async fn apply_latency_and_check_error(&self) -> LogrixResult<()> {
        let delay = *self.simulated_latency.lock().await;
        if let Some(d) = delay {
            tokio::time::sleep(d).await;
        }
        let mut curr = self.injected_errors.load(Ordering::SeqCst);
        while curr > 0 {
            if self
                .injected_errors
                .compare_exchange_weak(curr, curr - 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Err(LogrixError::transient(
                    ErrorSource::Queue,
                    "Simulated transient broker disconnect",
                ));
            }
            curr = self.injected_errors.load(Ordering::SeqCst);
        }
        Ok(())
    }

    pub async fn reap_expired_in_flight(&self) {
        let timeout = *self.visibility_timeout.lock().await;
        let mut inf = self.in_flight.lock().await;
        let mut q = self.queues.lock().await;
        reap_expired(&mut inf, &mut q, timeout);
    }

    pub async fn clear(&self) {
        self.queues.lock().await.clear();
        self.in_flight.lock().await.clear();
        self.acked.lock().await.clear();
        self.nacked.lock().await.clear();
        self.dead_lettered.lock().await.clear();
        self.injected_errors.store(0, Ordering::SeqCst);
    }
}

#[async_trait]
impl QueuePort for MockQueuePort {
    async fn publish(&self, queue_type: QueueType, message: &QueueMessage) -> LogrixResult<()> {
        self.apply_latency_and_check_error().await?;
        let max_cap = *self.max_capacity.lock().await;
        let mut queues = self.queues.lock().await;
        let q = queues.entry(queue_type).or_default();
        if let Some(cap) = max_cap {
            if q.len() >= cap {
                return Err(LogrixError::rate_limited(
                    ErrorSource::Queue,
                    "Simulated queue buffer full / backpressure rejection",
                    None,
                ));
            }
        }
        q.push_back(message.clone());
        Ok(())
    }

    async fn consume(&self, queue_type: QueueType) -> LogrixResult<Option<MessageHandle>> {
        self.apply_latency_and_check_error().await?;
        self.reap_expired_in_flight().await;
        let maybe_msg = self
            .queues
            .lock()
            .await
            .entry(queue_type)
            .or_default()
            .pop_front();
        if let Some(msg) = maybe_msg {
            let receipt_id = Uuid::new_v4().to_string();
            let entry = InFlightEntry {
                queue_type,
                message: msg.clone(),
                delivered_at: Instant::now(),
            };
            self.in_flight
                .lock()
                .await
                .insert(receipt_id.clone(), entry);
            Ok(Some(MessageHandle::new(receipt_id, msg, queue_type)))
        } else {
            Ok(None)
        }
    }

    async fn ack(&self, handle: &MessageHandle) -> LogrixResult<()> {
        self.apply_latency_and_check_error().await?;
        self.in_flight.lock().await.remove(&handle.receipt_id);
        self.acked.lock().await.push(handle.receipt_id.clone());
        Ok(())
    }

    async fn nack(&self, handle: &MessageHandle, requeue: bool) -> LogrixResult<()> {
        self.apply_latency_and_check_error().await?;
        if let Some(e) = self.in_flight.lock().await.remove(&handle.receipt_id) {
            if requeue {
                self.queues
                    .lock()
                    .await
                    .entry(e.queue_type)
                    .or_default()
                    .push_front(e.message);
            }
        }
        self.nacked
            .lock()
            .await
            .push((handle.receipt_id.clone(), requeue));
        Ok(())
    }

    async fn dead_letter(&self, handle: &MessageHandle, reason: &str) -> LogrixResult<()> {
        self.apply_latency_and_check_error().await?;
        if let Some(e) = self.in_flight.lock().await.remove(&handle.receipt_id) {
            self.queues
                .lock()
                .await
                .entry(QueueType::DeadLetter)
                .or_default()
                .push_back(e.message);
        }
        self.dead_lettered
            .lock()
            .await
            .push((handle.receipt_id.clone(), reason.to_string()));
        Ok(())
    }

    async fn depth(&self, queue_type: QueueType) -> LogrixResult<u64> {
        self.apply_latency_and_check_error().await?;
        self.reap_expired_in_flight().await;
        Ok(self
            .queues
            .lock()
            .await
            .get(&queue_type)
            .map(|q| q.len() as u64)
            .unwrap_or(0))
    }
}
