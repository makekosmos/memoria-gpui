//! `parseInlineMarkdown` + mark helpers — inline markdown → PM inline nodes.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use super::parse::{node, text_node};
use super::{merge_marks, normalize_marks};

static LINK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\[([^\]]+)\]\((\S+?)(?:\s+"([^"]*)")?\)"#).expect("link re"));
static IMAGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^!\[([^\]]*)\]\((\S+?)(?:\s+"([^"]*)")?\)"#).expect("image re"));

struct LinkLike<'a> {
    label: &'a str,
    href: &'a str,
    title: Option<&'a str>,
    length: usize,
}

fn parse_link_like(text: &str, image: bool) -> Option<LinkLike<'_>> {
    let re = if image { &*IMAGE_RE } else { &*LINK_RE };
    let m = re.captures(text)?;
    Some(LinkLike {
        label: m.get(1).map(|c| c.as_str()).unwrap_or(""),
        href: m.get(2).map(|c| c.as_str()).unwrap_or(""),
        title: m.get(3).map(|c| c.as_str()),
        length: m.get(0).map(|c| c.as_str().len()).unwrap_or(0),
    })
}

fn mark(node_type: &str, attrs: Option<Map<String, Value>>) -> Value {
    let mut map = Map::new();
    map.insert("type".into(), Value::from(node_type));
    if let Some(attrs) = attrs {
        map.insert("attrs".into(), Value::Object(attrs));
    }
    Value::Object(map)
}

/// `matchDelimited` — returns (inner, nextIndex); `single` skips `**`/`__`
/// when looking for `*`/`_`.
fn match_delimited<'a>(
    text: &'a str,
    index: usize,
    delimiters: &[&str],
    single: bool,
) -> Option<(&'a str, usize)> {
    for delimiter in delimiters {
        if !text[index..].starts_with(delimiter) {
            continue;
        }
        let doubled = format!("{delimiter}{delimiter}");
        if single && text[index..].starts_with(&doubled) {
            continue;
        }
        let start = index + delimiter.len();
        let closing = text[start..].find(delimiter).map(|p| start + p);
        match closing {
            Some(end) if end > start => return Some((&text[start..end], end + delimiter.len())),
            _ => continue,
        }
    }
    None
}

/// `applyMark` — add mark to every text node (recursing into content).
pub(crate) fn apply_mark(nodes: Vec<Value>, next_mark: &Value) -> Vec<Value> {
    nodes
        .into_iter()
        .map(|n| {
            if n.get("type").and_then(Value::as_str) == Some("text") {
                let mut out = n.as_object().cloned().unwrap_or_default();
                let merged = merge_marks(normalize_marks(n.get("marks")), next_mark);
                out.insert("marks".into(), Value::Array(merged));
                Value::Object(out)
            } else if let Some(content) = n.get("content").and_then(Value::as_array) {
                let mut out = n.as_object().cloned().unwrap_or_default();
                out.insert(
                    "content".into(),
                    Value::Array(apply_mark(content.clone(), next_mark)),
                );
                Value::Object(out)
            } else {
                n
            }
        })
        .collect()
}

/// `appendTextNode` — merges into a trailing text node with equal marks.
pub(crate) fn append_text_node(nodes: &mut Vec<Value>, text: &str, marks: Vec<Value>) {
    if text.is_empty() {
        return;
    }
    let normalized = normalize_marks(Some(&Value::Array(marks)));
    let mergeable = nodes.last().is_some_and(|last| {
        last.get("type").and_then(Value::as_str) == Some("text")
            && last.get("text").and_then(Value::as_str).is_some()
            && serde_json::to_string(&normalize_marks(last.get("marks"))).unwrap_or_default()
                == serde_json::to_string(&normalized).unwrap_or_default()
    });
    if mergeable {
        if let Some(Value::String(text_slot)) =
            nodes.last_mut().and_then(|last| last.get_mut("text"))
        {
            text_slot.push_str(text);
        }
        return;
    }
    nodes.push(text_node(text, Some(normalized)));
}

/// `parseInlineMarkdown` — images, links, code spans, strong/strike/emphasis.
pub(crate) fn parse_inline_markdown(text: &str) -> Vec<Value> {
    let mut nodes: Vec<Value> = Vec::new();
    let mut buffer = String::new();
    let mut index = 0;

    macro_rules! flush_buffer {
        () => {
            if !buffer.is_empty() {
                append_text_node(&mut nodes, &buffer, Vec::new());
                buffer.clear();
            }
        };
    }

    while index < text.len() {
        if text[index..].starts_with("![") {
            if let Some(image) = parse_link_like(&text[index..], true) {
                flush_buffer!();
                let mut attrs = Map::new();
                attrs.insert("src".into(), Value::from(image.href));
                attrs.insert("alt".into(), Value::from(image.label));
                if let Some(title) = image.title {
                    attrs.insert("title".into(), Value::from(title));
                }
                nodes.push(node("image", Some(attrs), None));
                index += image.length;
                continue;
            }
        }

        if text[index..].starts_with('[') {
            if let Some(link) = parse_link_like(&text[index..], false) {
                flush_buffer!();
                let mut attrs = Map::new();
                attrs.insert("href".into(), Value::from(link.href));
                if let Some(title) = link.title {
                    attrs.insert("title".into(), Value::from(title));
                }
                let link_mark = mark("link", Some(attrs));
                let labelled = apply_mark(parse_inline_markdown(link.label), &link_mark);
                nodes.extend(labelled);
                index += link.length;
                continue;
            }
        }

        if text[index..].starts_with('`') {
            if let Some(rel) = text[index + 1..].find('`') {
                let closing = index + 1 + rel;
                if closing > index + 1 {
                    flush_buffer!();
                    let marks = vec![mark("code", None)];
                    append_text_node(&mut nodes, &text[index + 1..closing], marks);
                    index = closing + 1;
                    continue;
                }
            }
        }

        if let Some((inner, next)) = match_delimited(text, index, &["**", "__"], false) {
            flush_buffer!();
            nodes.extend(apply_mark(
                parse_inline_markdown(inner),
                &mark("bold", None),
            ));
            index = next;
            continue;
        }

        if let Some((inner, next)) = match_delimited(text, index, &["~~"], false) {
            flush_buffer!();
            nodes.extend(apply_mark(
                parse_inline_markdown(inner),
                &mark("strike", None),
            ));
            index = next;
            continue;
        }

        if let Some((inner, next)) = match_delimited(text, index, &["*", "_"], true) {
            flush_buffer!();
            nodes.extend(apply_mark(
                parse_inline_markdown(inner),
                &mark("italic", None),
            ));
            index = next;
            continue;
        }

        let ch = text[index..].chars().next().unwrap_or_default();
        buffer.push(ch);
        index += ch.len_utf8();
    }

    flush_buffer!();
    nodes
}
