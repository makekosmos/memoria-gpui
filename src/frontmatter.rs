//! Port of `src/lib/markdownFrontmatter.ts` — `buildEntryMarkdownDocument`:
//! `eden`/`title`/`type`/header-props/`links.related` frontmatter serialized
//! as the Vue hand-rolled YAML subset.

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::model::{Entry, NoteType};

const FRONTMATTER_BOUNDARY: &str = "---";

/// `BuildEntryMarkdownDocumentArgs.relatedEntryTitleLookup` — a title map;
/// `None` ⇒ `[[id]]` links without aliases.
pub type RelatedTitleLookup<'a> = Option<&'a HashMap<String, String>>;

/// `buildEntryMarkdownDocument`.
pub fn build_entry_markdown_document(
    entry: &Entry,
    note_type: Option<&NoteType>,
    body_markdown: &str,
    related_lookup: RelatedTitleLookup<'_>,
) -> Result<String, String> {
    let body = normalize_body_markdown(body_markdown);
    let header_props = parse_entry_header_props(entry.header_props_json.as_deref())?;
    let related_notes = normalize_related_notes(header_props.get("related_notes"));
    let frontmatter = build_frontmatter(
        entry,
        note_type,
        &header_props,
        &related_notes,
        related_lookup,
    )?;
    Ok(format!(
        "{FRONTMATTER_BOUNDARY}\n{}\n{FRONTMATTER_BOUNDARY}\n{body}",
        serialize_yaml_document(&frontmatter)
    ))
}

fn build_frontmatter(
    entry: &Entry,
    note_type: Option<&NoteType>,
    header_props: &Map<String, Value>,
    related_notes: &[String],
    related_lookup: RelatedTitleLookup<'_>,
) -> Result<Map<String, Value>, String> {
    let mut frontmatter = Map::new();
    let mut eden = Map::new();
    eden.insert("id".into(), Value::from(entry.id.clone()));
    if let Some(t) = entry
        .type_id
        .clone()
        .or_else(|| note_type.map(|nt| nt.id.clone()))
    {
        eden.insert("type".into(), Value::from(t));
    }
    if let Some(v) = entry.schema_version {
        eden.insert("schema_version".into(), Value::from(v));
    }
    frontmatter.insert("eden".into(), Value::Object(eden));
    frontmatter.insert("title".into(), Value::from(entry.title.clone()));
    frontmatter.insert(
        "type".into(),
        Value::from(
            note_type
                .map(|nt| nt.slug.clone())
                .filter(|s| !s.is_empty())
                .or_else(|| entry.type_id.clone())
                .unwrap_or_else(|| "note_obj".into()),
        ),
    );

    for (key, value) in header_props {
        if key == "related_notes" || is_internal_header_prop_key(key) {
            continue;
        }
        if let Some(normalized) =
            normalize_frontmatter_value(value, &format!("header_props_json.{key}"))?
        {
            frontmatter.insert(key.clone(), normalized);
        }
    }

    if !related_notes.is_empty() {
        let mut links = Map::new();
        links.insert(
            "related".into(),
            Value::Array(
                related_notes
                    .iter()
                    .map(|id| Value::from(build_related_wikilink(id, related_lookup)))
                    .collect(),
            ),
        );
        frontmatter.insert("links".into(), Value::Object(links));
    }
    Ok(frontmatter)
}

fn parse_entry_header_props(raw: Option<&str>) -> Result<Map<String, Value>, String> {
    let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
        return Ok(Map::new());
    };
    let parsed: Value =
        serde_json::from_str(raw).map_err(|e| format!("Invalid entry.header_props_json: {e}"))?;
    match parsed {
        Value::Object(map) => Ok(map),
        _ => Err("Invalid entry.header_props_json: expected a JSON object".into()),
    }
}

fn normalize_body_markdown(body: &str) -> String {
    let normalized = body.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .strip_prefix('\n')
        .map(str::to_string)
        .unwrap_or(normalized)
}

