//! Port of `obsidianVaultImportFrontmatter.ts` — a "loose" frontmatter parser
//! for hand-written Obsidian YAML (not a full YAML impl) + wikilink helpers.

use serde_json::{Map, Value};
use std::sync::LazyLock;

use crate::model::NoteType;

/// `LooseFrontmatter` — parsed JSON-ish object, insertion order preserved.
pub type LooseFrontmatter = Map<String, Value>;

const RESERVED_FRONTMATTER_KEYS: [&str; 4] = ["eden", "title", "type", "links"];

/// `/(?<!!)\[\[([^|\]]+)(?:\|([^\]]+))?\]\]/g` — the leading `(?<!!)` is
/// emulated by capturing an optional `!` and skipping it (`!` group = "embed").
static WIKILINK_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"(!?)\[\[([^|\]]+)(?:\|([^\]]+))?\]\]").expect("wikilink"));

/// `splitLooseFrontmatter`.
pub fn split_loose_frontmatter(markdown: &str) -> (LooseFrontmatter, String) {
    let normalized = markdown
        .strip_prefix('\u{FEFF}')
        .unwrap_or(markdown)
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    if lines.first().map(|l| l.trim()) != Some("---") {
        return (Map::new(), normalized);
    }
    let close_index = lines.iter().enumerate().skip(1).find_map(|(index, line)| {
        let trimmed = line.trim();
        (trimmed == "---" || trimmed == "...").then_some(index)
    });
    let Some(close_index) = close_index else {
        return (Map::new(), normalized);
    };
    (
        parse_loose_yaml(&lines[1..close_index]),
        lines[close_index + 1..].join("\n"),
    )
}

/// `resolveTypeId`.
pub fn resolve_type_id(
    frontmatter: &LooseFrontmatter,
    note_types: &[NoteType],
    type_id_mapping: &Map<String, Value>,
) -> Option<String> {
    let eden = match frontmatter.get("eden") {
        Some(value) if is_plain_object(value) => value.as_object(),
        _ => None,
    };
    let candidates = [
        eden.and_then(|e| e.get("type")).map(frontmatter_string),
        frontmatter.get("type").map(frontmatter_string),
    ];
    for candidate in candidates.into_iter().flatten() {
        if let Some(mapped) = type_id_mapping.get(&candidate).and_then(Value::as_str) {
            if note_types.iter().any(|note_type| note_type.id == mapped) {
                return Some(mapped.to_string());
            }
        }
        if let Some(matched) = note_types.iter().find(|note_type| {
            note_type.id == candidate || note_type.slug == candidate || note_type.name == candidate
        }) {
            return Some(matched.id.clone());
        }
    }
    None
}

