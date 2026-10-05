use crate::filter_input::{build_filter_input, map_type_ref};
use crate::list_resolver::build_entity_list_field;
use crate::schema_parser::{EntityDef, SchemaDefinition};
use crate::subscriptions::SubscriptionBroadcaster;
use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, Object, Schema, Subscription,
    SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
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

        for entity in &schema_def.entities {
            // Only register storable (non-derived) fields in the GraphQL object
            let entity_obj = build_entity_object(entity);
            let filter_input = build_filter_input(entity);

            builder = builder.register(entity_obj);
            builder = builder.register(filter_input);

            let single_field = build_single_entity_field(entity, store.clone());
            query = query.field(single_field);

            let list_field = build_entity_list_field(entity, store.clone());
            query = query.field(list_field);
        }

        builder = builder.register(query);

        // Wire subscriptions if broadcaster provided
        if let Some(bc) = broadcaster {
            let subscription = build_subscription_type(schema_def, bc);
            builder = builder.register(subscription);
        }

        builder
            .data(store)
            .limit_depth(max_depth)
            .limit_complexity(max_complexity)
            .finish()
    }
}

fn build_entity_object(entity: &EntityDef) -> Object {
    let mut obj = Object::new(&entity.name);

    obj = obj.field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
        FieldFuture::new(async move {
            let row = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
            let id = row.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            Ok(Some(FieldValue::value(id.to_string())))
        })
    }));

    for field in &entity.fields {
        // Skip derived fields -- they are virtual reverse lookups, not stored columns
        if field.derived_from.is_some() {
            continue;
        }
        if field.name == "id" {
            continue;
        }
        let f_name = field.name.clone();
        let f_type = map_type_ref(&field.field_type, field.is_nullable);
        let field_key = f_name.clone();

        obj = obj.field(Field::new(f_name, f_type, move |ctx| {
            let key = field_key.clone();
            FieldFuture::new(async move {
                let row = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
                let val = row.get(&key);
                match val {
                    Some(serde_json::Value::Null) | None => Ok(None),
                    Some(serde_json::Value::String(s)) => Ok(Some(FieldValue::value(s.clone()))),
                    Some(serde_json::Value::Number(n)) => {
                        if let Some(i) = n.as_i64() {
                            Ok(Some(FieldValue::value(i)))
                        } else if let Some(f) = n.as_f64() {
                            Ok(Some(FieldValue::value(f)))
                        } else {
                            Ok(Some(FieldValue::value(n.to_string())))
                        }
                    }
                    Some(serde_json::Value::Bool(b)) => Ok(Some(FieldValue::value(*b))),
                    Some(other) => Ok(Some(FieldValue::value(other.to_string()))),
                }
            })
        }));
    }

    obj
}

fn build_single_entity_field(entity: &EntityDef, store: Arc<PostgresStore>) -> Field {
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
                .unwrap_or(1);

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
                _ => Ok(None),
            }
        })
    })
    .argument(InputValue::new("id", TypeRef::named_nn(TypeRef::ID)))
    .argument(InputValue::new("chainId", TypeRef::named(TypeRef::INT)))
}

/// Build the Subscription root with one field per entity type.
/// Each field streams EntityMutationEvents filtered to that entity type.
fn build_subscription_type(
    schema_def: &SchemaDefinition,
    broadcaster: Arc<SubscriptionBroadcaster>,
) -> Subscription {
    let mut subscription = Subscription::new("Subscription");

    for entity in &schema_def.entities {
        let entity_name = entity.name.clone();
        let entity_name_for_create = entity_name.clone();
        let bc = broadcaster.clone();

        // Field name: e.g. "transfer" -> "onTransfer"
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
