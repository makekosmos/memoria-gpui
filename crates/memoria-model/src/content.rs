//! Content codec — 1:1 port of Vue `src/editor-content/content.ts`.
//!
//! Envelopes: `{type:"markdown",version:1,text}` and
//! `{type:"tiptap",version:1,doc}`. The ARK wire `contentJson` carries a bare
//! ProseMirror `doc`. Unknown node `type`/`attrs`/`marks` are preserved: the
//! tree is stored as `serde_json::Value` and never destructured destructively.

pub mod inline;
pub mod legacy;
pub mod parse;
mod parse_list;
pub mod render;

pub use legacy::legacy_prose_mirror_to_text;
pub use parse::markdown_to_tiptap_doc;
pub use render::tiptap_doc_to_markdown;

use serde_json::{Map, Value};

pub const MARKDOWN_CONTENT_VERSION: i64 = 1;
pub const TIPTAP_CONTENT_VERSION: i64 = 1;

/// `isMarkdownContent`.
pub fn is_markdown_content(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("markdown")
}

/// `isTiptapContent` — envelope `{type:"tiptap", doc:{type:"doc",...}}`.
pub fn is_tiptap_content(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("tiptap")
        && value.get("doc").map(is_tiptap_doc).unwrap_or(false)
}

pub(crate) fn is_tiptap_doc(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("doc")
}

/// `writeEntryMarkdown` — key order matches the Vue object literal.
pub fn write_entry_markdown(md: &str) -> Value {
    let mut map = Map::new();
    map.insert("type".into(), Value::from("markdown"));
    map.insert("version".into(), Value::from(MARKDOWN_CONTENT_VERSION));
    map.insert("text".into(), Value::from(md));
    Value::Object(map)
}

/// `writeEntryTiptapDoc`.
pub fn write_entry_tiptap_doc(doc: Value) -> Value {
    let mut map = Map::new();
    map.insert("type".into(), Value::from("tiptap"));
    map.insert("version".into(), Value::from(TIPTAP_CONTENT_VERSION));
    map.insert("doc".into(), doc);
    Value::Object(map)
}

/// `readEntryMarkdown` — returns `""` for anything unreadable (never panics).
pub fn read_entry_markdown(value: &Value) -> String {
    if let Value::String(raw) = value {
        return match serde_json::from_str::<Value>(raw) {
            Ok(parsed) => read_entry_markdown(&parsed),
            Err(_) => String::new(),
        };
    }
    if is_markdown_content(value) {
        return value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
    }
    if is_tiptap_content(value) {
        return tiptap_doc_to_markdown(value.get("doc").unwrap_or(&Value::Null));
    }
    if is_tiptap_doc(value) {
        return tiptap_doc_to_markdown(value);
    }
    legacy_prose_mirror_to_text(value)
}

/// `readEntryTiptapDoc` — always yields a normalized `{type:"doc"}` doc.
pub fn read_entry_tiptap_doc(value: &Value) -> Value {
    if let Value::String(raw) = value {
        return match serde_json::from_str::<Value>(raw) {
            Ok(parsed) => read_entry_tiptap_doc(&parsed),
            Err(_) => markdown_to_tiptap_doc(""),
        };
    }
    if is_tiptap_content(value) {
        return normalize_tiptap_doc(value.get("doc").unwrap_or(&Value::Null));
    }
    if is_markdown_content(value) {
        let text = value
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return markdown_to_tiptap_doc(text);
    }
    if is_tiptap_doc(value) {
        return normalize_tiptap_doc(value);
    }
    markdown_to_tiptap_doc(&legacy_prose_mirror_to_text(value))
}

/// `isEntryTiptapContent` — string-safe envelope check.
pub fn is_entry_tiptap_content(value: &Value) -> bool {
    if let Value::String(raw) = value {
        return match serde_json::from_str::<Value>(raw) {
            Ok(parsed) => is_entry_tiptap_content(&parsed),
            Err(_) => false,
        };
    }
    is_tiptap_content(value)
}

/// `isReadableEntryContent`.
pub fn is_readable_entry_content(value: &Value) -> bool {
    if let Value::String(raw) = value {
        return match serde_json::from_str::<Value>(raw) {
            Ok(parsed) => is_readable_entry_content(&parsed),
            Err(_) => false,
        };
    }
    is_markdown_content(value)
        || is_tiptap_content(value)
        || is_tiptap_doc(value)
        || !legacy_prose_mirror_to_text(value).is_empty()
}