/// `extractHeaderProps` — reserved keys and `__*` internals are dropped.
pub fn extract_header_props(frontmatter: &LooseFrontmatter) -> Map<String, Value> {
    frontmatter
        .iter()
        .filter(|(key, _)| {
            !RESERVED_FRONTMATTER_KEYS.contains(&key.as_str()) && !key.starts_with("__")
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// `extractFrontmatterWikilinks` — recursively walks strings/arrays/objects.
pub fn extract_frontmatter_wikilinks(value: &Value) -> Vec<String> {
    match value {
        Value::String(text) => extract_body_wikilinks(text),
        Value::Array(items) => items
            .iter()
            .flat_map(extract_frontmatter_wikilinks)
            .collect(),
        Value::Object(map) => map
            .values()
            .flat_map(extract_frontmatter_wikilinks)
            .collect(),
        _ => Vec::new(),
    }
}

/// `extractBodyWikilinks` — `[[target|alias]]` embeds (`![[…]]`) excluded.
pub fn extract_body_wikilinks(markdown: &str) -> Vec<String> {
    WIKILINK_RE
        .captures_iter(markdown)
        .filter(|m| m.get(1).map(|g| g.as_str()).unwrap_or_default().is_empty())
        .filter_map(|m| m.get(2).map(|g| g.as_str().trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect()
}

/// `normalizeObsidianTitleTarget`.
pub fn normalize_obsidian_title_target(target: &str) -> String {
    let mut value = target.trim().to_string();
    if value.to_lowercase().ends_with(".md") {
        value.truncate(value.len() - 3);
    }
    if let Some(index) = value.find(['#', '^']) {
        value.truncate(index);
    }
    value.trim().to_string()
}

/// `frontmatterString` — trimmed string or `""` for non-strings.
pub fn frontmatter_string(value: &Value) -> String {
    value
        .as_str()
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

/// `isPlainObject`.
pub fn is_plain_object(value: &Value) -> bool {
    value.is_object()
}

fn parse_loose_yaml(lines: &[&str]) -> LooseFrontmatter {
    let mut result = Map::new();
    let mut current_array_key: Option<String> = None;
    let mut current_object_key: Option<String> = None;
    let mut current_nested_array_key: Option<String> = None;

    for raw in lines {
        let indent = raw.len() - raw.trim_start_matches(' ').len();
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let (Some(object_key), Some(nested_key)) =
            (current_object_key.clone(), current_nested_array_key.clone())
        {
            if indent >= 4 && line.starts_with("- ") {
                if let Some(Value::Object(obj)) = result.get_mut(&object_key) {
                    let next = obj
                        .get(&nested_key)
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let mut next = next;
                    next.push(parse_loose_scalar(line[2..].trim()));
                    obj.insert(nested_key.clone(), Value::Array(next));
                }
                continue;
            }
        }

        if let Some(object_key) = current_object_key.clone() {
            if indent >= 2 && !line.starts_with("- ") {
                let Some(Value::Object(obj)) = result.get_mut(&object_key) else {
                    continue;
                };
                let Some(index) = line.find(':').filter(|i| *i > 0) else {
                    continue;
                };
                let key = line[..index].trim().to_string();
                let value = line[index + 1..].trim();
                if value.is_empty() {
                    obj.insert(key.clone(), Value::Array(Vec::new()));
                    current_nested_array_key = Some(key);
                } else {
                    obj.insert(key, parse_loose_scalar(value));
                    current_nested_array_key = None;
                }
                continue;
            }
        }

        if let Some(array_key) = current_array_key.clone() {
            if let Some(item) = line.strip_prefix("- ") {
                let mut next = result
                    .get(&array_key)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                next.push(parse_loose_scalar(item.trim()));
                result.insert(array_key, Value::Array(next));
                continue;
            }
        }

        current_array_key = None;
        current_object_key = None;
        current_nested_array_key = None;
        let Some(index) = line.find(':').filter(|i| *i > 0) else {
            continue;
        };
        let key = line[..index].trim().to_string();
        let value = line[index + 1..].trim();
        if value.is_empty() {
            result.insert(key.clone(), Value::Object(Map::new()));
            current_object_key = Some(key.clone());
            current_array_key = Some(key);
        } else {
            result.insert(key, parse_loose_scalar(value));
        }
    }
    result
}

fn parse_loose_scalar(value: &str) -> Value {
    let trimmed = value.trim();
    match trimmed {
        "null" | "~" => return Value::Null,
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        _ => {}
    }
    static NUM_RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"^-?\d+(\.\d+)?$").expect("num"));
    if NUM_RE.is_match(trimmed) {
        if let Ok(number) = trimmed.parse::<f64>() {
            if let Some(json) = serde_json::Number::from_f64(number) {
                return Value::Number(json);
            }
        }
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return Value::Array(
            split_inline_yaml_array(&trimmed[1..trimmed.len() - 1])
                .iter()
                .map(|item| parse_loose_scalar(item))
                .collect(),
        );
    }
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        return Value::String(unquote_loose_scalar(trimmed));
    }
    Value::String(trimmed.to_string())
}

fn split_inline_yaml_array(value: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escape_next = false;
    for char in value.chars() {
        if escape_next {
            current.push(char);
            escape_next = false;
            continue;
        }
        if char == '\\' && quote == Some('"') {
            current.push(char);
            escape_next = true;
            continue;
        }
        if (char == '"' || char == '\'') && quote.is_none_or(|q| q == char) {
            quote = if quote.is_some() { None } else { Some(char) };
            current.push(char);
            continue;
        }
        if char == ',' && quote.is_none() {
            let item = current.trim();
            if !item.is_empty() {
                items.push(item.to_string());
            }
            current.clear();
            continue;
        }
        current.push(char);
    }
    let tail = current.trim();
    if !tail.is_empty() {
        items.push(tail.to_string());
    }
    items
}

fn unquote_loose_scalar(value: &str) -> String {
    let inner = &value[1..value.len() - 1];
    if !value.starts_with('"') {
        return inner.to_string();
    }
    inner.replace("\\\"", "\"").replace("\\\\", "\\")
}
