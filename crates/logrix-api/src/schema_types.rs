use serde::{Deserialize, Serialize};

/// Supported scalar and reference types for entity fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    Id,
    String,
    Int,
    BigInt,
    Boolean,
    Bytes,
    Float,
    Custom(String),
}

/// Relationship metadata parsed from @derivedFrom directive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedFrom {
    /// The entity type this field is derived from.
    pub entity: String,
    /// The field on the related entity that holds the foreign key.
    pub field: String,
}

/// Single field definition within an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub is_nullable: bool,
    pub is_list: bool,
    /// If Some, this is a virtual reverse-lookup field and must NOT be saved.
    pub derived_from: Option<DerivedFrom>,
    /// If true, an SQL index should be created for this column.
    pub indexed: bool,
}

/// Entity definition containing typed fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

impl EntityDef {
    /// Returns only the storable (non-derived) fields.
    pub fn storable_fields(&self) -> impl Iterator<Item = &FieldDef> {
        self.fields.iter().filter(|f| f.derived_from.is_none())
    }
}

/// Collection of user entities defining an indexer schema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub entities: Vec<EntityDef>,
}

pub fn parse_type_string(s: &str) -> (FieldType, bool, bool) {
    let trimmed = s.trim();
    let is_nullable = !trimmed.ends_with('!');
    let base = trimmed.trim_end_matches('!');

    if let Some(inner) = base.strip_prefix('[').and_then(|b| b.strip_suffix(']')) {
        let (ft, _, _) = parse_type_string(inner);
        (ft, is_nullable, true)
    } else {
        (map_scalar_name(base), is_nullable, false)
    }
}

pub fn map_scalar_name(name: &str) -> FieldType {
    match name {
        "ID" => FieldType::Id,
        "String" => FieldType::String,
        "Int" | "i32" | "u32" => FieldType::Int,
        "BigInt" | "i64" | "u64" | "U256" => FieldType::BigInt,
        "Boolean" | "bool" => FieldType::Boolean,
        "Bytes" | "hex" => FieldType::Bytes,
        "Float" | "f64" => FieldType::Float,
        other => FieldType::Custom(other.to_string()),
    }
}
