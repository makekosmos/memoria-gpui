//! `bubbleDiaryModel.ts` text helpers — draft parsing (`#tag` extraction),
//! tiptap plain-text extraction, tag stripping, draft-bubble construction.
//! `serde_json::Value` trees pass through untouched except the fields the Vue
//! functions rewrite (`text`, `content`), so unknown node keys survive.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use crate::local_time::{civil_at, civil_date_key};

use super::{BubbleKind, BubbleTimelineNode};

/// `TAG_PATTERN` — `/#[\p{L}\p{N}_-]+/gu`.
static TAG_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"#[\p{L}\p{N}_-]+"#).expect("tag re"));

/// `BLOCK_NODE_TYPES` — nodes that contribute a newline to plain text.
const BLOCK_NODE_TYPES: [&str; 5] = [
    "paragraph",
    "heading",
    "blockquote",
    "codeBlock",
    "listItem",
];

/// `parseBubbleDraft` — strip `#tag`s out of the text, collect them lowercased
/// and deduplicated (first occurrence wins).
pub fn parse_bubble_draft(input: &str) -> (String, Vec<String>) {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut tags: Vec<String> = Vec::new();
    let text = TAG_PATTERN
        .replace_all(&normalized, |caps: &regex::Captures| {
            let tag = caps[0][1..].trim().to_lowercase();
            if !tag.is_empty() {
                tags.push(tag);
            }
            " ".to_string()
        })
        .into_owned();
    let text = normalize_draft_text(&text);
    let mut seen = HashSet::new();
    tags.retain(|t| seen.insert(t.clone()));
    (text, tags)
}

