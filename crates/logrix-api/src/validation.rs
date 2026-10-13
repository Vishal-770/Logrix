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

/// Validate numeric string representation for safe, exact-precision SQL evaluation.
pub fn validate_numeric_str(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("Numeric filter value cannot be empty".to_string());
    }
    let trimmed = s.trim();
    let num_str = trimmed.strip_prefix('-').unwrap_or(trimmed);
    if num_str.is_empty() {
        return Err("Invalid numeric format".to_string());
    }
    let mut parts = num_str.splitn(2, '.');
    let int_part = parts.next().unwrap();
    if !int_part.chars().all(|c| c.is_ascii_digit()) || int_part.is_empty() {
        return Err(format!("Invalid integer part in numeric filter: {s}"));
    }
    if let Some(frac_part) = parts.next() {
        if !frac_part.chars().all(|c| c.is_ascii_digit()) || frac_part.is_empty() {
            return Err(format!("Invalid fractional part in numeric filter: {s}"));
        }
    }
    Ok(())
}
