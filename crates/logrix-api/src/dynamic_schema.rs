use crate::entity_object::build_entity_object;
use crate::filter_input::build_filter_input;
use crate::list_resolver::build_entity_list_field;
use crate::schema_types::{EntityDef, SchemaDefinition};
use crate::subscriptions::{build_subscription_type, SubscriptionBroadcaster};
use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, Object, Schema, TypeRef};
use logrix_store_postgres::PostgresStore;
use std::sync::Arc;

/// Dynamic schema engine that compiles user-defined entities into an async-graphql schema.
pub struct DynamicSchemaEngine;

impl DynamicSchemaEngine {
    /// Build a complete executable GraphQL schema from schema definition and PostgreSQL store.
    pub fn build(
        schema_def: &SchemaDefinition,
        store: Arc<PostgresStore>,
    ) -> Result<Schema, async_graphql::dynamic::SchemaError> {
        Self::build_with_config(schema_def, store, None, 7, 200)
    }

    /// Build dynamic schema with custom query depth, complexity limits, and optional subscriptions.
    pub fn build_with_limits(
        schema_def: &SchemaDefinition,
        store: Arc<PostgresStore>,
        max_depth: usize,
        max_complexity: usize,
    ) -> Result<Schema, async_graphql::dynamic::SchemaError> {
        Self::build_with_config(schema_def, store, None, max_depth, max_complexity)
    }

    /// Build dynamic schema with optional subscription broadcaster.
    pub fn build_with_config(
        schema_def: &SchemaDefinition,
        store: Arc<PostgresStore>,
        broadcaster: Option<Arc<SubscriptionBroadcaster>>,
        max_depth: usize,
        max_complexity: usize,
    ) -> Result<Schema, async_graphql::dynamic::SchemaError> {
        let cfg = crate::server::ApiConfig {
            max_depth,
            max_complexity,
            ..Default::default()
        };
        Self::build_with_api_config(schema_def, store, broadcaster, &cfg)
    }

    /// Build dynamic schema with full ApiConfig settings.
    pub fn build_with_api_config(
        schema_def: &SchemaDefinition,
        store: Arc<PostgresStore>,
        broadcaster: Option<Arc<SubscriptionBroadcaster>>,
        cfg: &crate::server::ApiConfig,
    ) -> Result<Schema, async_graphql::dynamic::SchemaError> {
        let has_subscriptions = broadcaster.is_some();
        let mut builder = if has_subscriptions {
            Schema::build("Query", None, Some("Subscription"))
        } else {
            Schema::build("Query", None, None)
        };
        let mut query = Object::new("Query");

        query = query.field(Field::new(
            "health",
            TypeRef::named_nn(TypeRef::STRING),
            |_| FieldFuture::new(async { Ok(Some(FieldValue::value("OK"))) }),
        ));

        let default_chain = cfg.chain_id.unwrap_or(1);
        let default_limit = cfg.default_limit;
        let max_limit = cfg.max_limit;

        for entity in &schema_def.entities {
            let entity_obj = build_entity_object(entity);
            let filter_input = build_filter_input(entity);

            builder = builder.register(entity_obj);
            builder = builder.register(filter_input);

            let single_field = build_single_entity_field(entity, store.clone(), default_chain);
            query = query.field(single_field);

            let list_field = build_entity_list_field(
                entity,
                store.clone(),
                default_chain,
                default_limit,
                max_limit,
            );
            query = query.field(list_field);
        }

        builder = builder.register(query);

        if let Some(bc) = broadcaster {
            let subscription = build_subscription_type(schema_def, bc);
            builder = builder.register(subscription);
        }

        builder
            .data(store)
            .limit_depth(cfg.max_depth)
            .limit_complexity(cfg.max_complexity)
            .finish()
    }
}

fn build_single_entity_field(
    entity: &EntityDef,
    store: Arc<PostgresStore>,
    default_chain: u64,
) -> Field {
    let query_name = entity.name.to_lowercase();
    let entity_type = entity.name.clone();

    Field::new(query_name, TypeRef::named(&entity.name), move |ctx| {
        let entity_type = entity_type.clone();
        let store = store.clone();
        FieldFuture::new(async move {
            let id = ctx
                .args
                .get("id")
                .and_then(|v| v.string().ok())
                .unwrap_or_default();
            let chain_id = ctx
                .args
                .get("chainId")
                .and_then(|v| v.u64().ok())
                .unwrap_or(default_chain);

            match store.get_entity(chain_id, &entity_type, id).await {
                Ok(Some(record)) => {
                    let mut data = record.data;
                    if let Some(obj) = data.as_object_mut() {
                        obj.insert(
                            "id".to_string(),
                            serde_json::Value::String(record.entity_id),
                        );
                    }
                    Ok(Some(FieldValue::owned_any(data)))
                }
                Ok(None) => Ok(None),
                Err(e) => {
                    tracing::error!(error = %e, "Entity fetch failed");
                    Err(async_graphql::Error::new(format!(
                        "Entity fetch failed: {e}"
                    )))
                }
            }
        })
    })
    .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID)))
    .argument(InputValue::new("chainId", TypeRef::named(TypeRef::INT)))
}
