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

fn strip_flag(mut header_props: Map<String, Value>) -> Map<String, Value> {
    header_props.shift_remove(UNTITLED_ENTRY_TITLE_FLAG);
    header_props
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

/// `getEditableEntryTitle`.
pub fn get_editable_entry_title(title: Option<&str>, source: Option<&Value>) -> String {
    if is_generated_untitle(title, source) {
        return String::new();
    }
    title.unwrap_or_default().to_string()
}

/// `resolveStoredEntryTitle`.
pub fn resolve_stored_entry_title(
    edited_title: &str,
    persisted_title: &str,
    persisted_source: Option<&Value>,
) -> String {
    let normalized = edited_title.trim();
    if !normalized.is_empty() {
        return normalized.to_string();
    }
    if is_generated_untitle(Some(persisted_title), persisted_source) {
        return persisted_title.to_string();
    }
    normalized.to_string()
}

/// `syncUntitledEntryTitleFlag`.
pub fn sync_untitled_entry_title_flag(
    header_props: Map<String, Value>,
    edited_title: &str,
    persisted_title: &str,
    persisted_source: Option<&Value>,
) -> Map<String, Value> {
    if !edited_title.trim().is_empty() {
        return strip_flag(header_props);
    }
    if is_generated_untitle(Some(persisted_title), persisted_source) {
        let mut out = strip_flag(header_props);
        out.insert(UNTITLED_ENTRY_TITLE_FLAG.into(), Value::Bool(true));
        return out;
    }
    strip_flag(header_props)
}
