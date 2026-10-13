pub use crate::validation::validate_identifier;
use crate::validation::validate_numeric_str;
use sqlx::{Postgres, QueryBuilder};

/// Supported filter comparison operators in GraphQL queries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOperator {
    Eq,
    NotEq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
    StartsWith,
    EndsWith,
}

/// A parsed field-level filter condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldFilter {
    pub field: String,
    pub op: FilterOperator,
    pub value: String,
}

/// Dynamic query parameters parsed from GraphQL arguments.
#[derive(Debug, Clone)]
pub struct DynamicQueryParams {
    pub chain_id: u64,
    pub entity_type: String,
    pub filters: Vec<FieldFilter>,
    pub order_by: Option<String>,
    pub order_direction: Option<String>,
    pub first: Option<usize>,
    pub skip: Option<usize>,
    pub after: Option<String>,
    pub default_limit: usize,
    pub max_limit: usize,
}

impl Default for DynamicQueryParams {
    fn default() -> Self {
        Self {
            chain_id: 1,
            entity_type: String::new(),
            filters: Vec::new(),
            order_by: None,
            order_direction: None,
            first: None,
            skip: None,
            after: None,
            default_limit: 100,
            max_limit: 1000,
        }
    }
}

impl DynamicQueryParams {
    /// Build parameterized PostgreSQL query with sanitized field identifiers.
    pub fn build_query(&self) -> Result<QueryBuilder<'static, Postgres>, String> {
        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            "SELECT entity_id, data, block_number, updated_at FROM logrix_entities WHERE chain_id = ",
        );

        builder.push_bind(self.chain_id as i64);
        builder.push(" AND entity_type = ");
        builder.push_bind(self.entity_type.clone());
        builder.push(" AND is_reverted = FALSE");

        // Cursor pagination
        if let Some(ref cursor) = self.after {
            builder.push(" AND entity_id > ");
            builder.push_bind(cursor.clone());
        }

        // Apply filters
        for f in &self.filters {
            validate_identifier(&f.field)?;
            builder.push(" AND (data->>'");
            builder.push(&f.field);
            builder.push("')");

            match f.op {
                FilterOperator::Eq => {
                    builder.push(" = ");
                    builder.push_bind(f.value.clone());
                }
                FilterOperator::NotEq => {
                    builder.push(" != ");
                    builder.push_bind(f.value.clone());
                }
                FilterOperator::Gt => {
                    validate_numeric_str(&f.value)?;
                    builder.push("::numeric > ");
                    builder.push_bind(f.value.clone());
                    builder.push("::numeric");
                }
                FilterOperator::Gte => {
                    validate_numeric_str(&f.value)?;
                    builder.push("::numeric >= ");
                    builder.push_bind(f.value.clone());
                    builder.push("::numeric");
                }
                FilterOperator::Lt => {
                    validate_numeric_str(&f.value)?;
                    builder.push("::numeric < ");
                    builder.push_bind(f.value.clone());
                    builder.push("::numeric");
                }
                FilterOperator::Lte => {
                    validate_numeric_str(&f.value)?;
                    builder.push("::numeric <= ");
                    builder.push_bind(f.value.clone());
                    builder.push("::numeric");
                }
                FilterOperator::Contains => {
                    builder.push(" ILIKE ");
                    builder.push_bind(format!("%{}%", f.value));
                }
                FilterOperator::StartsWith => {
                    builder.push(" ILIKE ");
                    builder.push_bind(format!("{}%", f.value));
                }
                FilterOperator::EndsWith => {
                    builder.push(" ILIKE ");
                    builder.push_bind(format!("%{}", f.value));
                }
            }
        }

        // Sorting
        let dir = match self
            .order_direction
            .as_deref()
            .unwrap_or("asc")
            .to_lowercase()
            .as_str()
        {
            "desc" => "DESC",
            _ => "ASC",
        };

        if let Some(ref order_col) = self.order_by {
            validate_identifier(order_col)?;
            if order_col == "id" || order_col == "entity_id" {
                builder.push(format!(" ORDER BY entity_id {dir}"));
            } else if order_col == "block_number" {
                builder.push(format!(" ORDER BY block_number {dir}"));
            } else {
                builder.push(format!(" ORDER BY (data->>'{order_col}') {dir}"));
            }
        } else {
            builder.push(" ORDER BY block_number DESC, entity_id ASC");
        }

        // Pagination guardrails: configured default and max limit
        let limit = self.first.unwrap_or(self.default_limit).min(self.max_limit) as i64;
        builder.push(" LIMIT ");
        builder.push_bind(limit);

        if let Some(skip) = self.skip {
            builder.push(" OFFSET ");
            builder.push_bind(skip as i64);
        }

        Ok(builder)
    }
}
