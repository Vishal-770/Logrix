use logrix_core::error::{ErrorClass, ErrorSource, LogrixError, LogrixResult};

pub(crate) fn expand_environment_placeholders(value: &mut serde_yaml::Value) -> LogrixResult<()> {
    match value {
        serde_yaml::Value::String(text) => *text = expand_string(text)?,
        serde_yaml::Value::Sequence(items) => {
            for item in items {
                expand_environment_placeholders(item)?;
            }
        }
        serde_yaml::Value::Mapping(entries) => {
            for item in entries.values_mut() {
                expand_environment_placeholders(item)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn expand_string(text: &str) -> LogrixResult<String> {
    let mut expanded = String::with_capacity(text.len());
    let mut cursor = 0;
    while let Some(offset) = text[cursor..].find("${") {
        let open = cursor + offset;
        expanded.push_str(&text[cursor..open]);
        let name_start = open + 2;
        let Some(close_offset) = text[name_start..].find('}') else {
            return Err(config_error("Unclosed environment placeholder in manifest"));
        };
        let close = name_start + close_offset;
        let name = &text[name_start..close];
        if !valid_name(name) {
            return Err(config_error(format!(
                "Invalid environment placeholder `${{{name}}}` in manifest"
            )));
        }
        let replacement = std::env::var(name).map_err(|error| {
            let message = match error {
                std::env::VarError::NotPresent => {
                    format!("Missing environment variable `{name}` referenced by manifest")
                }
                std::env::VarError::NotUnicode(_) => {
                    format!(
                        "Environment variable `{name}` referenced by manifest is not valid UTF-8"
                    )
                }
            };
            config_error(message)
        })?;
        expanded.push_str(&replacement);
        cursor = close + 1;
    }
    expanded.push_str(&text[cursor..]);
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

fn config_error(message: impl Into<String>) -> LogrixError {
    LogrixError::new(ErrorClass::Permanent, ErrorSource::Config, message)
}
