use crate::schema_types::SchemaDefinition;
use async_graphql::dynamic::{
    FieldValue, InputValue, Subscription, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
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
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn broadcast(&self, event: EntityMutationEvent) {
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EntityMutationEvent> {
        self.sender.subscribe()
    }

    pub fn event_stream(&self) -> impl Stream<Item = EntityMutationEvent> {
        use futures::StreamExt;
        BroadcastStream::new(self.subscribe()).filter_map(|res| async move { res.ok() })
    }

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

/// Build the Subscription root with one field per entity type.
pub fn build_subscription_type(
    schema_def: &SchemaDefinition,
    broadcaster: Arc<SubscriptionBroadcaster>,
) -> Subscription {
    let mut subscription = Subscription::new("Subscription");

    for entity in &schema_def.entities {
        let entity_name = entity.name.clone();
        let entity_name_for_create = entity_name.clone();
        let bc = broadcaster.clone();

        let field_name = format!(
            "on{}",
            entity_name
                .chars()
                .enumerate()
                .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                .collect::<String>()
        );

        let sub_field = SubscriptionField::new(
            field_name,
            TypeRef::named(&entity_name_for_create),
            move |_ctx| {
                let bc = bc.clone();
                let filter_type = entity_name.clone();
                SubscriptionFieldFuture::new(async move {
                    use futures::StreamExt;
                    let stream = bc
                        .event_stream()
                        .filter(move |ev| {
                            let matches = ev.entity_type == filter_type;
                            async move { matches }
                        })
                        .map(|ev| -> async_graphql::Result<FieldValue> {
                            Ok(FieldValue::owned_any(ev.data))
                        });
                    Ok(stream)
                })
            },
        )
        .argument(InputValue::new("chainId", TypeRef::named(TypeRef::INT)));

        subscription = subscription.field(sub_field);
    }

    subscription
}
