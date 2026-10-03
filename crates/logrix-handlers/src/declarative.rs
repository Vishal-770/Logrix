use crate::manifest::DeclarativeRule;
use crate::staging::EmittedEntity;
use logrix_core::domain::EventLog;
use serde_json::{json, Map, Value};
use tracing::debug;

/// Fast-path declarative mapper extracting event fields into structured entities.
#[derive(Debug, Clone, Default)]
pub struct DeclarativeMapper;

impl DeclarativeMapper {
    pub fn new() -> Self {
        Self
    }

    /// Evaluate declarative rules against an event log, producing emitted entities.
    pub fn apply_rules(&self, rules: &[DeclarativeRule], log: &EventLog) -> Vec<EmittedEntity> {
        let mut results = Vec::new();

        for rule in rules {
            // If rule.event matches or is wildcard
            let mut record = Map::new();

            for (field_name, expr) in &rule.fields {
                let value = self.evaluate_expression(expr, log);
                record.insert(field_name.clone(), value);
            }

            debug!(
                entity = %rule.entity,
                fields = record.len(),
                "Declarative rule produced entity"
            );

            results.push(EmittedEntity {
                entity_type: rule.entity.clone(),
                payload: Value::Object(record),
            });
        }

        results
    }

    /// Resolve field path expressions like `log.address`, `log.topics[0]`, `log.data`.
    fn evaluate_expression(&self, expr: &str, log: &EventLog) -> Value {
        let expr = expr.trim();

        if expr == "log.address" {
            json!(format!("{:#x}", log.address))
        } else if expr == "log.block_number" {
            json!(log.block_number)
        } else if expr == "log.tx_hash" {
            json!(format!("{:#x}", log.tx_hash))
        } else if expr == "log.log_index" {
            json!(log.log_index)
        } else if expr == "log.data" {
            json!(format!("0x{}", alloy_primitives::hex::encode(&log.data)))
        } else if let Some(idx_str) = expr
            .strip_prefix("log.topics[")
            .and_then(|s| s.strip_suffix(']'))
        {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(topic) = log.topics.get(idx) {
                    return json!(format!("{:#x}", topic));
                }
            }
            Value::Null
        } else if let Some(slice_expr) = expr
            .strip_prefix("log.data[")
            .and_then(|s| s.strip_suffix(']'))
        {
            // E.g. log.data[0..32]
            if let Some((start_s, end_s)) = slice_expr.split_once("..") {
                if let (Ok(start), Ok(end)) = (
                    start_s.trim().parse::<usize>(),
                    end_s.trim().parse::<usize>(),
                ) {
                    if start < log.data.len() && end <= log.data.len() && start <= end {
                        let slice = &log.data[start..end];
                        return json!(format!("0x{}", alloy_primitives::hex::encode(slice)));
                    }
                }
            }
            Value::Null
        } else {
            // Literal value or fallback
            json!(expr)
        }
    }
}