fn normalize_related_notes(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

fn build_related_wikilink(related_id: &str, lookup: RelatedTitleLookup<'_>) -> String {
    let title = lookup
        .and_then(|map| map.get(related_id))
        .map(|t| t.trim())
        .filter(|t| !t.is_empty());
    match title {
        Some(title) => format!("[[{related_id}|{title}]]"),
        None => format!("[[{related_id}]]"),
    }
}

/// `normalizeFrontmatterValue` — JSON `null`/scalars/arrays/objects pass;
/// undefined is skipped (impossible in JSON); other types can't occur.
fn normalize_frontmatter_value(value: &Value, path: &str) -> Result<Option<Value>, String> {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            Ok(Some(value.clone()))
        }
        Value::Array(items) => Ok(Some(Value::Array(
            items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    normalize_frontmatter_value(item, &format!("{path}[{i}]"))?
                        .ok_or_else(|| format!("Unsupported undefined value at {path}[{i}]"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        ))),
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, nested) in map {
                if let Some(v) = normalize_frontmatter_value(nested, &format!("{path}.{key}"))? {
                    out.insert(key.clone(), v);
                }
            }
            Ok(Some(Value::Object(out)))
        }
    }
}

fn serialize_yaml_document(value: &Map<String, Value>) -> String {
    serialize_object_lines(value, 0).join("\n")
}

fn serialize_object_lines(map: &Map<String, Value>, indent_level: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let indent = " ".repeat(indent_level);
    for (key, value) in map {
        match value {
            Value::Array(items) => {
                if items.is_empty() {
                    lines.push(format!("{indent}{key}: []"));
                } else {
                    lines.push(format!("{indent}{key}:"));
                    lines.extend(serialize_array_lines(items, indent_level + 2));
                }
            }
            Value::Object(nested) => {
                let nested_lines = serialize_object_lines(nested, indent_level + 2);
                if nested_lines.is_empty() {
                    lines.push(format!("{indent}{key}: {{}}"));
                } else {
                    lines.push(format!("{indent}{key}:"));
                    lines.extend(nested_lines);
                }
            }
            scalar => lines.push(format!("{indent}{key}: {}", serialize_scalar(scalar))),
        }
    }
    lines
}

fn serialize_array_lines(items: &[Value], indent_level: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let indent = " ".repeat(indent_level);
    for item in items {
        match item {
            Value::Array(inner) => {
                if inner.is_empty() {
                    lines.push(format!("{indent}- []"));
                } else {
                    lines.push(format!("{indent}-"));
                    lines.extend(serialize_array_lines(inner, indent_level + 2));
                }
            }
            Value::Object(map) => {
                let nested = serialize_object_lines(map, indent_level + 2);
                if nested.is_empty() {
                    lines.push(format!("{indent}- {{}}"));
                } else {
                    lines.push(format!("{indent}-"));
                    lines.extend(nested);
                }
            }
            scalar => lines.push(format!("{indent}- {}", serialize_scalar(scalar))),
        }
    }
    lines
}

/// `serializeScalar` — `null`/`String(number|boolean)`/`""`→`""`; strings that
/// need quoting go through `JSON.stringify`.
fn serialize_scalar(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            if s.is_empty() {
                "\"\"".into()
            } else if needs_quoted_string(s) {
                serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
            } else {
                s.clone()
            }
        }
        _ => "null".into(),
    }
}

fn needs_quoted_string(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed != value {
        return true;
    }
    let lowered = value.to_lowercase();
    if lowered == "null" || lowered == "true" || lowered == "false" {
        return true;
    }
    static NUM_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^-?\d+(\.\d+)?$").expect("num"));
    if NUM_RE.is_match(value) {
        return true;
    }
    static PLAIN_RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"^[\p{L}\p{N}_./-]+$").expect("plain"));
    !PLAIN_RE.is_match(value)
}

fn is_internal_header_prop_key(key: &str) -> bool {
    key == "internal" || key.starts_with("__")
}
