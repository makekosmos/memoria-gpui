//! Port of `src/lib/entryTitles.ts` — untitled-title marker handling.
//!
//! A generated "untitled" title is marked via `__untitledTitle` inside
//! `header_props_json`; display substitutes «Без названия» while the editor
//! sees an empty string.

use serde_json::{Map, Value};

pub const UNTITLED_ENTRY_PLACEHOLDER: &str = "Без названия";
pub const UNTITLED_ENTRY_TITLE_FLAG: &str = "__untitledTitle";

fn has_flag(source: &Value) -> bool {
    match source {
        // Vue parses a string source exactly once and requires the result to
        // be a plain object — a JSON-encoded string-of-JSON does not count.
        Value::String(raw) => match serde_json::from_str::<Value>(raw) {
            Ok(Value::Object(map)) => {
                map.get(UNTITLED_ENTRY_TITLE_FLAG) == Some(&Value::Bool(true))
            }
            _ => false,
        },
        Value::Object(map) => map.get(UNTITLED_ENTRY_TITLE_FLAG) == Some(&Value::Bool(true)),
        _ => false,
    }
}

/// `createUntitledEntryHeaderProps`.
pub fn create_untitled_entry_header_props() -> Map<String, Value> {
    let mut map = Map::new();
    map.insert(UNTITLED_ENTRY_TITLE_FLAG.into(), Value::Bool(true));
    map
}

fn is_generated_untitle(_title: Option<&str>, source: Option<&Value>) -> bool {
    match source {
        Some(src) => has_flag(src),
        None => false,
    }
}

/// `getEntryDisplayTitle`.
pub fn get_entry_display_title(title: Option<&str>, source: Option<&Value>) -> String {
    let normalized = title.map(str::trim).unwrap_or("");
    if normalized.is_empty() || is_generated_untitle(Some(normalized), source) {
        return UNTITLED_ENTRY_PLACEHOLDER.to_string();
    }
    normalized.to_string()
}
