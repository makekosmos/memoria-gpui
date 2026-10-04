//! JSON-LD/schema.org plumbing for `extract_book_metadata` — record
//! collection, `@type` filtering, and first-value lookup across scripts.
use serde_json::{Map, Value};
use std::sync::LazyLock;

use crate::book_metadata::clean_text;

static BOOK_TYPES: &[&str] = &["book", "audiobook", "publicationvolume", "product"];
static JSONLD_SEL: LazyLock<scraper::Selector> = LazyLock::new(|| {
    scraper::Selector::parse("script[type=\"application/ld+json\"]").expect("ld selector")
});

pub(super) fn string_from_json_value(value: &Value) -> String {
    match value {
        Value::Array(items) => items
            .iter()
            .map(string_from_json_value)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(record) => ["name", "@value", "url", "contentUrl"]
            .iter()
            .map(|key| clean_text(record.get(*key).unwrap_or(&Value::Null)))
            .find(|s| !s.is_empty())
            .unwrap_or_default(),
        _ => clean_text(value),
    }
}

fn collect_json_records(value: &Value, records: &mut Vec<Map<String, Value>>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_json_records(item, records);
            }
        }
        Value::Object(record) => {
            records.push(record.clone());
            for key in ["@graph", "mainEntity", "item"] {
                if let Some(inner) = record.get(key) {
                    collect_json_records(inner, records);
                }
            }
        }
        _ => {}
    }
}

fn json_type_matches(record: &Map<String, Value>) -> bool {
    let raw = record.get("@type").unwrap_or(&Value::Null);
    let values: Vec<&Value> = match raw {
        Value::Array(items) => items.iter().collect(),
        other => vec![other],
    };
    values
        .iter()
        .any(|v| BOOK_TYPES.contains(&clean_text(v).to_lowercase().as_str()))
}

pub(super) fn parse_json_ld(document: &scraper::Html) -> Vec<Map<String, Value>> {
    let mut records = Vec::new();
    for script in document.select(&JSONLD_SEL) {
        // A malformed block must not hide valid metadata from the rest.
        if let Ok(value) = serde_json::from_str::<Value>(&script.text().collect::<String>()) {
            collect_json_records(&value, &mut records);
        }
    }
    records.retain(json_type_matches);
    records
}

pub(super) fn first_json_value<'a>(
    records: &'a [Map<String, Value>],
    keys: &[&str],
) -> Option<&'a Value> {
    for record in records {
        for key in keys {
            if let Some(value) = record.get(*key) {
                if !string_from_json_value(value).is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}
