use logrix_core::domain::{QueueMessage, QueueType};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct InFlightEntry {
    pub queue_type: QueueType,
    pub message: QueueMessage,
    pub delivered_at: Instant,
}

pub fn reap_expired(
    in_flight: &mut HashMap<String, InFlightEntry>,
    queues: &mut HashMap<QueueType, VecDeque<QueueMessage>>,
    timeout: Option<Duration>,
) {
    if let Some(vt) = timeout {
        let now = Instant::now();
        let expired: Vec<_> = in_flight
            .iter()
            .filter(|(_, e)| now.duration_since(e.delivered_at) >= vt)
            .map(|(k, e)| (k.clone(), e.queue_type, e.message.clone()))
            .collect();
        for (k, qt, msg) in expired {
            queues.entry(qt).or_default().push_front(msg);
            in_flight.remove(&k);
        }
    }
}
