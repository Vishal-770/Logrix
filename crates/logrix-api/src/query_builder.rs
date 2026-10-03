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
#[derive(Debug, Clone, Default)]
pub struct DynamicQueryParams {
    pub chain_id: u64,
    pub entity_type: String,
    pub filters: Vec<FieldFilter>,
    pub order_by: Option<String>,
    pub order_direction: Option<String>,
    pub first: Option<usize>,
    pub skip: Option<usize>,
    pub after: Option<String>,
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
                    builder.push("::numeric > ");
                    let num = f
                        .value
                        .parse::<f64>()
                        .map_err(|_| "Invalid numeric value for _gt filter")?;
                    builder.push_bind(num);
                }
                FilterOperator::Gte => {
                    builder.push("::numeric >= ");
                    let num = f
                        .value
                        .parse::<f64>()
                        .map_err(|_| "Invalid numeric value for _gte filter")?;
                    builder.push_bind(num);
                }
                FilterOperator::Lt => {
                    builder.push("::numeric < ");
                    let num = f
                        .value
                        .parse::<f64>()
                        .map_err(|_| "Invalid numeric value for _lt filter")?;
                    builder.push_bind(num);
                }
                FilterOperator::Lte => {
                    builder.push("::numeric <= ");
                    let num = f
                        .value
                        .parse::<f64>()
                        .map_err(|_| "Invalid numeric value for _lte filter")?;
                    builder.push_bind(num);
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

        // Pagination guardrails: default 100, max 1000
        let limit = self.first.unwrap_or(100).min(1000) as i64;
        builder.push(" LIMIT ");
        builder.push_bind(limit);

        if let Some(skip) = self.skip {
            builder.push(" OFFSET ");
            builder.push_bind(skip as i64);
        }

        Ok(builder)
    }
}

/// Sanitize field identifiers to strictly allow only alphanumeric chars and underscores.
pub fn validate_identifier(ident: &str) -> Result<(), String> {
    if ident.is_empty() {
        return Err("Identifier cannot be empty".to_string());
    }
    if !ident.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!(
            "Invalid identifier contains unsafe characters: {ident}"
        ));
    }
    Ok(())
}
