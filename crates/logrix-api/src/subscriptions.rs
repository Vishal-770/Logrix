use futures::Stream;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgListener;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{error, info, warn};

/// Real-time entity mutation event broadcast to WebSocket subscribers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMutationEvent {
    pub chain_id: u64,
    pub entity_type: String,
    pub entity_id: String,
    pub block_number: u64,
    pub data: serde_json::Value,
}

/// Broadcaster managing non-blocking broadcast channels and Postgres LISTEN bridges.
#[derive(Clone)]
pub struct SubscriptionBroadcaster {
    sender: broadcast::Sender<EntityMutationEvent>,
}

impl Default for SubscriptionBroadcaster {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl SubscriptionBroadcaster {
    /// Create new broadcaster with specified buffer capacity.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Broadcast an entity mutation event to all active subscribers.
    pub fn broadcast(&self, event: EntityMutationEvent) {
        let _ = self.sender.send(event);
    }

    /// Subscribe to entity mutation stream.
    pub fn subscribe(&self) -> broadcast::Receiver<EntityMutationEvent> {
        self.sender.subscribe()
    }

    /// Create an async stream of events for GraphQL subscriptions.
    pub fn event_stream(&self) -> impl Stream<Item = EntityMutationEvent> {
        use futures::StreamExt;
        BroadcastStream::new(self.subscribe()).filter_map(|res| async move { res.ok() })
    }

    /// Spawn a background task listening to PostgreSQL pg_notify channel.
    pub fn start_postgres_listener(self: Arc<Self>, pool: PgPool) {
        tokio::spawn(async move {
            info!("Starting PostgreSQL LISTEN background task on channel 'logrix_entity_mutations'...");
            loop {
                let mut listener = match PgListener::connect_with(&pool).await {
                    Ok(l) => l,
                    Err(e) => {
                        error!(error = %e, "Failed to connect PgListener for subscriptions, retrying in 3s...");
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        continue;
                    }
                };

                if let Err(e) = listener.listen("logrix_entity_mutations").await {
                    error!(error = %e, "Failed to LISTEN on logrix_entity_mutations, retrying in 3s...");
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    continue;
                }

                while let Ok(notification) = listener.recv().await {
                    let payload_str = notification.payload();
                    if let Ok(event) = serde_json::from_str::<EntityMutationEvent>(payload_str) {
                        self.broadcast(event);
                    }
                }

                warn!("PgListener disconnected, reconnecting in 2s...");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
    }
}
