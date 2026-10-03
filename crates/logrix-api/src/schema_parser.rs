use async_graphql_parser::types::{BaseType, ServiceDocument, Type, TypeDefinition, TypeKind};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SchemaParserError {
    #[error("I/O error reading schema: {0}")]
    Io(#[from] std::io::Error),
    #[error("GraphQL parse error: {0}")]
    GraphQL(String),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Unsupported schema file format: {0}")]
    UnsupportedFormat(String),
}

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

/// Single field definition within an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub is_nullable: bool,
    pub is_list: bool,
}

/// Entity definition containing typed fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

/// Collection of user entities defining an indexer schema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub entities: Vec<EntityDef>,
}

impl SchemaDefinition {
    /// Load and parse schema definition from file (.graphql, .gql, .yaml, .yml).
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, SchemaParserError> {
        let path_ref = path.as_ref();
        let content = std::fs::read_to_string(path_ref)?;
        let ext = path_ref
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "graphql" | "gql" => Self::from_graphql_sdl(&content),
            "yaml" | "yml" => Self::from_yaml(&content),
            _ => Err(SchemaParserError::UnsupportedFormat(ext)),
        }
    }

    /// Parse GraphQL SDL format.
    pub fn from_graphql_sdl(sdl: &str) -> Result<Self, SchemaParserError> {
        let doc: ServiceDocument = async_graphql_parser::parse_schema(sdl)
            .map_err(|e| SchemaParserError::GraphQL(e.to_string()))?;

        let mut entities = Vec::new();

        for def in doc.definitions {
            if let async_graphql_parser::types::TypeSystemDefinition::Type(type_node) = def {
                let type_def: TypeDefinition = type_node.node;
                let type_name = type_def.name.node.to_string();

                // Skip root operations
                if matches!(type_name.as_str(), "Query" | "Mutation" | "Subscription") {
                    continue;
                }

                if let TypeKind::Object(obj_type) = type_def.kind {
                    let mut fields = Vec::new();
                    for field_node in obj_type.fields {
                        let field = field_node.node;
                        let field_name = field.name.node.to_string();
                        let (field_type, is_nullable, is_list) = parse_graphql_type(&field.ty.node);

                        fields.push(FieldDef {
                            name: field_name,
                            field_type,
                            is_nullable,
                            is_list,
                        });
                    }
                    entities.push(EntityDef {
                        name: type_name,
                        fields,
                    });
                }
            }
        }

        Ok(SchemaDefinition { entities })
    }

    /// Parse YAML format.
    pub fn from_yaml(yaml_str: &str) -> Result<Self, SchemaParserError> {
        #[derive(Deserialize)]
        struct YamlField {
            name: String,
            #[serde(rename = "type")]
            field_type: String,
        }

        #[derive(Deserialize)]
        struct YamlEntity {
            name: String,
            fields: Vec<YamlField>,
        }

        #[derive(Deserialize)]
        struct YamlSchema {
            entities: Vec<YamlEntity>,
        }

        let parsed: YamlSchema = serde_yaml::from_str(yaml_str)?;
        let entities = parsed
            .entities
            .into_iter()
            .map(|e| EntityDef {
                name: e.name,
                fields: e
                    .fields
                    .into_iter()
                    .map(|f| {
                        let (ft, nullable, list) = parse_type_string(&f.field_type);
                        FieldDef {
                            name: f.name,
                            field_type: ft,
                            is_nullable: nullable,
                            is_list: list,
                        }
                    })
                    .collect(),
            })
            .collect();

        Ok(SchemaDefinition { entities })
    }
}

fn parse_graphql_type(ty: &Type) -> (FieldType, bool, bool) {
    let is_nullable = ty.nullable;
    match &ty.base {
        BaseType::Named(name) => (map_scalar_name(name.as_str()), is_nullable, false),
        BaseType::List(inner) => {
            let (inner_type, _, _) = parse_graphql_type(inner);
            (inner_type, is_nullable, true)
        }
    }
}

fn parse_type_string(s: &str) -> (FieldType, bool, bool) {
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

fn map_scalar_name(name: &str) -> FieldType {
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