/// `normalizeTiptapDoc` — drops unknown doc-level keys, exactly like Vue.
fn normalize_tiptap_doc(doc: &Value) -> Value {
    let content = match doc.get("content") {
        Some(Value::Array(items)) => Value::Array(items.clone()),
        _ => Value::Array(vec![parse::node("paragraph", None, None)]),
    };
    let mut map = Map::new();
    map.insert("type".into(), Value::from("doc"));
    map.insert("content".into(), content);
    Value::Object(map)
}

// --- shared helpers --------------------------------------------------------

/// `normalizeMarks` — keep object marks with a string `type`; `attrs` must be
/// an object to survive.
pub(crate) fn normalize_marks(marks: Option<&Value>) -> Vec<Value> {
    let Some(Value::Array(items)) = marks else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|m| m.is_object())
        .filter_map(|m| {
            let mark_type = m.get("type").and_then(Value::as_str)?;
            let mut out = Map::new();
            out.insert("type".into(), Value::from(mark_type));
            if let Some(attrs) = m.get("attrs").filter(|a| a.is_object()) {
                out.insert("attrs".into(), attrs.clone());
            }
            Some(Value::Object(out))
        })
        .collect()
}

/// `mergeMarks` — replace any existing mark of the same type.
pub(crate) fn merge_marks(existing: Vec<Value>, next: &Value) -> Vec<Value> {
    let next_type = next.get("type").and_then(Value::as_str);
    let mut out: Vec<Value> = existing
        .into_iter()
        .filter(|m| m.get("type").and_then(Value::as_str) != next_type)
        .collect();
    out.push(next.clone());
    out
}

/// `renderMarkedText`.
pub(crate) fn render_marked_text(text: &str, marks: &[Value]) -> String {
    let find = |t: &str| {
        marks
            .iter()
            .find(|m| m.get("type").and_then(Value::as_str) == Some(t))
    };
    let has = |ts: &[&str]| ts.iter().any(|t| find(t).is_some());
    let code = find("code").is_some();
    let mut rendered = if code {
        wrap_inline_code(text)
    } else {
        escape_markdown_text(text)
    };
    if !code && has(&["bold", "strong"]) {
        rendered = format!("**{rendered}**");
    }
    if !code && has(&["italic", "em"]) {
        rendered = format!("*{rendered}*");
    }
    if !code && find("strike").is_some() {
        rendered = format!("~~{rendered}~~");
    }
    if let Some(href) = find("link")
        .and_then(|m| m.get("attrs"))
        .and_then(|a| a.get("href"))
        .and_then(Value::as_str)
    {
        rendered = format!("[{rendered}]({href})");
    }
    rendered
}

/// `wrapInlineCode` — the delimiter grows past the longest inner backtick
/// run (same rule as `render_code_fence`); a fixed `` `` `` fence would be
/// re-parsed as nested spans when the text contains `` `` ``.
fn wrap_inline_code(text: &str) -> String {
    let max_run = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    if max_run == 0 {
        return format!("`{text}`");
    }
    let fence = "`".repeat((max_run + 1).max(2));
    format!("{fence} {text} {fence}")
}

/// `escapeMarkdownText` — escapes `\ [ ] * _ ~`.
fn escape_markdown_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(ch, '\\' | '[' | ']' | '*' | '_' | '~') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// `renderCodeFence(text, attrs)` — fence grows past the longest backtick run.
pub(crate) fn render_code_fence(text: &str, attrs: Option<&Value>) -> String {
    let language = attrs
        .and_then(|a| a.get("language"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let max_run = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat((max_run + 1).max(3));
    format!("{fence}{language}\n{text}\n{fence}")
}

/// `prefixMultiline` — first line gets the marker, rest get two spaces.
pub(crate) fn prefix_multiline(text: &str, prefix: &str) -> String {
    text.split('\n')
        .enumerate()
        .map(|(i, line)| {
            if i == 0 {
                format!("{prefix}{line}")
            } else {
                format!("  {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// JS truthiness for JSON values (used by the ported renderers).
pub(crate) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

pub(crate) fn attrs_of(node: &Value) -> Option<&Map<String, Value>> {
    node.get("attrs").and_then(Value::as_object)
}

pub(crate) fn content_of(node: &Value) -> &[Value] {
    match node.get("content") {
        Some(Value::Array(items)) => items.as_slice(),
        _ => &[],
    }
}
