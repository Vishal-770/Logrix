//! English pluralization helper for dynamic GraphQL entity queries.

/// Pluralize an entity name according to standard English rules,
/// returning a lowercase pluralized string suitable for GraphQL query fields.
/// E.g. "Account" -> "accounts", "Activity" -> "activities", "Match" -> "matches".
pub fn pluralize_entity_name(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    if lower.is_empty() {
        return lower;
    }

    // Irregular nouns
    match lower.as_str() {
        "person" => return "people".to_string(),
        "child" => return "children".to_string(),
        "man" => return "men".to_string(),
        "woman" => return "women".to_string(),
        "foot" => return "feet".to_string(),
        "tooth" => return "teeth".to_string(),
        "goose" => return "geese".to_string(),
        "mouse" => return "mice".to_string(),
        "ox" => return "oxen".to_string(),
        _ => {}
    }

    // Consonant + y -> ies (e.g. activity -> activities, city -> cities)
    // Vowel + y -> ys (e.g. day -> days, key -> keys)
    if lower.ends_with('y') && lower.len() > 1 {
        let prev_char = lower.chars().rev().nth(1).unwrap_or(' ');
        if !matches!(prev_char, 'a' | 'e' | 'i' | 'o' | 'u') {
            return format!("{}ies", &lower[..lower.len() - 1]);
        }
    }

    // Sibilant endings: s, x, z, ch, sh -> append es
    if lower.ends_with("ch")
        || lower.ends_with("sh")
        || lower.ends_with('x')
        || lower.ends_with('z')
        || lower.ends_with('s')
    {
        return format!("{lower}es");
    }

    // Default: append s
    format!("{lower}s")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pluralize_consonant_y() {
        assert_eq!(pluralize_entity_name("Activity"), "activities");
        assert_eq!(pluralize_entity_name("City"), "cities");
        assert_eq!(pluralize_entity_name("Proxy"), "proxies");
        assert_eq!(pluralize_entity_name("Company"), "companies");
    }

    #[test]
    fn test_pluralize_vowel_y() {
        assert_eq!(pluralize_entity_name("Day"), "days");
        assert_eq!(pluralize_entity_name("Key"), "keys");
        assert_eq!(pluralize_entity_name("Boy"), "boys");
    }

    #[test]
    fn test_pluralize_sibilants() {
        assert_eq!(pluralize_entity_name("Match"), "matches");
        assert_eq!(pluralize_entity_name("Batch"), "batches");
        assert_eq!(pluralize_entity_name("Dish"), "dishes");
        assert_eq!(pluralize_entity_name("Box"), "boxes");
        assert_eq!(pluralize_entity_name("Tax"), "taxes");
        assert_eq!(pluralize_entity_name("Address"), "addresses");
        assert_eq!(pluralize_entity_name("Status"), "statuses");
        assert_eq!(pluralize_entity_name("Quiz"), "quizes");
    }

    #[test]
    fn test_pluralize_standard_and_irregular() {
        assert_eq!(pluralize_entity_name("Account"), "accounts");
        assert_eq!(pluralize_entity_name("Transfer"), "transfers");
        assert_eq!(pluralize_entity_name("Token"), "tokens");
        assert_eq!(pluralize_entity_name("Person"), "people");
        assert_eq!(pluralize_entity_name("Child"), "children");
        assert_eq!(pluralize_entity_name(""), "");
    }
}
