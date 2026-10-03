use crate::filter_input::parse_filter_key;
use crate::query_builder::{DynamicQueryParams, FieldFilter};
use crate::schema_parser::EntityDef;
use async_graphql::dynamic::{Field, FieldFuture, FieldValue, InputValue, TypeRef};
use logrix_store_postgres::PostgresStore;
use std::sync::Arc;

/// Build dynamic collection query field: e.g. accounts(where: AccountFilter, first: Int, ...)
pub fn build_entity_list_field(entity: &EntityDef, store: Arc<PostgresStore>) -> Field {
    let list_query_name = format!("{}s", entity.name.to_lowercase());
    let entity_name = entity.name.clone();
    let filter_name = format!("{}Filter", entity.name);

    Field::new(
        list_query_name,
        TypeRef::named_nn_list_nn(&entity.name),
        move |ctx| {
            let entity_name = entity_name.clone();
            let store = store.clone();
            FieldFuture::new(async move {
                let chain_id = ctx
                    .args
                    .get("chainId")
                    .and_then(|v| v.u64().ok())
                    .unwrap_or(1);
                let first = ctx
                    .args
                    .get("first")
                    .and_then(|v| v.u64().ok())
                    .map(|v| v as usize);
                let skip = ctx
                    .args
                    .get("skip")
                    .and_then(|v| v.u64().ok())
                    .map(|v| v as usize);
                let after = ctx
                    .args
                    .get("after")
                    .and_then(|v| v.string().ok())
                    .map(|s| s.to_string());
                let order_by = ctx
                    .args
                    .get("orderBy")
                    .and_then(|v| v.string().ok())
                    .map(|s| s.to_string());
                let order_dir = ctx
                    .args
                    .get("orderDirection")
                    .and_then(|v| v.string().ok())
                    .map(|s| s.to_string());

                let mut filters = Vec::new();
                if let Some(where_arg) = ctx.args.get("where") {
                    if let Ok(obj) = where_arg.object() {
                        for (k, v) in obj.iter() {
                            if let Ok(val_str) = v.string() {
                                let (field, op) = parse_filter_key(k);
                                filters.push(FieldFilter {
                                    field,
                                    op,
                                    value: val_str.to_string(),
                                });
                            }
                        }
                    }
                }

                let params = DynamicQueryParams {
                    chain_id,
                    entity_type: entity_name,
                    filters,
                    order_by,
                    order_direction: order_dir,
                    first,
                    skip,
                    after,
                };

                let builder = match params.build_query() {
                    Ok(b) => b,
                    Err(e) => return Err(async_graphql::Error::new(e)),
                };

                let query_str = builder.into_sql();
                let records = store
                    .query_entities_raw(&query_str)
                    .await
                    .unwrap_or_default();

                let results: Vec<FieldValue> = records
                    .into_iter()
                    .map(|r| {
                        let mut data = r.data;
                        if let Some(obj) = data.as_object_mut() {
                            obj.insert("id".to_string(), serde_json::Value::String(r.entity_id));
                        }
                        FieldValue::owned_any(data)
                    })
                    .collect();

                Ok(Some(FieldValue::list(results)))
            })
        },
    )
    .argument(InputValue::new("where", TypeRef::named(&filter_name)))
    .argument(InputValue::new("orderBy", TypeRef::named(TypeRef::STRING)))
    .argument(InputValue::new(
        "orderDirection",
        TypeRef::named(TypeRef::STRING),
    ))
    .argument(InputValue::new("first", TypeRef::named(TypeRef::INT)))
    .argument(InputValue::new("skip", TypeRef::named(TypeRef::INT)))
    .argument(InputValue::new("after", TypeRef::named(TypeRef::STRING)))
    .argument(InputValue::new("chainId", TypeRef::named(TypeRef::INT)))
}
