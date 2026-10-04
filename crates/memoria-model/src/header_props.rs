//! Port of `src/lib/typedNoteHeaderProps.ts` — header-prop defaults,
//! per-kind coercion, `normalizeHeaderProps`/`safeParseHeaderProps`.

use serde_json::{Map, Value};
use std::sync::LazyLock;

use crate::model::{NoteType, NoteTypeField};
use crate::note_type_schemas::parse_note_type_definition;

static NUMBER_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^$|^-?\d+(\.\d+)?$").expect("num re"));

/// `parseNoteTypeDefinition` throws in Vue — every public function here
/// propagates that instead of silently treating the type as field-less.
fn fields_of(note_type: Option<&NoteType>) -> Result<Vec<NoteTypeField>, String> {
    note_type
        .map(|nt| parse_note_type_definition(&nt.schema_json).map(|d| d.fields))
        .transpose()
        .map(|d| d.unwrap_or_default())
}

/// `createDefaultHeaderProps` — per-kind empty values.
pub fn create_default_header_props(
    note_type: Option<&NoteType>,
) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    let Some(note_type) = note_type else {
        return Ok(out);
    };
    for field in fields_of(Some(note_type))? {
        let value = match field.kind.as_str() {
            "boolean" => Value::Bool(false),
            "multi_select" => Value::Array(vec![]),
            "relation" if field.multiple == Some(false) => Value::from(""),
            "relation" => Value::Array(vec![]),
            _ => Value::from(""),
        };
        out.insert(field.id.clone(), value);
    }
    Ok(out)
}

fn is_person_like(note_type: Option<&NoteType>) -> bool {
    let Some(nt) = note_type else { return false };
    // Vue wraps the whole check in try/catch: an unparseable schema makes
    // even `slug === "person"` return false.
    let Ok(definition) = parse_note_type_definition(&nt.schema_json) else {
        return false;
    };
    if nt.slug == "person" {
        return true;
    }
    let ids: std::collections::HashSet<&str> =
        definition.fields.iter().map(|f| f.id.as_str()).collect();
    ids.contains("first_name") && ids.contains("last_name") && ids.contains("patronymic")
}

fn split_person_title(title: &str) -> (String, String, String) {
    let parts: Vec<&str> = title.split_whitespace().collect();
    (
        parts.first().copied().unwrap_or("").to_string(),
        parts.get(1).copied().unwrap_or("").to_string(),
        parts.get(2..).unwrap_or(&[]).join(" "),
    )
}

/// `createHeaderPropsForTypeChange` — person types inherit split title.
pub fn create_header_props_for_type_change(
    note_type: Option<&NoteType>,
    source_title: &str,
) -> Result<Map<String, Value>, String> {
    let mut props = create_default_header_props(note_type)?;
    if !is_person_like(note_type) {
        return Ok(props);
    }
    let (first, last, patronymic) = split_person_title(source_title);
    props.insert("first_name".into(), Value::from(first));
    props.insert("last_name".into(), Value::from(last));
    props.insert("patronymic".into(), Value::from(patronymic));
    Ok(props)
}

fn string_list(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
        Value::String(s) => s
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// `coerceHeaderFieldValue`.
fn coerce_field_value(field: &NoteTypeField, value: Option<&Value>) -> Value {
    let value = value.cloned().unwrap_or(Value::Null);
    match field.kind.as_str() {
        "text" | "url" | "long_text" | "date" | "image" => match &value {
            Value::String(_) => value,
            Value::Number(n) if n.as_f64().map(|f| f.is_finite()).unwrap_or(false) => {
                Value::from(n.to_string())
            }
            _ => Value::from(""),
        },
        "number" => match &value {
            Value::Number(n) if n.as_f64().map(|f| f.is_finite()).unwrap_or(false) => value,
            Value::String(s) => {
                let trimmed = s.trim();
                if NUMBER_RE.is_match(trimmed) {
                    Value::from(trimmed)
                } else {
                    Value::from("")
                }
            }
            _ => Value::from(""),
        },
        "boolean" => match &value {
            Value::Bool(_) => value,
            Value::Number(n) if n.as_f64() == Some(1.0) => Value::Bool(true),
            Value::Number(n) if n.as_f64() == Some(0.0) => Value::Bool(false),
            Value::String(s) if s == "1" || s == "true" => Value::Bool(true),
            Value::String(s) if s == "0" || s == "false" => Value::Bool(false),
            _ => Value::Bool(false),
        },
        "multi_select" => Value::Array(string_list(&value).into_iter().map(Value::from).collect()),
        "relation" if field.multiple == Some(false) => match &value {
            Value::String(s) => Value::from(s.trim()),
            Value::Array(items) => items
                .iter()
                .find_map(Value::as_str)
                .map(|s| Value::from(s.trim()))
                .unwrap_or(Value::from("")),
            _ => Value::from(""),
        },
        "relation" => match &value {
            Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    Value::Array(vec![])
                } else {
                    Value::Array(vec![Value::from(trimmed)])
                }
            }
            _ => Value::Array(string_list(&value).into_iter().map(Value::from).collect()),
        },
        "select" => match &value {
            Value::String(_) => value,
            Value::Number(n) if n.as_f64().map(|f| f.is_finite()).unwrap_or(false) => {
                Value::from(n.to_string())
            }
            Value::Array(items) => items
                .iter()
                .find_map(Value::as_str)
                .map(Value::from)
                .unwrap_or(Value::from("")),
            _ => Value::from(""),
        },
        _ => value,
    }
}

/// `normalizeHeaderProps` — defaults + coerced known fields + passthrough.
pub fn normalize_header_props(
    note_type: Option<&NoteType>,
    raw: &Value,
) -> Result<Map<String, Value>, String> {
    let valid = matches!(raw, Value::Object(_));
    let mut out = create_default_header_props(note_type)?;
    if !valid {
        return Ok(out);
    }
    let Some(note_type) = note_type else {
        return Ok(raw.as_object().cloned().unwrap_or_default());
    };
    let raw_map = raw.as_object().cloned().unwrap_or_default();
    for (key, value) in &raw_map {
        out.insert(key.clone(), value.clone());
    }
    for field in fields_of(Some(note_type))? {
        out.insert(
            field.id.clone(),
            coerce_field_value(&field, raw_map.get(&field.id)),
        );
    }
    Ok(out)
}

/// `validateHeaderProps` — zod shape check post-normalization.
pub fn validate_header_props(
    note_type: Option<&NoteType>,
    raw: &Value,
) -> Result<Map<String, Value>, String> {
    let normalized = normalize_header_props(note_type, raw)?;
    let Some(note_type) = note_type else {
        return Ok(normalized);
    };
    for field in fields_of(Some(note_type))? {
        let present = normalized.get(&field.id);
        if field.required && present.is_none() {
            return Err(format!("missing required field {}", field.id));
        }
        let Some(value) = present else { continue };
        let ok = match field.kind.as_str() {
            "number" => {
                matches!(value, Value::Number(_))
                    || matches!(value, Value::String(s) if NUMBER_RE.is_match(s))
            }
            "date" => value.is_string(),
            "boolean" => value.is_boolean(),
            "multi_select" => {
                matches!(value, Value::Array(items) if items.iter().all(|v| v.is_string()))
            }
            "relation" if field.multiple == Some(false) => value.is_string(),
            "relation" => {
                matches!(value, Value::Array(items) if items.iter().all(|v| v.is_string()))
            }
            _ => value.is_string(),
        };
        if !ok {
            return Err(format!("invalid value for field {}", field.id));
        }
    }
    Ok(normalized)
}
