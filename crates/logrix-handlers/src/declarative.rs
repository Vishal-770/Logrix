use crate::manifest::DeclarativeRule;
use crate::staging::EmittedEntity;
use alloy_primitives::U256;
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
            if !self.matches_event(&rule.event, log) {
                continue;
            }

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

    /// Check if rule event pattern matches the log.
    fn matches_event(&self, pattern: &str, log: &EventLog) -> bool {
        let p = pattern.trim();
        if p == "*" || p.is_empty() {
            return true;
        }
        if let Some(topic0) = log.topics.first() {
            let hex_t0 = format!("{:#x}", topic0);
            if p.eq_ignore_ascii_case(&hex_t0) || p.eq_ignore_ascii_case(&format!("{:x}", topic0)) {
                return true;
            }
        }
        p.eq_ignore_ascii_case("Transfer") || p.eq_ignore_ascii_case("Approval")
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
        } else if let Some(sub) = expr.strip_prefix("log.topics[") {
            self.eval_topic(sub, log)
        } else if let Some(sub) = expr.strip_prefix("log.data[") {
            self.eval_data_slice(sub, log)
        } else {
            json!(expr)
        }
    }

    fn eval_topic(&self, sub: &str, log: &EventLog) -> Value {
        let as_u256 = sub.ends_with("].as_u256");
        let idx_part = if as_u256 {
            sub.strip_suffix("].as_u256")
        } else {
            sub.strip_suffix(']')
        };
        if let Some(idx_s) = idx_part {
            if let Ok(idx) = idx_s.parse::<usize>() {
                if let Some(topic) = log.topics.get(idx) {
                    if as_u256 {
                        let num = U256::from_be_bytes(topic.0);
                        return json!(num.to_string());
                    }
                    return json!(format!("{:#x}", topic));
                }
            }
        }
        Value::Null
    }

    fn eval_data_slice(&self, sub: &str, log: &EventLog) -> Value {
        let as_u256 = sub.ends_with("].as_u256");
        let range_part = if as_u256 {
            sub.strip_suffix("].as_u256")
        } else {
            sub.strip_suffix(']')
        };
        if let Some(slice_expr) = range_part {
            if let Some((start_s, end_s)) = slice_expr.split_once("..") {
                if let (Ok(start), Ok(end)) = (
                    start_s.trim().parse::<usize>(),
                    end_s.trim().parse::<usize>(),
                ) {
                    if start < log.data.len() && end <= log.data.len() && start <= end {
                        let slice = &log.data[start..end];
                        if as_u256 {
                            let num = U256::try_from_be_slice(slice).unwrap_or(U256::ZERO);
                            return json!(num.to_string());
                        }
                        return json!(format!("0x{}", alloy_primitives::hex::encode(slice)));
                    }
                }
            }
        }
        Value::Null
    }
}
