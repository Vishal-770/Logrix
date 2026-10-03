use crate::query_builder::FilterOperator;
use crate::schema_parser::{EntityDef, FieldType};
use async_graphql::dynamic::{InputObject, InputValue, TypeRef};

/// Build dynamic GraphQL InputObject for filtering an entity collection.
pub fn build_filter_input(entity: &EntityDef) -> InputObject {
    let filter_name = format!("{}Filter", entity.name);
    let mut input = InputObject::new(filter_name);

    input = input.field(InputValue::new("id", TypeRef::named(TypeRef::STRING)));
    input = input.field(InputValue::new("id_not", TypeRef::named(TypeRef::STRING)));

    for field in &entity.fields {
        if field.name == "id" {
            continue;
        }
        let name = &field.name;
        for op in ["", "_not", "_gt", "_gte", "_lt", "_lte", "_contains"] {
            input = input.field(InputValue::new(
                format!("{name}{op}"),
                TypeRef::named(TypeRef::STRING),
            ));
        }
    }

    input
}

/// Map FieldType to async-graphql TypeRef.
pub fn map_type_ref(ft: &FieldType, is_nullable: bool) -> TypeRef {
    let name = match ft {
        FieldType::Id => TypeRef::ID,
        FieldType::String | FieldType::Bytes => TypeRef::STRING,
        FieldType::Int => TypeRef::INT,
        FieldType::BigInt => TypeRef::STRING,
        FieldType::Boolean => TypeRef::BOOLEAN,
        FieldType::Float => TypeRef::FLOAT,
        FieldType::Custom(s) => s.as_str(),
    };

    if is_nullable {
        TypeRef::named(name)
    } else {
        TypeRef::named_nn(name)
    }
}

/// Parse filter key like `balance_gt` into (`balance`, `FilterOperator::Gt`).
pub fn parse_filter_key(key: &str) -> (String, FilterOperator) {
    if let Some(base) = key.strip_suffix("_not") {
        (base.to_string(), FilterOperator::NotEq)
    } else if let Some(base) = key.strip_suffix("_gte") {
        (base.to_string(), FilterOperator::Gte)
    } else if let Some(base) = key.strip_suffix("_gt") {
        (base.to_string(), FilterOperator::Gt)
    } else if let Some(base) = key.strip_suffix("_lte") {
        (base.to_string(), FilterOperator::Lte)
    } else if let Some(base) = key.strip_suffix("_lt") {
        (base.to_string(), FilterOperator::Lt)
    } else if let Some(base) = key.strip_suffix("_contains") {
        (base.to_string(), FilterOperator::Contains)
    } else {
        (key.to_string(), FilterOperator::Eq)
    }
}
