use crate::filter_input::map_type_ref;
use crate::schema_types::EntityDef;
use async_graphql::dynamic::{Field, FieldFuture, FieldValue, Object, TypeRef};

pub(crate) fn build_entity_object(entity: &EntityDef) -> Object {
    let mut obj = Object::new(&entity.name);

    obj = obj.field(Field::new("id", TypeRef::named_nn(TypeRef::ID), |ctx| {
        FieldFuture::new(async move {
            let row = ctx.parent_value.try_downcast_ref::<serde_json::Value>()?;
            let id = row.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            Ok(Some(FieldValue::value(id.to_string())))
        })
    }));

    for field in &entity.fields {
        if field.derived_from.is_some() || field.name == "id" {
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
                        } else if let Some(u) = n.as_u64() {
                            Ok(Some(FieldValue::value(u)))
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
