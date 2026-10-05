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
