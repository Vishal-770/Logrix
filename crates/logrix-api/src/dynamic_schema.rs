use crate::filter_input::{build_filter_input, map_type_ref};
use crate::list_resolver::build_entity_list_field;
use crate::schema_parser::{EntityDef, SchemaDefinition};
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
        let mut builder = Schema::build("Query", None, None);
        let mut query = Object::new("Query");

        query = query.field(Field::new(
            "health",
            TypeRef::named_nn(TypeRef::STRING),
            |_| FieldFuture::new(async { Ok(Some(FieldValue::value("OK"))) }),
        ));

        for entity in &schema_def.entities {
            let entity_obj = build_entity_object(entity);
            let filter_input = build_filter_input(entity);

            builder = builder.register(entity_obj);
            builder = builder.register(filter_input);

            let single_field = build_single_entity_field(entity, store.clone());
            query = query.field(single_field);

            let list_field = build_entity_list_field(entity, store.clone());
            query = query.field(list_field);
        }

        builder
            .register(query)
            .data(store)
            .limit_depth(7)
            .limit_complexity(200)
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
