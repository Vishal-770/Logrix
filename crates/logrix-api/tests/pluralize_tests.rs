use logrix_api::pluralize_entity_name;

#[test]
fn test_pluralize_consonant_y_endings() {
    assert_eq!(pluralize_entity_name("Activity"), "activities");
    assert_eq!(pluralize_entity_name("activity"), "activities");
    assert_eq!(pluralize_entity_name("City"), "cities");
    assert_eq!(pluralize_entity_name("Category"), "categories");
    assert_eq!(pluralize_entity_name("Factory"), "factories");
    assert_eq!(pluralize_entity_name("Supply"), "supplies");
}

#[test]
fn test_pluralize_vowel_y_endings() {
    assert_eq!(pluralize_entity_name("Day"), "days");
    assert_eq!(pluralize_entity_name("Key"), "keys");
    assert_eq!(pluralize_entity_name("Boy"), "boys");
    assert_eq!(pluralize_entity_name("Toy"), "toys");
    assert_eq!(pluralize_entity_name("Survey"), "surveys");
}

#[test]
fn test_pluralize_sibilant_endings() {
    assert_eq!(pluralize_entity_name("Match"), "matches");
    assert_eq!(pluralize_entity_name("Batch"), "batches");
    assert_eq!(pluralize_entity_name("Dish"), "dishes");
    assert_eq!(pluralize_entity_name("Flash"), "flashes");
    assert_eq!(pluralize_entity_name("Box"), "boxes");
    assert_eq!(pluralize_entity_name("Tax"), "taxes");
    assert_eq!(pluralize_entity_name("Index"), "indexes");
    assert_eq!(pluralize_entity_name("Address"), "addresses");
    assert_eq!(pluralize_entity_name("Status"), "statuses");
    assert_eq!(pluralize_entity_name("Quiz"), "quizes");
}

#[test]
fn test_pluralize_standard_and_irregulars() {
    assert_eq!(pluralize_entity_name("Account"), "accounts");
    assert_eq!(pluralize_entity_name("Transfer"), "transfers");
    assert_eq!(pluralize_entity_name("Block"), "blocks");
    assert_eq!(pluralize_entity_name("Token"), "tokens");
    assert_eq!(pluralize_entity_name("Person"), "people");
    assert_eq!(pluralize_entity_name("Child"), "children");
    assert_eq!(pluralize_entity_name("Man"), "men");
    assert_eq!(pluralize_entity_name("Woman"), "women");
}

#[test]
fn test_pluralize_whitespace_and_empty() {
    assert_eq!(pluralize_entity_name(""), "");
    assert_eq!(pluralize_entity_name("  "), "");
    assert_eq!(pluralize_entity_name("  Account  "), "accounts");
}
