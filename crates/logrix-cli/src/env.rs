use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};

/// Expand `${VAR}` references at runtime without serializing their values into deployment files.
pub fn expand_env_placeholders(value: &str, field_name: &str) -> LogrixResult<String> {
    let mut expanded = String::with_capacity(value.len());
    let mut cursor = 0;

    while let Some(offset) = value[cursor..].find("${") {
        let open = cursor + offset;
        expanded.push_str(&value[cursor..open]);
        let name_start = open + 2;
        let close = value[name_start..]
            .find('}')
            .map(|n| name_start + n)
            .ok_or_else(|| {
                config_error(format!("Unclosed environment placeholder in {field_name}"))
            })?;
        let name = &value[name_start..close];
        if !valid_name(name) {
            return Err(config_error(format!(
                "Invalid environment placeholder `${{{name}}}` in {field_name}"
            )));
        }
        let replacement = std::env::var(name).map_err(|error| {
            let message = match error {
                std::env::VarError::NotPresent => {
                    format!("Environment variable `{name}` is required by {field_name}")
                }
                std::env::VarError::NotUnicode(_) => {
                    format!("Environment variable `{name}` is not valid UTF-8")
                }
            };
            config_error(message)
        })?;
        expanded.push_str(&replacement);
        cursor = close + 1;
    }

    expanded.push_str(&value[cursor..]);
    Ok(expanded)
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn config_error(message: String) -> LogrixError {
    LogrixError::new(ErrorClass::Permanent, ErrorSource::Config, message)
}