/// `normalizeDraftText` — collapse inline whitespace, drop blank lines, trim.
pub fn normalize_draft_text(input: &str) -> String {
    input
        .split('\n')
        .map(|line| collapse_inline_ws(line).trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// `/[^\S\n]+/g → " "` — any run of non-newline whitespace becomes one space.
fn collapse_inline_ws(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut pending = false;
    for ch in line.chars() {
        if ch != '\n' && ch.is_whitespace() {
            pending = true;
        } else {
            if pending {
                out.push(' ');
            }
            pending = false;
            out.push(ch);
        }
    }
    if pending {
        out.push(' ');
    }
    out
}

/// `plainTextToTiptapDoc` — one paragraph, `content: []` when empty.
pub fn plain_text_to_tiptap_doc(text: &str) -> Value {
    let mut paragraph = Map::new();
    paragraph.insert("type".into(), Value::from("paragraph"));
    let inner = if text.is_empty() {
        Vec::new()
    } else {
        vec![text_node(text)]
    };
    paragraph.insert("content".into(), Value::Array(inner));
    let mut doc = Map::new();
    doc.insert("type".into(), Value::from("doc"));
    doc.insert(
        "content".into(),
        Value::Array(vec![Value::Object(paragraph)]),
    );
    Value::Object(doc)
}

fn text_node(text: &str) -> Value {
    let mut map = Map::new();
    map.insert("type".into(), Value::from("text"));
    map.insert("text".into(), Value::from(text));
    Value::Object(map)
}

/// `docFromBlock` — wrap a single top-level block as a doc.
pub(crate) fn doc_from_block(block: &Value) -> Value {
    let mut doc = Map::new();
    doc.insert("type".into(), Value::from("doc"));
    doc.insert("content".into(), Value::Array(vec![block.clone()]));
    Value::Object(doc)
}

/// `tiptapPlainText` / `bubblePlainText` — recursive text collection with a
/// newline after each block-type node; `>2` newlines collapse, edges trimmed.
pub fn bubble_plain_text(node: &Value) -> String {
    let mut chunks: Vec<String> = Vec::new();
    collect_tiptap_text(node, &mut chunks);
    let joined = chunks.join("");
    // `\n{3,}` → `\n\n` then trim.
    let mut out = String::with_capacity(joined.len());
    let mut run = 0;
    for ch in joined.chars() {
        if ch == '\n' {
            run += 1;
            if run <= 2 {
                out.push(ch);
            }
        } else {
            run = 0;
            out.push(ch);
        }
    }
    out.trim().to_string()
}

fn collect_tiptap_text(node: &Value, chunks: &mut Vec<String>) {
    if node.get("type").and_then(Value::as_str) == Some("text") {
        chunks.push(
            node.get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        );
        return;
    }
    if let Some(Value::Array(children)) = node.get("content") {
        for child in children {
            collect_tiptap_text(child, chunks);
        }
    }
    if let Some(t) = node.get("type").and_then(Value::as_str) {
        if BLOCK_NODE_TYPES.contains(&t) {
            chunks.push("\n".into());
        }
    }
}

/// `stripTagsFromTiptapDoc` — `#tag`s removed from text nodes (kept verbatim
/// inside `codeBlock`); emptied block nodes drop; `content` disappears when
/// empty. Anything unrecognized passes through as-is.
pub fn strip_tags_from_tiptap_doc(doc: &Value) -> Value {
    strip_tags_from_tiptap_node(doc, false).unwrap_or_else(|| plain_text_to_tiptap_doc(""))
}

fn strip_tags_from_tiptap_node(node: &Value, inside_code_block: bool) -> Option<Value> {
    let inside_code_block =
        inside_code_block || node.get("type").and_then(Value::as_str) == Some("codeBlock");
    if node.get("type").and_then(Value::as_str) == Some("text") {
        let raw = node.get("text").and_then(Value::as_str).unwrap_or("");
        let text = if inside_code_block {
            raw.trim_end().to_string()
        } else {
            normalize_draft_text(&TAG_PATTERN.replace_all(raw, " "))
        };
        if text.trim().is_empty() {
            return None;
        }
        let mut out = node.clone();
        out.as_object_mut()?
            .insert("text".into(), Value::from(text));
        return Some(out);
    }

    let children: Vec<Value> = match node.get("content") {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|child| strip_tags_from_tiptap_node(child, inside_code_block))
            .collect(),
        _ => Vec::new(),
    };
    let node_type = node.get("type").and_then(Value::as_str).unwrap_or("");
    if BLOCK_NODE_TYPES.contains(&node_type) && children.is_empty() {
        return None;
    }
    let mut out = node.clone();
    let map = out.as_object_mut()?;
    if children.is_empty() {
        map.shift_remove("content");
    } else {
        map.insert("content".into(), Value::Array(children));
    }
    Some(out)
}

/// `createDraftBubble` — `contentJson` is either the string's plain doc or a
/// tiptap tree the caller supplies; `plainText` defaults like the Vue
/// signature (`input` itself for strings, `tiptapPlainText` for docs).
pub fn create_draft_bubble(
    content_json: &Value,
    plain_text: Option<&str>,
    now_ms: i64,
    offset_min: i64,
) -> Option<BubbleTimelineNode> {
    let plain = match plain_text {
        Some(text) => text.to_string(),
        None => bubble_plain_text(content_json),
    };
    let (text, tags) = parse_bubble_draft(&plain);
    if text.is_empty() {
        return None;
    }
    let now = civil_at(now_ms, offset_min);
    Some(BubbleTimelineNode {
        id: format!("draft-{now_ms}"),
        created_at: Some(crate::time::millis_to_iso(now_ms)),
        updated_at: Some(crate::time::millis_to_iso(now_ms)),
        date: Some(civil_date_key(now.year, now.month, now.day)),
        time: format!("{:02}:{:02}", now.hour, now.minute),
        sort_key: Some(now_ms as f64),
        text,
        content_json: Some(strip_tags_from_tiptap_doc(content_json)),
        tags,
        kind: BubbleKind::Plain,
        ..Default::default()
    })
}

/// `createBubbleFromContent` — the journal/draft constructor that tags via
/// `plainText` and strips `#tag`s out of the stored doc.
pub(crate) fn create_bubble_from_content(
    content_json: &Value,
    id: &str,
    date: Option<&str>,
    time: &str,
    sort_key: Option<f64>,
    plain_text: &str,
) -> Option<BubbleTimelineNode> {
    let (text, tags) = parse_bubble_draft(plain_text);
    if text.is_empty() {
        return None;
    }
    Some(BubbleTimelineNode {
        id: id.into(),
        date: crate::diary::storage::normalize_bubble_date_key_str(date, Some(time)),
        time: time.to_string(),
        sort_key,
        text,
        content_json: Some(strip_tags_from_tiptap_doc(content_json)),
        tags,
        kind: BubbleKind::Plain,
        ..Default::default()
    })
}
